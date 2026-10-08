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

use std::borrow::Cow;
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
pub(crate) struct JsonField<'a> {
    pub(crate) kind: JsonValueKind,
    /// Scalar values as text: strings unescaped, numbers and literals verbatim.
    /// Containers carry an empty string. Unescaped strings, numbers, and
    /// `true` / `false` / `null` borrow the artifact bytes.
    pub(crate) text: Cow<'a, str>,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// Element count for arrays, member count for objects.
    pub(crate) len: Option<usize>,
    /// Immediate object keys, including punctuation in the original names.
    pub(crate) member_names: Vec<String>,
    /// Distance from the document root. Used with spans for subtree limits.
    pub(crate) depth: usize,
}

impl JsonField<'_> {
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
#[derive(Debug, Clone)]
pub(crate) struct JsonDocument<'a> {
    input: &'a str,
    fields: BTreeMap<String, JsonField<'a>>,
}

impl<'a> JsonDocument<'a> {
    pub(crate) fn artifact(&self) -> &'a str {
        self.input
    }
    pub(crate) fn parse(input: &'a str) -> Result<Self, JsonError> {
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
            input,
            fields: reader.fields,
        })
    }

    /// Whether the text looks like it is meant to be a JSON document. Used to
    /// route an artifact whose kind the caller did not state.
    pub(crate) fn looks_like_json(input: &str) -> bool {
        let trimmed = input.trim_start();
        trimmed.starts_with('{') || trimmed.starts_with('[')
    }

    pub(crate) fn get(&self, path: &str) -> Option<&JsonField<'a>> {
        self.fields.get(path)
    }

    pub(crate) fn contains(&self, path: &str) -> bool {
        self.fields.contains_key(path)
    }

    /// Immediate object members as their original names and concrete paths.
    pub(crate) fn members(&self, parent: &str) -> Vec<(String, String)> {
        let Some(field) = self.get(parent) else {
            return Vec::new();
        };
        if field.kind != JsonValueKind::Object {
            return Vec::new();
        }
        field
            .member_names
            .iter()
            .map(|key| (key.clone(), member_path(parent, key)))
            .collect()
    }

    pub(crate) fn member_name(&self, path: &str) -> Option<&str> {
        let parent = parent_path(path)?;
        self.get(parent)?
            .member_names
            .iter()
            .find(|name| member_path(parent, name) == path)
            .map(String::as_str)
    }

    /// Maximum container depth below a value. The selected container counts
    /// as one level; a scalar has depth zero.
    pub(crate) fn subtree_depth(&self, path: &str) -> Option<usize> {
        let root = self.get(path)?;
        if !matches!(root.kind, JsonValueKind::Object | JsonValueKind::Array) {
            return Some(0);
        }
        Some(
            self.fields
                .values()
                .filter(|field| {
                    field.start >= root.start
                        && field.end <= root.end
                        && matches!(field.kind, JsonValueKind::Object | JsonValueKind::Array)
                })
                .map(|field| field.depth - root.depth + 1)
                .max()
                .unwrap_or(1),
        )
    }

    /// Number of immediate values in an object, counting an array member by
    /// its element count and each other member as one value.
    pub(crate) fn member_value_count(&self, path: &str) -> Option<usize> {
        let root = self.get(path)?;
        if root.kind != JsonValueKind::Object {
            return None;
        }
        Some(
            self.fields
                .values()
                .filter(|field| {
                    field.depth == root.depth + 1
                        && field.start >= root.start
                        && field.end <= root.end
                })
                .map(|field| {
                    if field.kind == JsonValueKind::Array {
                        field.len.unwrap_or(0)
                    } else {
                        1
                    }
                })
                .sum(),
        )
    }

    /// The span of the nearest ancestor that is present, so a violation about a
    /// missing field can still point somewhere useful.
    pub(crate) fn nearest_present_ancestor<'p>(
        &self,
        path: &'p str,
    ) -> Option<(&'p str, &JsonField<'a>)> {
        let mut candidate = path;

        while let Some(parent) = parent_path(candidate) {
            if let Some(field) = self.fields.get(parent) {
                return Some((parent, field));
            }
            candidate = parent;
        }

        self.fields.get("").map(|field| ("", field))
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
        if pattern.is_empty() {
            return vec![String::new()];
        }
        let steps = parse_pattern(pattern);
        if steps.is_empty() {
            return Vec::new();
        }

        let last = steps.len() - 1;
        let mut prefixes = vec![String::new()];

        for (index, step) in steps.iter().enumerate() {
            let mut next = Vec::new();
            for prefix in &prefixes {
                match step {
                    PatternStep::Member(key) => {
                        if self
                            .get(prefix)
                            .is_none_or(|field| field.kind != JsonValueKind::Object)
                        {
                            continue;
                        }
                        let path = member_path(prefix, key);
                        // Keep a missing final leaf for a presence contract.
                        if index == last || self.contains(&path) {
                            next.push(path);
                        }
                    }
                    PatternStep::EveryMember => {
                        next.extend(self.members(prefix).into_iter().map(|(_, path)| path));
                    }
                    PatternStep::EveryDescendant => {
                        if let Some(root) = self.get(prefix) {
                            next.extend(
                                self.fields
                                    .iter()
                                    .filter(|(_, field)| {
                                        field.depth > root.depth
                                            && field.start >= root.start
                                            && field.end <= root.end
                                    })
                                    .map(|(path, _)| path.clone()),
                            );
                        }
                    }
                    PatternStep::EveryElement => {
                        if let Some(len) = self.array_len(prefix) {
                            next.extend((0..len).map(|element| format!("{prefix}[{element}]")));
                        }
                    }
                    PatternStep::Element(element) => {
                        if let Some(len) = self.array_len(prefix)
                            && (*element < len || index == last)
                        {
                            next.push(format!("{prefix}[{element}]"));
                        }
                    }
                }
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

/// Captures identify one logical wildcard slot across alternate field names.
/// Literal members are compared but omitted from the slot identity.
pub(crate) fn alias_slot_key(pattern: &str, path: &str) -> Option<Vec<String>> {
    let expected = parse_pattern(pattern);
    let actual = parse_pattern(path);
    if expected.len() != actual.len() {
        return None;
    }
    let mut captures = Vec::new();
    for (expected, actual) in expected.iter().zip(actual.iter()) {
        match (expected, actual) {
            (PatternStep::Member(left), PatternStep::Member(right)) if left == right => {}
            (PatternStep::Element(left), PatternStep::Element(right)) if left == right => {}
            (PatternStep::EveryElement, PatternStep::Element(index)) => {
                captures.push(format!("[{index}]"))
            }
            (PatternStep::EveryMember, PatternStep::Member(key)) => captures.push(key.clone()),
            _ => return None,
        }
    }
    Some(captures)
}

/// One object member or array selection in a path pattern.
enum PatternStep {
    Member(String),
    EveryMember,
    EveryDescendant,
    EveryElement,
    Element(usize),
}

fn parse_pattern(pattern: &str) -> Vec<PatternStep> {
    let mut steps = Vec::new();
    let bytes = pattern.as_bytes();
    let mut pos = 0;
    let mut needs_member = true;
    while pos < bytes.len() {
        match bytes[pos] {
            b'.' => {
                if needs_member {
                    return Vec::new();
                }
                needs_member = true;
                pos += 1;
            }
            b'[' => {
                pos += 1;
                if bytes.get(pos) == Some(&b'"') {
                    let start = pos;
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != b'"' {
                        if bytes[pos] == b'\\' {
                            pos += 1;
                        }
                        pos += 1;
                    }
                    if pos >= bytes.len() {
                        return Vec::new();
                    }
                    pos += 1;
                    let Ok(key) = serde_json::from_str::<String>(&pattern[start..pos]) else {
                        return Vec::new();
                    };
                    if bytes.get(pos) != Some(&b']') {
                        return Vec::new();
                    }
                    steps.push(PatternStep::Member(key));
                    pos += 1;
                } else {
                    let start = pos;
                    while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
                        pos += 1;
                    }
                    if bytes.get(pos) != Some(&b']') {
                        return Vec::new();
                    }
                    let selector = &pattern[start..pos];
                    steps.push(if selector.is_empty() {
                        PatternStep::EveryElement
                    } else {
                        let Ok(element) = selector.parse() else {
                            return Vec::new();
                        };
                        PatternStep::Element(element)
                    });
                    pos += 1;
                }
                needs_member = false;
            }
            b']' => return Vec::new(),
            _ => {
                if !needs_member {
                    return Vec::new();
                }
                let start = pos;
                while pos < bytes.len() && !matches!(bytes[pos], b'.' | b'[' | b']') {
                    pos += 1;
                }
                let key = &pattern[start..pos];
                steps.push(if key == "**" {
                    PatternStep::EveryDescendant
                } else if key == "*" {
                    PatternStep::EveryMember
                } else {
                    PatternStep::Member(key.to_string())
                });
                needs_member = false;
            }
        }
    }
    if needs_member {
        return Vec::new();
    }
    steps
}

/// A concrete member path, quoting names that have path syntax of their own.
pub(crate) fn member_path(prefix: &str, key: &str) -> String {
    if key.is_empty() || key.contains(['.', '[', ']', '*', '"', '\\']) {
        format!(
            "{prefix}[{}]",
            serde_json::to_string(key).expect("string serializes")
        )
    } else if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// The path one level up, whether the last step was a key or a subscript.
pub(crate) fn parent_path(path: &str) -> Option<&str> {
    if path.is_empty() {
        return None;
    }

    let bytes = path.as_bytes();
    let mut quoted = false;
    let mut pos = 0;
    let mut last = None;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' if quoted => pos += 1,
            b'"' => quoted = !quoted,
            b'.' | b'[' if !quoted => last = Some(pos),
            _ => {}
        }
        pos += 1;
    }
    Some(last.map_or("", |index| &path[..index]))
}

struct Reader<'a> {
    bytes: &'a [u8],
    input: &'a str,
    pos: usize,
    fields: BTreeMap<String, JsonField<'a>>,
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
        text: Cow<'a, str>,
        span: (usize, usize),
        len: Option<usize>,
        depth: usize,
    ) {
        let (start, end) = span;
        self.fields.insert(
            path,
            JsonField {
                kind,
                text,
                start,
                end,
                len,
                member_names: Vec::new(),
                depth,
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
                self.record(
                    path,
                    JsonValueKind::String,
                    text,
                    (start, self.pos),
                    None,
                    depth,
                );
                Ok(())
            }
            Some(b't') | Some(b'f') | Some(b'n') => {
                let start = self.pos;
                let (literal, kind) = self.read_literal()?;
                self.record(path, kind, literal, (start, self.pos), None, depth);
                Ok(())
            }
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => {
                let start = self.pos;
                let number = self.read_number()?;
                self.record(
                    path,
                    JsonValueKind::Number,
                    number,
                    (start, self.pos),
                    None,
                    depth,
                );
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
        let mut member_names = Vec::new();

        if self.peek() == Some(b'}') {
            self.pos += 1;
            self.record(
                path,
                JsonValueKind::Object,
                Cow::Borrowed(""),
                (start, self.pos),
                Some(0),
                depth,
            );
            return Ok(());
        }

        loop {
            self.skip_whitespace();
            let key = self.read_string()?;
            member_names.push(key.to_string());
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            self.read_value(member_path(&path, key.as_ref()), depth + 1)?;
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
            path.clone(),
            JsonValueKind::Object,
            Cow::Borrowed(""),
            (start, self.pos),
            Some(members),
            depth,
        );
        if let Some(field) = self.fields.get_mut(&path) {
            field.member_names = member_names;
        }
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
                Cow::Borrowed(""),
                (start, self.pos),
                Some(0),
                depth,
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
            Cow::Borrowed(""),
            (start, self.pos),
            Some(elements),
            depth,
        );
        Ok(())
    }

    /// Reads a string. Unescaped content borrows the artifact; escapes copy.
    fn read_string(&mut self) -> Result<Cow<'a, str>, JsonError> {
        self.expect(b'"')?;
        let content_start = self.pos;

        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("unterminated string"));
            };

            match byte {
                b'"' => {
                    let borrowed = &self.input[content_start..self.pos];
                    self.pos += 1;
                    return Ok(Cow::Borrowed(borrowed));
                }
                b'\\' => return self.read_string_escaped(content_start),
                0x00..=0x1f => return Err(self.error("control character in string")),
                _ => self.pos += 1,
            }
        }
    }

    fn read_string_escaped(&mut self, content_start: usize) -> Result<Cow<'a, str>, JsonError> {
        let mut out = String::from(&self.input[content_start..self.pos]);
        let mut literal_start = self.pos;

        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("unterminated string"));
            };

            match byte {
                b'"' => {
                    out.push_str(&self.input[literal_start..self.pos]);
                    self.pos += 1;
                    return Ok(Cow::Owned(out));
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

        let digits = &self.bytes[self.pos..self.pos + 4];
        let mut value = 0;
        for digit in digits {
            let digit = match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                b'A'..=b'F' => digit - b'A' + 10,
                _ => return Err(self.error("invalid unicode escape")),
            };
            value = value * 16 + u32::from(digit);
        }
        self.pos += 4;
        Ok(value)
    }

    fn read_literal(&mut self) -> Result<(Cow<'a, str>, JsonValueKind), JsonError> {
        for (literal, kind) in [
            ("true", JsonValueKind::Bool),
            ("false", JsonValueKind::Bool),
            ("null", JsonValueKind::Null),
        ] {
            if self.input[self.pos..].starts_with(literal) {
                self.pos += literal.len();
                return Ok((Cow::Borrowed(literal), kind));
            }
        }

        Err(self.error("expected `true`, `false`, or `null`"))
    }

    fn read_number(&mut self) -> Result<Cow<'a, str>, JsonError> {
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

        Ok(Cow::Borrowed(&self.input[start..self.pos]))
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
        assert!(matches!(
            document.get("a").expect("field").text,
            Cow::Owned(_)
        ));
    }

    #[test]
    fn unescaped_scalars_borrow_the_artifact() {
        let input = r#"{"a":"Purchase","b":4200,"c":true}"#;
        let document = JsonDocument::parse(input).expect("parses");

        assert!(matches!(
            document.get("a").expect("string").text,
            Cow::Borrowed("Purchase")
        ));
        assert!(matches!(
            document.get("b").expect("number").text,
            Cow::Borrowed("4200")
        ));
        assert!(matches!(
            document.get("c").expect("bool").text,
            Cow::Borrowed("true")
        ));
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
        assert_eq!(ancestor.0, "data[0]");
    }

    #[test]
    fn a_bare_root_array_is_addressable() {
        let document = JsonDocument::parse(r#"[{"event":"a"},{"event":"b"}]"#).expect("parses");

        assert_eq!(document.expand("[].event"), vec!["[0].event", "[1].event"]);
        assert_eq!(document.get("[1].event").expect("field").text, "b");
        assert!(document.matches_pattern("[].event"));
    }

    #[test]
    fn a_root_array_pattern_finds_nothing_in_an_object() {
        let document = JsonDocument::parse(r#"{"event":"a"}"#).expect("parses");

        assert!(document.expand("[].event").is_empty());
        assert!(!document.matches_pattern("[].event"));
    }

    #[test]
    fn empty_path_tokens_and_malformed_selectors_are_rejected() {
        for pattern in [
            "a..b",
            ".a",
            "a.",
            "data[",
            "data]",
            "data[-1]",
            "data[no]",
            "data[1]tail",
            "data[[1]]",
            "data[999999999999999999999999999999999999999]",
        ] {
            assert!(
                !is_valid_pattern(pattern),
                "expected `{pattern}` to be rejected"
            );
        }
    }

    #[test]
    fn tuple_positions_expand_under_root_and_scoped_arrays() {
        let document =
            JsonDocument::parse(r#"[["sku","name",2],["second","other",3]]"#).expect("parses");
        assert_eq!(document.expand("[][0]"), vec!["[0][0]", "[1][0]"]);
        assert_eq!(document.expand("[]. [0]"), Vec::<String>::new());
        assert_eq!(document.expand("[0].[1]"), vec!["[0][1]"]);
        assert_eq!(document.expand("[1][2]"), vec!["[1][2]"]);
        assert_eq!(document.get("[1][2]").expect("tuple value").text, "3");
    }

    #[test]
    fn fixed_and_wildcard_selectors_can_be_combined() {
        let document =
            JsonDocument::parse(r#"{"rows":[{"values":[[1,2],[3,4]]},{"values":[[5,6]]}]}"#)
                .expect("parses");
        assert_eq!(
            document.expand("rows[].values[][1]"),
            vec![
                "rows[0].values[0][1]",
                "rows[0].values[1][1]",
                "rows[1].values[0][1]"
            ]
        );
        assert_eq!(
            document.expand("rows[0].values[1][0]"),
            vec!["rows[0].values[1][0]"]
        );
        assert_eq!(
            document.expand("rows[0].values.[]. [0]"),
            Vec::<String>::new()
        );
        assert_eq!(
            document.expand("rows[0].values.[].[0]"),
            vec!["rows[0].values[0][0]", "rows[0].values[1][0]"]
        );
    }

    #[test]
    fn missing_final_tuple_positions_remain_addressable() {
        let document = JsonDocument::parse(r#"{"items":[["a"],[],["b",2]]}"#).expect("parses");
        assert_eq!(
            document.expand("items[][1]"),
            vec!["items[0][1]", "items[1][1]", "items[2][1]"]
        );
        assert!(!document.contains("items[0][1]"));
        assert!(document.expand("items[9][0]").is_empty());
        assert!(document.expand("items[][9].leaf").is_empty());
        assert!(!document.matches_pattern("items[9]"));
        assert_eq!(
            document.nearest_present_ancestor("items[1][0]").unwrap().0,
            "items[1]"
        );
    }

    #[test]
    fn indexed_selectors_do_not_address_object_keys_or_scalars() {
        let document = JsonDocument::parse(r#"{"tuple":{"0":1},"scalar":3}"#).expect("parses");
        assert!(document.expand("tuple[0]").is_empty());
        assert!(document.expand("scalar[0]").is_empty());
        assert!(document.expand("[0]").is_empty());
    }

    #[test]
    fn wildcard_object_scopes_support_tuple_positions() {
        let document = JsonDocument::parse(r#"{"items":{"a":[1,2],"b":[3,4]}}"#).expect("parses");
        assert_eq!(
            document.expand("items.*[1]"),
            vec!["items.a[1]", "items.b[1]"]
        );
    }

    #[test]
    fn canonical_quoted_members_preserve_literal_punctuation_and_empty_names() {
        let input = r#"{"data":{"a.b":42,"a":{"b":"ok"},"":false,"[0]":[1],"*":3,"a\"b\\c":4}}"#;
        let document = JsonDocument::parse(input).expect("parses");
        for (key, path, text) in [
            ("a.b", r#"data["a.b"]"#, "42"),
            ("", r#"data[""]"#, "false"),
            ("*", r#"data["*"]"#, "3"),
            ("a\"b\\c", r#"data["a\"b\\c"]"#, "4"),
        ] {
            assert_eq!(member_path("data", key), path);
            assert_eq!(document.expand(path), vec![path]);
            let field = document.get(path).expect("literal member");
            assert_eq!(field.text, text);
            assert_eq!(&input[field.start..field.end], text);
        }
        assert_eq!(document.get("data.a.b").expect("nested member").text, "ok");
        assert_eq!(
            document.expand(r#"data["[0]"][0]"#),
            vec![r#"data["[0]"][0]"#]
        );
        assert_eq!(document.members("data").len(), 6);
        assert!(document.members(r#"data["a.b"]"#).is_empty());
    }

    #[test]
    fn wildcard_expansion_uses_concrete_quoted_member_paths() {
        let document =
            JsonDocument::parse(r#"{"data":{"a.b":{"value":1},"":{"value":2},"*":{"value":3}}}"#)
                .expect("parses");
        assert_eq!(
            document.expand("data.*.value"),
            vec![
                r#"data["a.b"].value"#,
                r#"data[""].value"#,
                r#"data["*"].value"#
            ]
        );
        assert_eq!(
            document.expand(r#"data["*"].value"#),
            vec![r#"data["*"].value"#]
        );
    }

    #[test]
    fn quoted_member_parent_lookup_ignores_delimiters_inside_names() {
        let document = JsonDocument::parse(r#"{"data":{"a[b.c]":{"":1}}}"#).expect("parses");
        let ancestor = document
            .nearest_present_ancestor(r#"data["a[b.c]"][""].missing"#)
            .expect("ancestor");
        assert_eq!(ancestor.0, r#"data["a[b.c]"][""]"#);
        assert_eq!(parent_path(r#"data["a[b.c]"]"#), Some("data"));
        assert_eq!(parent_path(r#"["a[b.c]"]"#), Some(""));
    }

    #[test]
    fn malformed_quoted_path_names_are_rejected() {
        for path in [
            r#"data["unterminated]"#,
            r#"data["a""#,
            r#"data["a"]tail"#,
            r#"data["\q"]"#,
            r#"data["\u12"]"#,
        ] {
            assert!(!is_valid_pattern(path), "{path}");
        }
    }

    #[test]
    fn malformed_unicode_escapes_return_errors_without_slicing_multibyte_text() {
        for input in [r#"{"a":"\ué😀"}"#, r#"{"a":"\u😀é"}"#, r#"{"a":"\u0é0"}"#] {
            assert!(JsonDocument::parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn subtree_depth_counts_containers_and_ignores_neighbors() {
        let document = JsonDocument::parse(
            r#"{"selected":{"items":[{"x":[1]}]},"neighbor":[[[[{}]]]],"scalar":1,"empty":[]}"#,
        )
        .expect("parses");
        assert_eq!(document.subtree_depth(""), Some(6));
        assert_eq!(document.subtree_depth("selected"), Some(4));
        assert_eq!(document.subtree_depth("selected.items[0].x"), Some(1));
        assert_eq!(document.subtree_depth("scalar"), Some(0));
        assert_eq!(document.subtree_depth("empty"), Some(1));
        assert_eq!(document.subtree_depth("missing"), None);
    }

    #[test]
    fn member_value_count_expands_only_immediate_array_members() {
        let document = JsonDocument::parse(
            r#"{"selected":{"scalar":1,"values":[1,2],"empty":[],"object":{"values":[1,2,3]},"nested":[[1,2],[3]]},"neighbor":[1,2,3,4]}"#,
        )
        .expect("parses");
        assert_eq!(document.member_value_count("selected"), Some(6));
        assert_eq!(document.member_value_count("selected.object"), Some(3));
        assert_eq!(document.member_value_count("selected.empty"), None);
        assert_eq!(document.member_value_count("selected.scalar"), None);
        assert_eq!(document.member_value_count("missing"), None);
        assert_eq!(
            JsonDocument::parse("{}").unwrap().member_value_count(""),
            Some(0)
        );
    }

    #[test]
    fn looks_like_json_only_for_containers() {
        assert!(JsonDocument::looks_like_json("  {\"a\":1}"));
        assert!(JsonDocument::looks_like_json("[1]"));
        assert!(!JsonDocument::looks_like_json("https://example.com/?a=1"));
    }
}
