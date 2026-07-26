//! A span-tracking JSON reader.
//!
//! Conversion APIs carry their events in a JSON request body, so rulepacks need
//! to contract fields inside that body and point at the exact bytes when one is
//! wrong. A structural parser such as `serde_json` gives the shape but discards
//! positions, so this module walks the document itself and records a byte span
//! for every value it finds.
//!
//! Fields are addressed by path: `data[0].user_data.em[1]`. Patterns use an
//! empty subscript to mean "every element", so `data[].event_name` expands to
//! one concrete path per event in the payload.

use std::collections::BTreeMap;
use std::fmt;

/// Deepest nesting the reader accepts. Bodies arrive from the network, so an
/// adversarial payload of a hundred thousand open brackets must not take the
/// stack down with it.
const MAX_DEPTH: usize = 64;

/// What kind of JSON value sits at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonValueKind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

impl JsonValueKind {
    /// The name used in violation text.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Array => "array",
            Self::String => "string",
            Self::Number => "number",
            Self::Bool => "boolean",
            Self::Null => "null",
        }
    }
}

/// One value in the document, with the byte span it occupies in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsonField {
    pub(crate) path: String,
    pub(crate) kind: JsonValueKind,
    /// Scalar values as text: strings unescaped, numbers and literals verbatim.
    /// Containers carry an empty string.
    pub(crate) text: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// Element count for arrays, member count for objects.
    pub(crate) len: Option<usize>,
}

