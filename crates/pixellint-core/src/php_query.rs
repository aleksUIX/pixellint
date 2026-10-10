//! Bounded native-map readers from Matomo's PHP8 server, not URL coercion.
//!
//! Source: matomo-org/matomo 1e9169ddd9eba7c982dc67b8bd3f9310a7a312c6,
//! core/Common.php::getRequestVar and core/Tracker/Request.php::getParam.
//! Values whose conversion needs PHP architecture/precision or an unmapped
//! plugin reader stay explicitly unvalidated. They never become invented
//! missing-field errors.
use serde_json::Value;

pub(crate) const MATOMO_PHP8_SOURCE: &str = "https://raw.githubusercontent.com/matomo-org/matomo/1e9169ddd9eba7c982dc67b8bd3f9310a7a312c6/core/Common.php";

pub(crate) enum MatomoMap {
    Ignored,
    Ready(Vec<(String, String)>),
    Unsupported(Vec<String>),
}

#[derive(Clone, Copy)]
enum Reader {
    String,
    Integer(i32),
    Float,
    Json,
}

fn reader(name: &str) -> Option<Reader> {
    Some(match name {
        "idsite" | "rec" | "_refts" | "ping" | "bots" | "dp" | "new_visit" | "cdo" | "ca"
        | "cookie" | "pdf" | "fla" | "java" | "qt" | "realp" | "wma" | "ag" => Reader::Integer(0),
        "idgoal" | "search_count" | "pf_net" | "pf_srv" | "pf_tfr" | "pf_dm1" | "pf_dm2"
        | "pf_onl" => Reader::Integer(-1),
        "ec_st" | "ec_tx" | "ec_sh" | "ec_dt" | "_pkp" | "e_v" | "revenue" => Reader::Float,
        "ec_items" | "uadata" => Reader::Json,
        "_id" | "ua" | "lang" | "url" | "urlref" | "res" | "_ref" | "_rcn" | "_rck" | "ec_id"
        | "_pkc" | "_pks" | "_pkn" | "e_c" | "e_a" | "e_n" | "cip" | "cdt" | "cid" | "uid"
        | "cs" | "download" | "link" | "action_name" | "search" | "search_cat" | "pv_id"
        | "c_p" | "c_n" | "c_t" | "c_i" | "token_auth" => Reader::String,
        _ => return None,
    })
}

// The intersection of PHP32 and PHP64 integers avoids assuming server
// architecture. Floating stringification additionally depends on `precision`.
fn portable_number(value: &Value) -> Result<String, ()> {
    let Value::Number(number) = value else {
        return Err(());
    };
    let text = number.to_string();
    if let Ok(integer) = text.parse::<i32>() {
        return Ok(integer.to_string());
    }
    let float = text.parse::<f64>().map_err(|_| ())?;
    if !float.is_finite() || float.abs() > f64::from(i32::MAX) {
        return Err(());
    }
    if float.fract() == 0.0 {
        return Ok((float as i32).to_string());
    }
    // Nonintegral native numbers require server precision configuration.
    Err(())
}

fn numeric_text(text: &str) -> Option<f64> {
    let text = text.trim_matches(|c: char| matches!(c, '\t'..='\r' | ' '));
    if text.is_empty() {
        return None;
    }
    // PHP8 numeric strings allow surrounding ASCII whitespace, decimal and
    // exponent syntax. Rust's float parser also accepts inf/NaN, excluded here.
    if !text
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    let value: f64 = text.parse().ok()?;
    value.is_finite().then_some(value)
}

fn integer(value: &Value, default: i32) -> Result<Option<String>, ()> {
    let number = match value {
        Value::Null => return Ok(None),
        Value::Bool(true) => return Ok(Some("1".into())),
        Value::Bool(false) => return Ok(Some(default.to_string())),
        Value::String(text) => numeric_text(&sanitize(text)),
        Value::Number(_) => {
            let Value::Number(number) = value else {
                unreachable!()
            };
            let parsed = number.to_string().parse::<f64>().map_err(|_| ())?;
            if !parsed.is_finite() || parsed.abs() > f64::from(i32::MAX) {
                return Err(());
            }
            Some(parsed)
        }
        Value::Array(_) | Value::Object(_) => None,
    };
    let Some(number) = number else {
        return Ok(Some(default.to_string()));
    };
    if number < f64::from(i32::MIN) || number > f64::from(i32::MAX) {
        return Err(());
    }
    Ok(Some(if number.fract() == 0.0 {
        (number as i32).to_string()
    } else {
        default.to_string()
    }))
}

