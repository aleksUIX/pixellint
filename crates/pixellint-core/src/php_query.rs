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
        "idsite" | "rec" | "_refts" | "ping" | "bots" | "dp" | "new_visit" | "cdo" | "ca" => {
            Reader::Integer(0)
        }
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
    let text = text.trim_matches(|c: char| c.is_ascii_whitespace());
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
        Value::String(text) => numeric_text(text),
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
        Value::String(text) => Ok((!text.is_empty()).then(|| text.clone())),
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
        Value::String(text) => text.clone(),
        // str_replace preserves arrays; preg_match then has runtime-dependent
        // failure behavior. Do not pretend that PHP returns a numeric default.
        Value::Array(_) | Value::Object(_) => return Err(()),
    };
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        regex::Regex::new(r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")
            .expect("static PHP float grammar")
    });
    // Common validates a comma-normalized copy, then settype casts the
    // original value. In PHP, "1,5" consequently becomes 1, not 1.5.
    if !pattern.is_match(&text.replace(',', ".")) {
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
    Ok(Some(text))
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
                Value::String(text) => Ok((!text.is_empty()).then(|| text.clone())),
                other => Ok(Some(other.to_string())),
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

/// Bounded scalar parse_str profile for string bulk entries. The pinned PHP8
/// defaults use '&' and max_input_vars=1000. Bracket trees, alternate separator
/// candidates, invalid UTF-8 and sanitizer-sensitive values stay explicit gaps.
pub(crate) fn matomo_php8_query(query: &str) -> MatomoMap {
    // BulkTracking tests PHP empty() on the parse_url query before parse_str.
    if query.is_empty() || query == "0" {
        return MatomoMap::Ignored;
    }
    if query.contains(';') || query.split('&').filter(|part| !part.is_empty()).count() > 1000 {
        return MatomoMap::Unsupported(vec!["PHP query separator or input-limit context".into()]);
    }
    let mut fields = serde_json::Map::new();
    let mut macros = Vec::new();
    for part in query.split('&').filter(|part| !part.is_empty()) {
        let (name, value) = part.split_once('=').unwrap_or((part, ""));
        let decode = |text: &str| {
            let mut bytes = Vec::new();
            let mut input = text.as_bytes().iter().copied().peekable();
            while let Some(byte) = input.next() {
                if byte == b'%' {
                    let mut probe = input.clone();
                    let hex = |byte: u8| char::from(byte).to_digit(16).map(|value| value as u8);
                    if let Some((high, low)) =
                        probe.next().and_then(hex).zip(probe.next().and_then(hex))
                    {
                        bytes.push((high << 4) | low);
                        input = probe;
                        continue;
                    }
                }
                bytes.push(if byte == b'+' { b' ' } else { byte });
            }
            String::from_utf8(bytes)
        };
        let (Ok(name), Ok(value)) = (decode(name), decode(value)) else {
            return MatomoMap::Unsupported(vec!["non-UTF-8 PHP query bytes".into()]);
        };
        let name = name.trim_start_matches(' ').replace([' ', '.'], "_");
        if name.contains(['[', '\0']) || value.contains(['\0', '&', '<', '>']) {
            return MatomoMap::Unsupported(vec![
                "PHP bracket, sanitizer or float conversion context".into(),
            ]);
        }
        if name.is_empty() {
            continue;
        }
        if !crate::detect_macro_spans(&value).is_empty() {
            macros.push(name.clone());
        } else {
            macros.retain(|previous| previous != &name);
        }
        // PHP scalar assignments shadow prior values after name normalization.
        fields.insert(name, Value::String(value));
    }
    let mut preserved = Vec::new();
    for name in macros {
        if let Some(Value::String(value)) = fields.remove(&name) {
            preserved.push((name, value));
        }
    }
    match matomo_php8_map(&Value::Object(fields)) {
        MatomoMap::Ready(mut pairs) => {
            pairs.extend(preserved);
            MatomoMap::Ready(pairs)
        }
        MatomoMap::Ignored if !preserved.is_empty() => MatomoMap::Ready(preserved),
        other => other,
    }
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