impl JsonField {
    /// Whether the value counts as empty for a presence check. A container with
    /// no members is as absent as a blank string, and a JSON `null` is the way
    /// most senders spell "I had nothing to put here".
    pub(crate) fn is_blank(&self) -> bool {
        match self.kind {
            JsonValueKind::Null => true,
            JsonValueKind::Object | JsonValueKind::Array => self.len == Some(0),
            _ => self.text.trim().is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsonError {
    pub(crate) message: String,
    pub(crate) offset: usize,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

/// A parsed document, flattened to path to value.
#[derive(Debug, Clone, Default)]
pub(crate) struct JsonDocument {
    fields: BTreeMap<String, JsonField>,
}

impl JsonDocument {
    pub(crate) fn parse(input: &str) -> Result<Self, JsonError> {
        let mut reader = Reader {
            bytes: input.as_bytes(),
            input,
            pos: 0,
            fields: BTreeMap::new(),
        };

        reader.skip_whitespace();
        reader.read_value(String::new(), 0)?;
        reader.skip_whitespace();

        if reader.pos < reader.bytes.len() {
            return Err(reader.error("unexpected trailing content"));
        }

        Ok(Self {
            fields: reader.fields,
        })
    }

    /// Whether the text looks like it is meant to be a JSON document. Used to
    /// route an artifact whose kind the caller did not state.
    pub(crate) fn looks_like_json(input: &str) -> bool {
        let trimmed = input.trim_start();
        trimmed.starts_with('{') || trimmed.starts_with('[')
    }

    pub(crate) fn get(&self, path: &str) -> Option<&JsonField> {
        self.fields.get(path)
    }

    pub(crate) fn contains(&self, path: &str) -> bool {
        self.fields.contains_key(path)
    }

    /// The span of the nearest ancestor that is present, so a violation about a
    /// missing field can still point somewhere useful.
    pub(crate) fn nearest_present_ancestor(&self, path: &str) -> Option<&JsonField> {
        let mut candidate = path;

        while let Some(parent) = parent_path(candidate) {
            if let Some(field) = self.fields.get(parent) {
                return Some(field);
            }
            candidate = parent;
        }

        self.fields.get("")
    }

    fn array_len(&self, path: &str) -> Option<usize> {
        match self.fields.get(path) {
            Some(field) if field.kind == JsonValueKind::Array => field.len,
            _ => None,
        }
    }

    /// Turns a pattern such as `data[].user_data.em[]` into the concrete paths
    /// it addresses in this document.
    ///
    /// A branch dies when a container on the way is missing: an absent `data`
    /// yields nothing rather than a stream of complaints about every field
    /// underneath it. Only the last step is allowed to be absent, because that
    /// absence is exactly what a presence contract is there to report.
    pub(crate) fn expand(&self, pattern: &str) -> Vec<String> {
        let steps = parse_pattern(pattern);
        if steps.is_empty() {
            return Vec::new();
        }

        let last = steps.len() - 1;
        let mut prefixes = vec![String::new()];

        for (index, step) in steps.iter().enumerate() {
            let mut next = Vec::new();

            for prefix in &prefixes {
                let path = join_key(prefix, &step.key);

                // An intermediate key has to exist for anything below it to be
                // addressable. The final key is left to the contract.
                if index < last && !self.contains(&path) {
                    continue;
                }

                if step.subscripts == 0 {
                    next.push(path);
                    continue;
                }

                let mut level = vec![path];
                for _ in 0..step.subscripts {
                    let mut expanded = Vec::new();
                    for candidate in &level {
                        let Some(len) = self.array_len(candidate) else {
                            continue;
                        };
                        for element in 0..len {
                            expanded.push(format!("{candidate}[{element}]"));
                        }
                    }
                    level = expanded;
                }

                next.extend(level);
            }

            prefixes = next;
            if prefixes.is_empty() {
                return Vec::new();
            }
        }

        prefixes
    }

    /// Whether at least one concrete path under this pattern is present. This is
    /// what a pack matches its shape on.
    pub(crate) fn matches_pattern(&self, pattern: &str) -> bool {
        self.expand(pattern).iter().any(|path| self.contains(path))
    }
}

/// Whether a path pattern is well formed. Checked when a manifest is compiled,
/// so a typo in a path fails the load rather than silently matching nothing.
pub(crate) fn is_valid_pattern(pattern: &str) -> bool {
    !parse_pattern(pattern).is_empty()
}

/// One step of a path pattern: a key, plus however many subscripts follow it.
struct PatternStep {
    key: String,
    subscripts: usize,
}

fn parse_pattern(pattern: &str) -> Vec<PatternStep> {
    let mut steps = Vec::new();

    for token in pattern.split('.') {
        if token.is_empty() {
            return Vec::new();
        }

        let mut key = token;
        let mut subscripts = 0;
        while let Some(stripped) = key.strip_suffix("[]") {
            key = stripped;
            subscripts += 1;
        }

        if key.is_empty() {
            return Vec::new();
        }

        steps.push(PatternStep {
            key: key.to_string(),
            subscripts,
        });
    }

    steps
}

fn join_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// The path one level up, whether the last step was a key or a subscript.
fn parent_path(path: &str) -> Option<&str> {
    if path.is_empty() {
        return None;
    }

    if path.ends_with(']')
        && let Some(index) = path.rfind('[')
    {
        return Some(&path[..index]);
    }

    match path.rfind('.') {
        Some(index) => Some(&path[..index]),
        None => Some(""),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    input: &'a str,
    pos: usize,
    fields: BTreeMap<String, JsonField>,
}

impl<'a> Reader<'a> {
    fn error(&self, message: &str) -> JsonError {
        JsonError {
            message: message.to_string(),
            offset: self.pos,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected `{}`", byte as char)))
        }
    }

    fn record(
        &mut self,
        path: String,
        kind: JsonValueKind,
        text: String,
        start: usize,
        end: usize,
        len: Option<usize>,
    ) {
        self.fields.insert(
            path.clone(),
            JsonField {
                path,
                kind,
                text,
                start,
                end,
                len,
            },
        );
    }

    fn read_value(&mut self, path: String, depth: usize) -> Result<(), JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting is too deep"));
        }

        match self.peek() {
            Some(b'{') => self.read_object(path, depth),
            Some(b'[') => self.read_array(path, depth),
            Some(b'"') => {
                let start = self.pos;
                let text = self.read_string()?;
                self.record(path, JsonValueKind::String, text, start, self.pos, None);
                Ok(())
            }
            Some(b't') | Some(b'f') | Some(b'n') => {
                let start = self.pos;
                let (literal, kind) = self.read_literal()?;
                self.record(path, kind, literal, start, self.pos, None);
                Ok(())
            }
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => {
                let start = self.pos;
                let number = self.read_number()?;
                self.record(path, JsonValueKind::Number, number, start, self.pos, None);
                Ok(())
            }
            Some(_) => Err(self.error("expected a value")),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn read_object(&mut self, path: String, depth: usize) -> Result<(), JsonError> {
        let start = self.pos;
        self.expect(b'{')?;
        self.skip_whitespace();

        let mut members = 0;

        if self.peek() == Some(b'}') {
            self.pos += 1;
            self.record(
                path,
                JsonValueKind::Object,
                String::new(),
                start,
                self.pos,
                Some(0),
            );
            return Ok(());
        }

        loop {
            self.skip_whitespace();
            let key = self.read_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            self.read_value(join_key(&path, &key), depth + 1)?;
            members += 1;
            self.skip_whitespace();

            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }

        self.record(
            path,
            JsonValueKind::Object,
            String::new(),
            start,
            self.pos,
            Some(members),
        );
        Ok(())
    }

    fn read_array(&mut self, path: String, depth: usize) -> Result<(), JsonError> {
        let start = self.pos;
        self.expect(b'[')?;
        self.skip_whitespace();

        let mut elements = 0;

        if self.peek() == Some(b']') {
            self.pos += 1;
            self.record(
                path,
                JsonValueKind::Array,
                String::new(),
                start,
                self.pos,
                Some(0),
            );
            return Ok(());
        }

        loop {
            self.skip_whitespace();
            self.read_value(format!("{path}[{elements}]"), depth + 1)?;
            elements += 1;
            self.skip_whitespace();

            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }

        self.record(
            path,
            JsonValueKind::Array,
            String::new(),
            start,
            self.pos,
            Some(elements),
        );
        Ok(())
    }

    /// Reads a string and returns its unescaped content.
    fn read_string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        let mut literal_start = self.pos;

        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("unterminated string"));
            };

            match byte {
                b'"' => {
                    out.push_str(&self.input[literal_start..self.pos]);
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    out.push_str(&self.input[literal_start..self.pos]);
                    self.pos += 1;
                    let escaped = self.read_escape()?;
                    out.push_str(&escaped);
                    literal_start = self.pos;
                }
                0x00..=0x1f => return Err(self.error("control character in string")),
                _ => self.pos += 1,
            }
        }
    }

    fn read_escape(&mut self) -> Result<String, JsonError> {
        let Some(byte) = self.peek() else {
            return Err(self.error("unterminated escape"));
        };
        self.pos += 1;

        let simple = match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.read_unicode_escape(),
            _ => return Err(self.error("unknown escape")),
        };

        Ok(simple.to_string())
    }