fn string(value: &Value) -> Result<Option<String>, ()> {
    match value {
        Value::String(text) => Ok((!text.is_empty()).then(|| sanitize(text))),
        Value::Number(_) => portable_number(value).map(Some),
        // Common's string reader accepts int/float but rejects boolean/array.
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => Ok(None),
    }
}

fn float(value: &Value) -> Result<Option<String>, ()> {
    let text = match value {
        Value::Null | Value::Bool(false) => return Ok(None),
        Value::Bool(true) => "1".into(),
        Value::Number(number) => {
            let text = number.to_string();
            let parsed = text.parse::<f64>().map_err(|_| ())?;
            if !parsed.is_finite() {
                return Err(());
            }
            text
        }
        Value::String(text) if text.is_empty() => return Ok(None),
        Value::String(text) => sanitize(text),
        // str_replace preserves arrays; preg_match then has runtime-dependent
        // failure behavior. Do not pretend that PHP returns a numeric default.
        Value::Array(_) | Value::Object(_) => return Err(()),
    };
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        regex::Regex::new(r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")
            .expect("static PHP float grammar")
    });
    // PHP PCRE permits one final LF before its dollar anchor.
    // Common validates a comma-normalized copy, then settype casts the
    // original value. In PHP, "1,5" consequently becomes 1, not 1.5.
    if !pattern.is_match(&text.strip_suffix('\n').unwrap_or(&text).replace(',', ".")) {
        return Ok(Some("0".into()));
    }
    if let Some((prefix, _)) = text.split_once(',') {
        let number = numeric_text(prefix).unwrap_or(0.0);
        if !number.is_finite() || number < f64::from(i32::MIN) || number > f64::from(i32::MAX) {
            return Err(());
        }
        return Ok(Some((number as i32).to_string()));
    }
    let Some(number) = numeric_text(&text) else {
        return Err(());
    };
    if !number.is_finite() {
        return Err(());
    }
    // Keep the documented numeric grammar; numeric format checks accept its
    // exponent spelling and avoid a PHP precision-dependent reserialization.
    Ok(Some(text.strip_suffix('\n').unwrap_or(&text).to_string()))
}

pub(crate) fn matomo_php8_map(value: &Value) -> MatomoMap {
    let entries: Vec<(String, &Value)> = match value {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(array) => array
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => return MatomoMap::Ignored,
    };
    if entries.is_empty() {
        return MatomoMap::Ignored;
    }
    let mut pairs = Vec::new();
    let mut unsupported = Vec::new();
    for (name, value) in entries {
        let result = match reader(&name) {
            Some(Reader::String) if name == "token_auth" => string(value).map(|text| {
                text.map(|text| text.replace('\0', ""))
                    .filter(|text| !text.is_empty() && text != "0")
            }),
            Some(Reader::String) => string(value),
            Some(Reader::Integer(default)) => integer(value, default),
            Some(Reader::Float) => float(value),
            Some(Reader::Json) => match value {
                Value::Null => Ok(None),
                Value::String(text) if text.is_empty() => Ok(None),
                Value::String(text) => match serde_json::from_str::<Value>(text) {
                    Ok(value) => sanitize_json(&value).map(|value| Some(value.to_string())),
                    // Keep malformed submitted JSON available to the existing
                    // wire-format rule instead of inventing a valid entity.
                    Err(_) => Ok(Some(text.clone())),
                },
                other => sanitize_json(other).map(|value| Some(value.to_string())),
            },
            // Ordinary string fields retain their literal bytes. Native types
            // need that field's actual PHP or plugin reader to be source-backed.
            None => match value {
                Value::String(text) => Ok(Some(text.clone())),
                Value::Null => Ok(None),
                _ => Err(()),
            },
        };
        match result {
            Ok(Some(text)) => pairs.push((name, text)),
            Ok(None) => {}
            Err(()) => unsupported.push(name),
        }
    }
    if unsupported.is_empty() {
        MatomoMap::Ready(pairs)
    } else {
        MatomoMap::Unsupported(unsupported)
    }
}

