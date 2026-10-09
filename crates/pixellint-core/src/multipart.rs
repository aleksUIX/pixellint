//! Bounded UTF-8 text fields from RFC 7578 multipart/form-data captures.
//! Files and ambiguous encodings remain unavailable rather than guessed.

use std::collections::BTreeMap;

use crate::ViolationTargetComponent;
use crate::manifest::RawParam;

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_PARTS: usize = 1024;
const MAX_HEADERS: usize = 16 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MultipartError {
    Invalid(&'static str),
    Unsupported(&'static str),
}
use MultipartError::{Invalid, Unsupported};

// Quoted parameter values may contain semicolons and escaped quotes.
fn parameters(value: &str) -> Result<(String, BTreeMap<String, String>), MultipartError> {
    let (kind, tail) = value.split_once(';').unwrap_or((value, ""));
    let mut rest = tail.trim();
    let mut params = BTreeMap::new();
    while !rest.is_empty() {
        let (name, after) = rest
            .split_once('=')
            .ok_or(Invalid("Malformed MIME parameter."))?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        {
            return Err(Invalid("Malformed MIME parameter name."));
        }
        rest = after.trim_start();
        let parsed;
        if let Some(quoted) = rest.strip_prefix('"') {
            let mut out = String::new();
            let mut chars = quoted.char_indices();
            let mut end = None;
            while let Some((index, ch)) = chars.next() {
                match ch {
                    '\\' => out.push(
                        chars
                            .next()
                            .ok_or(Invalid("Unterminated MIME quoted parameter."))?
                            .1,
                    ),
                    '"' => {
                        end = Some(index + 1);
                        break;
                    }
                    '\r' | '\n' | '\0' => {
                        return Err(Invalid("Control character in MIME parameter."));
                    }
                    _ => out.push(ch),
                }
            }
            rest = quoted
                .get(end.ok_or(Invalid("Unterminated MIME quoted parameter."))?..)
                .ok_or(Invalid("Malformed MIME parameter."))?
                .trim_start();
            parsed = out;
            if !rest.is_empty() {
                rest = rest
                    .strip_prefix(';')
                    .ok_or(Invalid("Malformed MIME quoted parameter suffix."))?
                    .trim_start();
            }
        } else {
            let (part, remaining) = rest.split_once(';').unwrap_or((rest, ""));
            parsed = part.trim().to_string();
            if parsed.is_empty()
                || !parsed
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            {
                return Err(Invalid("Unquoted MIME parameters must use token syntax."));
            }
            rest = remaining.trim_start();
        }
        if params.insert(name, parsed).is_some() {
            return Err(Unsupported(
                "Repeated MIME parameters make the capture ambiguous.",
            ));
        }
    }
    Ok((kind.trim().to_ascii_lowercase(), params))
}

pub(crate) fn decode(
    content_type: &str,
    body: &str,
) -> Result<Vec<RawParam<'static>>, MultipartError> {
    if body.len() > MAX_BYTES {
        return Err(Unsupported(
            "Multipart text exceeds the local 4 MiB decoding limit.",
        ));
    }
    let (kind, params) = parameters(content_type)?;
    if kind != "multipart/form-data" {
        return Err(Unsupported(
            "Only multipart/form-data text fields are decoded locally.",
        ));
    }
    let boundary = params.get("boundary").ok_or(Invalid(
        "Multipart form data requires a boundary parameter.",
    ))?;
    if !crate::detect_macro_spans(boundary).is_empty() {
        return Err(Unsupported(
            "The multipart boundary contains an unresolved macro.",
        ));
    }
    if boundary.is_empty()
        || boundary.len() > 70
        || boundary.ends_with(' ')
        || !boundary
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"'()+_,-./:=? ".contains(&b))
    {
        return Err(Invalid(
            "Multipart boundary must follow RFC 2046 syntax and its 70-character limit.",
        ));
    }
    let marker = format!("--{boundary}");
    let mut delimiters = Vec::new();
    for (start, _) in body.match_indices(&marker) {
        if start != 0 && !body[..start].ends_with("\r\n") {
            continue;
        }
        let suffix = &body[start + marker.len()..];
        let closing = suffix.starts_with("--");
        let suffix = if closing { &suffix[2..] } else { suffix };
        let padding = suffix.len() - suffix.trim_start_matches([' ', '\t']).len();
        let suffix = &suffix[padding..];
        let end = if suffix.starts_with("\r\n") {
            start + marker.len() + usize::from(closing) * 2 + padding + 2
        } else if closing && suffix.is_empty() {
            body.len()
        } else if suffix.starts_with('\n') {
            return Err(Invalid("Multipart boundary lines require CRLF framing."));
        } else {
            return Err(Unsupported(
                "A boundary prefix inside a part makes its framing ambiguous.",
            ));
        };
        delimiters.push((start, end, closing));
        if closing {
            break;
        }
        if delimiters.len() > MAX_PARTS {
            return Err(Unsupported(
                "Multipart text exceeds the local 1024-part limit.",
            ));
        }
    }
    if delimiters.first().is_none_or(|d| d.2) || delimiters.last().is_none_or(|d| !d.2) {
        if crate::detect_macro_spans(body)
            .iter()
            .any(|span| span.start == 0 && span.end == body.len())
        {
            return Err(Unsupported(
                "The multipart body is an unresolved payload macro.",
            ));
        }
        return Err(Invalid(
            "Multipart form data needs opening and closing boundary lines with CRLF framing.",
        ));
    }
    let mut fields = Vec::new();
    let mut default_non_ascii_text = false;
    for pair in delimiters.windows(2) {
        let start = pair[0].1;
        let end = pair[1]
            .0
            .checked_sub(2)
            .filter(|end| *end >= start)
            .ok_or(Invalid("Malformed multipart part framing."))?;
        let part = &body[start..end];
        let header_end = part
            .find("\r\n\r\n")
            .ok_or(Invalid("Multipart parts need a header/body separator."))?;
        if header_end > MAX_HEADERS {
            return Err(Unsupported(
                "Multipart part headers exceed the local 16 KiB limit.",
            ));
        }
        let mut headers = BTreeMap::new();
        for line in part[..header_end].split("\r\n") {
            if line.starts_with([' ', '\t']) {
                return Err(Unsupported(
                    "Folded multipart headers are not decoded locally.",
                ));
            }
            let (name, value) = line
                .split_once(':')
                .ok_or(Invalid("Malformed multipart part header."))?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            {
                return Err(Invalid("Multipart header names must use token syntax."));
            }
            if value
                .bytes()
                .any(|b| b == 0 || b == b'\r' || b == b'\n' || (b < 32 && b != b'\t') || b == 127)
            {
                return Err(Invalid("Control character in multipart part header."));
            }
            if headers
                .insert(name.to_ascii_lowercase(), value.trim())
                .is_some()
            {
                return Err(Unsupported(
                    "Repeated multipart part headers make the capture ambiguous.",
                ));
            }
        }
        let disposition = headers.get("content-disposition").ok_or(Invalid(
            "Each multipart form part requires Content-Disposition.",
        ))?;
        let (kind, params) = parameters(disposition)?;
        if kind != "form-data" {
            return Err(Invalid(
                "Multipart form parts require form-data disposition.",
            ));
        }
        let name = params.get("name").ok_or(Invalid(
            "Multipart form disposition requires a name parameter.",
        ))?;
        if params.contains_key("filename")
            || params.contains_key("filename*")
            || params.contains_key("name*")
            || headers.contains_key("content-transfer-encoding")
        {
            return Err(Unsupported(
                "File parts, extended names and transfer encodings are not decoded locally.",
            ));
        }
        let mut default_text_charset = true;
        if let Some(content_type) = headers.get("content-type") {
            let (kind, params) = parameters(content_type)?;
            default_text_charset = kind == "text/plain" && !params.contains_key("charset");
            if !matches!(kind.as_str(), "text/plain" | "application/json")
                || params.get("charset").is_some_and(|v| {
                    !v.eq_ignore_ascii_case("utf-8") && !v.eq_ignore_ascii_case("us-ascii")
                })
            {
                return Err(Unsupported(
                    "Multipart parts need locally supported UTF-8 text media types.",
                ));
            }
            if params
                .get("charset")
                .is_some_and(|v| v.eq_ignore_ascii_case("us-ascii"))
                && !part[header_end + 4..].is_ascii()
            {
                return Err(Unsupported(
                    "The part text does not match its declared US-ASCII charset.",
                ));
            }
        }
        let value = &part[header_end + 4..];
        if value.contains('\0')
            || (name == "_charset_"
                && !value.eq_ignore_ascii_case("utf-8")
                && !value.eq_ignore_ascii_case("us-ascii"))
        {
            return Err(Unsupported(
                "Multipart binary text or alternate default charsets are not decoded locally.",
            ));
        }
        default_non_ascii_text |= default_text_charset && !value.is_ascii();
        let value_start = start + header_end + 4;
        let mut field = RawParam::query(name.clone(), value.to_string(), value_start, end);
        field.component = ViolationTargetComponent::BodyField;
        fields.push(field);
    }
    let defaults: Vec<_> = fields
        .iter()
        .filter(|field| field.name == "_charset_")
        .collect();
    if defaults.len() > 1
        || (default_non_ascii_text
            && defaults
                .first()
                .is_some_and(|field| field.value.eq_ignore_ascii_case("us-ascii")))
    {
        return Err(Unsupported(
            "Repeated default charsets or text outside the declared default charset are unavailable.",
        ));
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn part(name: &str, value: &str) -> String {
        format!("--abc\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
    }
    #[test]
    fn fields_keep_unicode_duplicates_whitespace_and_wire_spans() {
        let body = format!(
            "preamble\r\n{}{}{}--abc--\r\nepilogue",
            part("a", "héllo+%20\r\n"),
            part("a", ""),
            part("i", "{\"user_id\":\"😀\"}")
        );
        let fields = decode("multipart/form-data; boundary=\"abc\"", &body).unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].value, "héllo+%20\r\n");
        assert_eq!(fields[1].value, "");
        for field in fields {
            assert_eq!(&body[field.start..field.end], field.value);
        }
    }
    #[test]
    fn distinct_delimiter_prefixes_and_quoted_names_are_lossless() {
        let body = "--abc \t\r\nContent-Disposition: form-data; name=\"a;\\\"b\"\r\n\r\none\r\n--ab-not-a-delimiter\r\ntwo\r\n--abc--";
        let f = decode("multipart/form-data; boundary=abc", body).unwrap();
        assert_eq!(f[0].name, "a;\"b");
        assert_eq!(f[0].value, "one\r\n--ab-not-a-delimiter\r\ntwo");
    }
    #[test]
    fn malformed_is_distinct_from_valid_but_unavailable() {
        for (mime, body) in [
            ("multipart/form-data", "opaque"),
            ("multipart/form-data; boundary=abc", "--abc\nno\n--abc--"),
            (
                "multipart/form-data; boundary=abc",
                "--abc\r\n\r\ntext\r\n--abc--",
            ),
        ] {
            assert!(matches!(decode(mime, body), Err(Invalid(_))));
        }
        for header in [
            "Content-Disposition: form-data; name=\"file\"; filename=\"x.txt\"",
            "Content-Disposition: form-data; name=\"a\"\r\nContent-Type: text/plain; charset=iso-8859-1",
            "Content-Disposition: form-data; name=\"a\"\r\nContent-Transfer-Encoding: base64",
        ] {
            let body = format!("--abc\r\n{header}\r\n\r\ntext\r\n--abc--");
            assert!(matches!(
                decode("multipart/form-data; boundary=abc", &body),
                Err(Unsupported(_))
            ));
        }
        assert!(matches!(
            decode(
                "multipart/form-data; boundary=abc",
                &"x".repeat(MAX_BYTES + 1)
            ),
            Err(Unsupported(_))
        ));
        let body = format!("{}--abc--", part("a", "x").repeat(1025));
        assert!(matches!(
            decode("multipart/form-data; boundary=abc", &body),
            Err(Unsupported(_))
        ));
    }
}