    fn read_unicode_escape(&mut self) -> Result<String, JsonError> {
        let first = self.read_hex4()?;

        // Surrogate pairs arrive as two escapes and mean one character. A lone
        // half is not a character, so it becomes the replacement rather than an
        // error: the point here is to read the payload, not to police it.
        if (0xd800..0xdc00).contains(&first) {
            let saved = self.pos;
            if self.peek() == Some(b'\\') {
                self.pos += 1;
                if self.peek() == Some(b'u') {
                    self.pos += 1;
                    let second = self.read_hex4()?;
                    if (0xdc00..0xe000).contains(&second) {
                        let combined = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
                        return Ok(char::from_u32(combined)
                            .unwrap_or(char::REPLACEMENT_CHARACTER)
                            .to_string());
                    }
                }
            }
            self.pos = saved;
            return Ok(char::REPLACEMENT_CHARACTER.to_string());
        }

        Ok(char::from_u32(first)
            .unwrap_or(char::REPLACEMENT_CHARACTER)
            .to_string())
    }

    fn read_hex4(&mut self) -> Result<u32, JsonError> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.error("truncated unicode escape"));
        }

        let digits = &self.input[self.pos..self.pos + 4];
        let value =
            u32::from_str_radix(digits, 16).map_err(|_| self.error("invalid unicode escape"))?;
        self.pos += 4;
        Ok(value)
    }

    fn read_literal(&mut self) -> Result<(String, JsonValueKind), JsonError> {
        for (literal, kind) in [
            ("true", JsonValueKind::Bool),
            ("false", JsonValueKind::Bool),
            ("null", JsonValueKind::Null),
        ] {
            if self.input[self.pos..].starts_with(literal) {
                self.pos += literal.len();
                return Ok((literal.to_string(), kind));
            }
        }

        Err(self.error("expected `true`, `false`, or `null`"))
    }

    fn read_number(&mut self) -> Result<String, JsonError> {
        let start = self.pos;

        if self.peek() == Some(b'-') {
            self.pos += 1;
        }

        // A leading zero may not be followed by more digits, so `01` is caught
        // here rather than silently read as `1`.
        let leading_zero = self.peek() == Some(b'0');
        let digits_before = self.skip_digits();
        if digits_before == 0 {
            return Err(self.error("expected a digit"));
        }
        if leading_zero && digits_before > 1 {
            return Err(self.error("number has a leading zero"));
        }

        if self.peek() == Some(b'.') {
            self.pos += 1;
            if self.skip_digits() == 0 {
                return Err(self.error("expected a digit after the decimal point"));
            }
        }

        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if self.skip_digits() == 0 {
                return Err(self.error("expected a digit in the exponent"));
            }
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn skip_digits(&mut self) -> usize {
        let start = self.pos;
        while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
            self.pos += 1;
        }
        self.pos - start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_paths_and_spans() {
        let input = r#"{"data":[{"event_name":"Purchase"}]}"#;
        let document = JsonDocument::parse(input).expect("parses");

        let field = document
            .get("data[0].event_name")
            .expect("field is present");
        assert_eq!(field.text, "Purchase");
        assert_eq!(field.kind, JsonValueKind::String);
        assert_eq!(&input[field.start..field.end], "\"Purchase\"");
    }

    #[test]
    fn records_container_lengths() {
        let document = JsonDocument::parse(r#"{"data":[1,2,3],"user":{"a":1}}"#).expect("parses");

        assert_eq!(document.get("data").expect("array").len, Some(3));
        assert_eq!(document.get("user").expect("object").len, Some(1));
    }

    #[test]
    fn expands_one_path_per_element() {
        let document =
            JsonDocument::parse(r#"{"data":[{"a":1},{"a":2},{"b":3}]}"#).expect("parses");

        assert_eq!(
            document.expand("data[].a"),
            vec!["data[0].a", "data[1].a", "data[2].a"]
        );
    }

    #[test]
    fn expansion_stops_at_a_missing_container() {
        let document = JsonDocument::parse(r#"{"data":[{"a":1}]}"#).expect("parses");

        // `user_data` is not there, so nothing underneath it is addressable and
        // the caller is spared a complaint per leaf.
        assert!(document.expand("data[].user_data.em").is_empty());
        assert!(document.expand("missing[].a").is_empty());
    }

    #[test]
    fn expansion_keeps_a_missing_leaf() {
        let document = JsonDocument::parse(r#"{"data":[{"a":1}]}"#).expect("parses");

        assert_eq!(
            document.expand("data[].event_name"),
            vec!["data[0].event_name"]
        );
        assert!(!document.contains("data[0].event_name"));
    }

    #[test]
    fn expands_nested_subscripts() {
        let document =
            JsonDocument::parse(r#"{"data":[{"em":["a","b"]},{"em":["c"]}]}"#).expect("parses");

        assert_eq!(
            document.expand("data[].em[]"),
            vec!["data[0].em[0]", "data[0].em[1]", "data[1].em[0]"]
        );
    }

    #[test]
    fn matches_pattern_needs_a_present_path() {
        let document = JsonDocument::parse(r#"{"data":[{"event_name":"x"}]}"#).expect("parses");

        assert!(document.matches_pattern("data[].event_name"));
        assert!(!document.matches_pattern("data[].event_time"));
        assert!(!document.matches_pattern("conversion"));
    }

    #[test]
    fn unescapes_strings() {
        let document = JsonDocument::parse(r#"{"a":"line\nbreak A 😀"}"#).expect("parses");

        assert_eq!(document.get("a").expect("field").text, "line\nbreak A 😀");
    }

    #[test]
    fn keeps_spans_correct_after_escapes() {
        let input = r#"{"a":"x\ty","b":"z"}"#;
        let document = JsonDocument::parse(input).expect("parses");
        let field = document.get("b").expect("field");

        assert_eq!(&input[field.start..field.end], "\"z\"");
    }

    #[test]
    fn keeps_spans_correct_after_multibyte_text() {
        let input = r#"{"a":"héllo 😀","b":"z"}"#;
        let document = JsonDocument::parse(input).expect("parses");
        let field = document.get("b").expect("field");

        assert_eq!(&input[field.start..field.end], "\"z\"");
    }

    #[test]
    fn reads_numbers_and_literals() {
        let document =
            JsonDocument::parse(r#"{"a":-1.5e3,"b":true,"c":null,"d":0}"#).expect("parses");

        assert_eq!(document.get("a").expect("number").text, "-1.5e3");
        assert_eq!(document.get("b").expect("bool").kind, JsonValueKind::Bool);
        assert_eq!(document.get("c").expect("null").kind, JsonValueKind::Null);
        assert_eq!(document.get("d").expect("zero").text, "0");
    }

    #[test]
    fn rejects_malformed_input() {
        for input in [
            "{",
            "{\"a\":}",
            "{\"a\":1,}",
            "[1 2]",
            "{\"a\":01}",
            "{\"a\":\"unterminated}",
            "{}trailing",
            "{\"a\":tru}",
        ] {
            assert!(
                JsonDocument::parse(input).is_err(),
                "expected `{input}` to be rejected"
            );
        }
    }

    #[test]
    fn rejects_runaway_nesting() {
        let input = "[".repeat(MAX_DEPTH + 5);
        let error = JsonDocument::parse(&input).expect_err("is rejected");

        assert_eq!(error.message, "nesting is too deep");
    }

    #[test]
    fn blank_covers_null_and_empty_containers() {
        let document = JsonDocument::parse(r#"{"a":null,"b":[],"c":{},"d":"","e":" ","f":"x"}"#)
            .expect("parses");

        for path in ["a", "b", "c", "d", "e"] {
            assert!(document.get(path).expect("field").is_blank(), "{path}");
        }
        assert!(!document.get("f").expect("field").is_blank());
    }

    #[test]
    fn nearest_ancestor_walks_up_both_step_kinds() {
        let document = JsonDocument::parse(r#"{"data":[{"a":1}]}"#).expect("parses");

        let ancestor = document
            .nearest_present_ancestor("data[0].user_data.em")
            .expect("ancestor");
        assert_eq!(ancestor.path, "data[0]");
    }

    #[test]
    fn looks_like_json_only_for_containers() {
        assert!(JsonDocument::looks_like_json("  {\"a\":1}"));
        assert!(JsonDocument::looks_like_json("[1]"));
        assert!(!JsonDocument::looks_like_json("https://example.com/?a=1"));
    }
}