/// The portable part of PHP's ordered array keys. Large numeric indices and
/// negative-only auto-append have architecture or PHP minor-version semantics.
#[derive(Clone, Debug, PartialEq)]
enum PhpKey {
    Integer(i32),
    String(String),
}

#[derive(Default, Debug)]
struct PhpArray(Vec<(PhpKey, PhpNode)>);

#[derive(Debug)]
enum PhpNode {
    String(String),
    Array(PhpArray),
}

fn php_key(text: &str) -> Result<PhpKey, ()> {
    let numeric = text.parse::<i64>().ok();
    if numeric.is_some_and(|number| number.to_string() == text) {
        return numeric
            .and_then(|number| i32::try_from(number).ok())
            .map(PhpKey::Integer)
            .ok_or(());
    }
    // A canonical integer outside even PHP64 remains a string key. Values
    // between the PHP32 and PHP64 ranges require deployment architecture.
    Ok(PhpKey::String(text.into()))
}

impl PhpArray {
    fn assign(&mut self, keys: &[Option<PhpKey>], value: String) -> Result<(), ()> {
        let key = match &keys[0] {
            Some(key) => key.clone(),
            None => {
                let maximum = self
                    .0
                    .iter()
                    .filter_map(|(key, _)| match key {
                        PhpKey::Integer(number) => Some(*number),
                        PhpKey::String(_) => None,
                    })
                    .max();
                // PHP8.3 changed negative-only append. Preserve this boundary.
                if maximum.is_some_and(|number| number < -1) {
                    return Err(());
                }
                PhpKey::Integer(maximum.unwrap_or(-1).checked_add(1).ok_or(())?)
            }
        };
        let index = self.0.iter().position(|(existing, _)| *existing == key);
        let index = match index {
            Some(index) => index,
            None => {
                self.0.push((key, PhpNode::String(String::new())));
                self.0.len() - 1
            }
        };
        if keys.len() == 1 {
            self.0[index].1 = PhpNode::String(value);
        } else {
            if !matches!(self.0[index].1, PhpNode::Array(_)) {
                self.0[index].1 = PhpNode::Array(PhpArray::default());
            }
            let PhpNode::Array(array) = &mut self.0[index].1 else {
                unreachable!()
            };
            array.assign(&keys[1..], value)?;
        }
        Ok(())
    }

    fn value(self) -> Value {
        let dense = self.0.iter().enumerate().all(|(index, (key, _))| {
            matches!(key, PhpKey::Integer(number) if usize::try_from(*number) == Ok(index))
        });
        let node = |value: PhpNode| match value {
            PhpNode::String(text) => Value::String(text),
            PhpNode::Array(array) => array.value(),
        };
        if dense {
            Value::Array(self.0.into_iter().map(|(_, value)| node(value)).collect())
        } else {
            Value::Object(
                self.0
                    .into_iter()
                    .map(|(key, value)| {
                        let key = match key {
                            PhpKey::Integer(number) => number.to_string(),
                            PhpKey::String(text) => text,
                        };
                        (key, node(value))
                    })
                    .collect(),
            )
        }
    }
}

fn query_decode(text: &str) -> Result<String, ()> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut input = text.as_bytes().iter().copied().peekable();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let mut probe = input.clone();
            let hex = |byte: u8| char::from(byte).to_digit(16).map(|value| value as u8);
            if let Some((high, low)) = probe.next().and_then(hex).zip(probe.next().and_then(hex)) {
                bytes.push((high << 4) | low);
                input = probe;
                continue;
            }
        }
        bytes.push(if byte == b'+' { b' ' } else { byte });
    }
    String::from_utf8(bytes).map_err(|_| ())
}

fn bracket_keys(name: &str) -> Result<Vec<Option<PhpKey>>, ()> {
    // php_register_variable_ex uses C strings for decoded names.
    let name = name
        .split('\0')
        .next()
        .unwrap_or_default()
        .trim_start_matches(' ');
    let (root, mut rest) = name
        .split_once('[')
        .map_or((name, ""), |(root, tail)| (root, tail));
    let root = root.replace([' ', '.'], "_");
    if root.is_empty() {
        return Ok(Vec::new());
    }
    let mut keys = vec![Some(php_key(&root)?)];
    if !name.contains('[') {
        return Ok(keys);
    }
    loop {
        let (key, tail) = rest.split_once(']').ok_or(())?;
        // PHP treats [] and a single ASCII whitespace before ] as append.
        let append =
            key.is_empty() || (key.len() == 1 && matches!(key.as_bytes()[0], 9..=13 | b' '));
        keys.push(if append { None } else { Some(php_key(key)?) });
        if keys.len() > 65 {
            return Err(());
        }
        let Some(next) = tail.strip_prefix('[') else {
            // PHP ignores trailing text after the last complete index.
            break;
        };
        rest = next;
    }
    Ok(keys)
}

/// Bounded PHP8 parse_str profile. At most 1000 '&' variables and 64 bracket
/// levels are modeled. Configuration-sensitive, malformed or nonportable forms
/// stay explicitly unvalidated, without inventing missing event fields.
fn parse_query(query: &str) -> Result<Value, &'static str> {
    if query.contains(';') || query.split('&').filter(|part| !part.is_empty()).count() > 1000 {
        return Err("PHP query separator or input-limit context");
    }
    let mut fields = PhpArray::default();
    for part in query.split('&').filter(|part| !part.is_empty()) {
        let (name, value) = part.split_once('=').unwrap_or((part, ""));
        let name = query_decode(name).map_err(|_| "non-UTF-8 PHP query bytes")?;
        let value = query_decode(value).map_err(|_| "non-UTF-8 PHP query bytes")?;
        let keys = bracket_keys(&name)
            .map_err(|_| "PHP malformed bracket, nesting or numeric-key context")?;
        if keys.is_empty() {
            continue;
        }
        fields
            .assign(&keys, value)
            .map_err(|_| "PHP negative append or numeric-key context")?;
    }
    Ok(fields.value())
}

pub(crate) fn matomo_php8_query(query: &str) -> MatomoMap {
    // BulkTracking tests PHP empty() on the parse_url query before parse_str.
    if query.is_empty() || query == "0" {
        return MatomoMap::Ignored;
    }
    // BulkTracking calls parse_url before parse_str. Literal control bytes
    // need its URL normalization; percent-encoded bytes are decoded later.
    if query.bytes().any(|byte| byte.is_ascii_control()) {
        return MatomoMap::Unsupported(vec![
            "PHP parse_url control-character normalization context".into(),
        ]);
    }
    let mut fields = match parse_query(query) {
        Ok(Value::Object(fields)) => fields,
        Ok(Value::Array(fields)) if fields.is_empty() => return MatomoMap::Ignored,
        Ok(other) => return matomo_php8_map(&other),
        Err(reason) => return MatomoMap::Unsupported(vec![reason.into()]),
    };
    // Deferred macros retain their original spellings. Coercing them to PHP
    // numeric defaults would erase the engine's expansion-state evidence.
    let names: Vec<_> = fields
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_str()
                .filter(|text| !crate::detect_macro_spans(text).is_empty())
                .map(|_| name.clone())
        })
        .collect();
    let preserved: Vec<_> = names
        .into_iter()
        .map(|name| {
            let value = fields.remove(&name).unwrap().as_str().unwrap().to_string();
            (name, value)
        })
        .collect();
    match matomo_php8_map(&Value::Object(fields)) {
        MatomoMap::Ready(mut pairs) => {
            pairs.extend(preserved);
            MatomoMap::Ready(pairs)
        }
        MatomoMap::Ignored if !preserved.is_empty() => MatomoMap::Ready(preserved),
        other => other,
    }
}

/// Common::sanitizeString uses one HTML4 decode pass, removes literal null
/// bytes and applies ENT_QUOTES escaping. HTML5-only names stay literal.
fn sanitize(text: &str) -> String {
    static ENTITIES: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
        std::sync::OnceLock::new();
    let entities = ENTITIES.get_or_init(|| {
        serde_json::from_str(include_str!("php_html4_entities.json"))
            .expect("executed PHP HTML4 translation table")
    });
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        rest = &rest[start..];
        let token = rest[1..].find(['&', ';']).and_then(|end| {
            let end = end + 1;
            (rest.as_bytes()[end] == b';').then(|| &rest[..=end])
        });
        let value = token.and_then(|token| {
            let inner = &token[1..token.len() - 1];
            if let Some(number) = inner.strip_prefix('#') {
                let number = if let Some(hex) = number.strip_prefix(['x', 'X']) {
                    (!hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
                        .then(|| u32::from_str_radix(hex, 16).ok())
                        .flatten()
                } else {
                    (!number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
                        .then(|| number.parse::<u32>().ok())
                        .flatten()
                }?;
                // ENT_HTML401 excludes controls except TAB, LF and CR, as well
                // as C1 controls and invalid Unicode scalars.
                if !matches!(number, 9 | 10 | 13 | 32..=126 | 160..=0x10ffff) {
                    return None;
                }
                char::from_u32(number).map(|character| character.to_string())
            } else {
                entities.get(token).cloned()
            }
        });
        if let (Some(token), Some(value)) = (token, value) {
            decoded.push_str(&value);
            rest = &rest[token.len()..];
        } else {
            decoded.push('&');
            rest = &rest[1..];
        }
    }
    decoded.push_str(rest);
    let mut escaped = String::with_capacity(decoded.len());
    for character in decoded.chars() {
        match character {
            '\0' => {}
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#039;"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn sanitize_json(value: &Value) -> Result<Value, ()> {
    Ok(match value {
        Value::String(text) => Value::String(sanitize(text)),
        Value::Array(array) => {
            Value::Array(array.iter().map(sanitize_json).collect::<Result<_, _>>()?)
        }
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (name, value) in map {
                let name = sanitize(name);
                // PHP's in-place key collisions depend on original insertion
                // order. The bounded JSON view does not expose that context.
                if result.insert(name, sanitize_json(value)?).is_some() {
                    return Err(());
                }
            }
            Value::Object(result)
        }
        other => other.clone(),
    })
}

/// Outer bulk credentials use Common's string reader, then PHP empty().
/// String "0" is empty in PHP and must permit per-item token fallback.
pub(crate) fn matomo_php8_auth_string(value: &Value) -> Result<Option<String>, ()> {
    string(value).map(|text| text.filter(|text| text != "0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn pairs(value: Value) -> Vec<(String, String)> {
        match matomo_php8_map(&value) {
            MatomoMap::Ready(pairs) => pairs,
            _ => panic!("expected supported map"),
        }
    }
    #[test]
    fn bracket_trees_match_executed_php_parse_str() {
        let oracle: Value = serde_json::from_str(include_str!(
            "../../../fixtures/vendor-matomo-php-tree-depth/producer-proof.json"
        ))
        .unwrap();
        for (index, row) in oracle["rows"].as_array().unwrap().iter().enumerate() {
            let actual = parse_query(row["query"].as_str().unwrap());
            if matches!(index, 18 | 19 | 20 | 21) {
                assert!(actual.is_err(), "explicit portable boundary: {row}");
            } else {
                assert_eq!(actual.unwrap(), row["parsed"], "query {index}: {row}");
            }
        }
    }

    #[test]
    fn deterministic_generated_bracket_vectors_match_executed_php() {
        let oracle: Value = serde_json::from_str(include_str!(
            "../../../fixtures/vendor-matomo-php-tree-depth/generated-parser-proof.json"
        ))
        .unwrap();
        for row in oracle["rows"].as_array().unwrap() {
            assert_eq!(
                parse_query(row["query"].as_str().unwrap()).unwrap(),
                row["parsed"],
                "{row}"
            );
        }
        let oracle: Value = serde_json::from_str(include_str!(
            "../../../fixtures/vendor-matomo-php-tree-depth/extended-sanitizer-proof.json"
        ))
        .unwrap();
        for row in oracle["sanitizer"].as_array().unwrap() {
            assert_eq!(
                sanitize(row["input"].as_str().unwrap()),
                row["output"].as_str().unwrap()
            );
        }
        for row in oracle["floats"].as_array().unwrap() {
            assert_eq!(
                float(&row["input"])
                    .unwrap()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                row["output"].as_f64().unwrap()
            );
        }
    }

    #[test]
    fn sanitizer_long_values_stay_linear_and_retain_numeric_entity_semantics() {
        let entity = format!("&#{}49;", "0".repeat(4096));
        assert_eq!(sanitize(&entity), "1");
        let repeated = "&".repeat(100_000);
        assert_eq!(sanitize(&repeated), "&amp;".repeat(100_000));
        assert_eq!(sanitize("&amp;#49;"), "&amp;#49;");
        assert_eq!(sanitize("&apos;"), "&amp;apos;");
    }

    #[test]
    fn sanitizer_matches_executed_php_html4_and_native_readers() {
        let oracle: Value = serde_json::from_str(include_str!(
            "../../../fixtures/vendor-matomo-php-tree-depth/producer-proof.json"
        ))
        .unwrap();
        for row in oracle["numeric_entities"].as_array().unwrap() {
            let input = format!("&#{};", row["codepoint"]);
            assert_eq!(
                sanitize(&input),
                row["sanitized"].as_str().unwrap(),
                "{row}"
            );
        }
        for row in oracle["native"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            let actual = pairs(json!({name: row["input"]}));
            let expected = match &row["output"] {
                Value::String(text) => Some(text.to_string()),
                Value::Number(number) => Some(number.to_string()),
                other => panic!("unsupported native oracle output: {other}"),
            };
            let actual = actual
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone());
            assert_eq!(actual, expected, "{row}");
        }
    }

    #[test]
    fn native_float_readers_match_executed_pinned_php_source() {
        let oracle: Value = serde_json::from_str(include_str!(
            "../../../fixtures/vendor-matomo-php-scalar-depth/producer-proof.json"
        ))
        .unwrap();
        for row in oracle["native_float_rows"].as_array().unwrap() {
            let input = &row["input"];
            let actual = float(input);
            let portable_comma = input.as_str().is_some_and(|text| {
                text.split_once(',').is_some_and(|(prefix, _)| {
                    numeric_text(prefix).is_some_and(|number| {
                        number < f64::from(i32::MIN) || number > f64::from(i32::MAX)
                    })
                })
            });
            if row.get("exception_class").is_some() || portable_comma {
                assert!(actual.is_err(), "source-dependent boundary: {row}");
            } else if row["output"] == Value::Bool(false) {
                assert_eq!(actual, Ok(None), "{row}");
            } else {
                let value = actual.unwrap().unwrap().parse::<f64>().unwrap();
                assert_eq!(Some(value), row["output"].as_f64(), "{row}");
            }
        }
    }
    #[test]
    fn upstream_common_examples_and_php8_comparison_boundaries() {
        // Independent rows from pinned CommonTest::getRequestVarValues and
        // PHP's type comparison tables, applied to Request's field defaults.
        for (value, expected) in [
            (json!("45645646"), "45645646"),
            (json!(1413.431413), "0"),
            (json!(["test", 1345524, ["gaga"]]), "0"),
            (json!("1e0"), "1"),
            (json!("01"), "1"),
            (json!(" 1 \t"), "1"),
            (json!(true), "1"),
            (json!(false), "0"),
            (json!("1.5"), "0"),
        ] {
            let result = integer(&value, 0).unwrap() == Some(expected.into());
            assert!(result);
        }
        assert_eq!(
            pairs(json!({"idsite":1,"rec":true,"action_name":123,"e_c":false})),
            vec![
                ("action_name".into(), "123".into()),
                ("idsite".into(), "1".into()),
                ("rec".into(), "1".into())
            ]
        );
    }
    #[test]
    fn native_json_does_not_url_decode_and_empty_entries_are_ignored() {
        assert!(matches!(matomo_php8_map(&json!({})), MatomoMap::Ignored));
        assert!(matches!(matomo_php8_map(&json!([])), MatomoMap::Ignored));
        assert!(matches!(matomo_php8_map(&json!(false)), MatomoMap::Ignored));
        assert_eq!(
            pairs(
                json!({"url":"https://example.com/a%20b?q=a+b","ec_items":[["sku","name",[],1,2]]})
            ),
            vec![
                ("ec_items".into(), "[[\"sku\",\"name\",[],1,2]]".into()),
                ("url".into(), "https://example.com/a%20b?q=a+b".into())
            ]
        );
    }
    #[test]
    fn precision_and_plugin_readers_remain_observable() {
        assert!(matches!(
            matomo_php8_map(&json!({"idsite":2147483648i64})),
            MatomoMap::Unsupported(_)
        ));
        assert_eq!(
            pairs(json!({"e_v":0.1})),
            vec![("e_v".into(), "0.1".into())]
        );
        assert!(matches!(
            matomo_php8_map(&json!({"ma_st":true})),
            MatomoMap::Unsupported(_)
        ));
        assert_eq!(matomo_php8_auth_string(&json!("0")).unwrap(), None);
        assert_eq!(
            matomo_php8_auth_string(&json!(123)).unwrap(),
            Some("123".into())
        );
        assert_eq!(matomo_php8_auth_string(&json!(true)).unwrap(), None);
    }
}
