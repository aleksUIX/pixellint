//! Parse an artifact once per `Engine::validate` call.
//!
//! Plugin selection and the packs that run used to re-parse the same URL or
//! JSON body. This type holds those parses so the hot path can reuse them.

use std::cell::OnceCell;

use crate::json::{JsonDocument, JsonError};
use crate::{ArtifactKind, ValidationRequest, detect_macro_spans, sanitize_macro_spans};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactUrl {
    pub(crate) host: Option<String>,
    pub(crate) path: String,
}

/// Parsed view of one artifact, shared across plugin selection and validation.
pub struct PreparedArtifact<'a> {
    request: &'a ValidationRequest,
    trimmed: &'a str,
    url: OnceCell<Option<ArtifactUrl>>,
    json: OnceCell<Result<JsonDocument, JsonError>>,
}

impl<'a> PreparedArtifact<'a> {
    pub fn from_request(request: &'a ValidationRequest) -> Self {
        Self {
            request,
            trimmed: request.artifact.trim(),
            url: OnceCell::new(),
            json: OnceCell::new(),
        }
    }

    pub fn request(&self) -> &ValidationRequest {
        self.request
    }

    pub(crate) fn trimmed(&self) -> &str {
        self.trimmed
    }

    pub(crate) fn wants_json(&self) -> bool {
        self.request.artifact_kind == ArtifactKind::JsonPayload
            || (self.request.artifact_kind == ArtifactKind::Unknown
                && JsonDocument::looks_like_json(self.trimmed))
    }

    pub(crate) fn url(&self) -> Option<&ArtifactUrl> {
        self.url
            .get_or_init(|| parse_artifact_url(self.trimmed))
            .as_ref()
    }

    pub(crate) fn json(&self) -> Option<&Result<JsonDocument, JsonError>> {
        if !self.wants_json() {
            return None;
        }
        Some(self.json.get_or_init(|| JsonDocument::parse(self.trimmed)))
    }
}

/// Host suffix match that respects DNS label boundaries.
///
/// `example.com` matches `example.com` and `a.example.com`, not `notexample.com`.
/// Both sides are assumed already lowercased.
pub(crate) fn host_matches_suffix(host: &str, suffix: &str) -> bool {
    if host == suffix {
        return true;
    }
    let Some(dot) = host.len().checked_sub(suffix.len() + 1) else {
        return false;
    };
    host.as_bytes().get(dot) == Some(&b'.') && host[dot + 1..] == *suffix
}

/// Parses an artifact URL with macros neutralized, so a templated URL still
/// resolves to a host and path.
pub(crate) fn parse_artifact_url(artifact: &str) -> Option<ArtifactUrl> {
    let spans = detect_macro_spans(artifact);
    if spans.is_empty() {
        scan_url(artifact)
    } else {
        scan_url(&sanitize_macro_spans(artifact, &spans))
    }
}

fn scan_url(artifact: &str) -> Option<ArtifactUrl> {
    let scheme_end = artifact.find("://")?;
    if scheme_end == 0 {
        return None;
    }
    if !artifact.as_bytes()[..scheme_end]
        .iter()
        .all(|&byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'-' || byte == b'.')
    {
        return None;
    }

    let bytes = artifact.as_bytes();
    let mut index = scheme_end + 3;
    if index > artifact.len() {
        return None;
    }
    if index == artifact.len() {
        return Some(ArtifactUrl {
            host: None,
            path: "/".to_string(),
        });
    }

    if bytes[index] == b'[' {
        let close = artifact[index..].find(']')?;
        let host_raw = &artifact[index..index + close + 1];
        if !host_raw.is_ascii() {
            return parse_with_url_crate(artifact);
        }
        let host = host_raw.to_ascii_lowercase();
        index += close + 1;
        if index < artifact.len() && bytes[index] == b':' {
            index += 1;
            while index < artifact.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
        }
        return Some(ArtifactUrl {
            host: nonempty_host(host),
            path: path_from(artifact, index),
        });
    }

    let auth_end = artifact[index..]
        .find(['/', '?', '#'])
        .map(|offset| index + offset)
        .unwrap_or(artifact.len());
    let authority = &artifact[index..auth_end];
    let hostport = match authority.rfind('@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };

    if hostport.is_empty() {
        return parse_with_url_crate(artifact);
    }
    if !hostport.is_ascii() {
        return parse_with_url_crate(artifact);
    }

    let host = match hostport.rsplit_once(':') {
        Some((name, port))
            if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            name
        }
        _ => hostport,
    };

    if host.is_empty() {
        return Some(ArtifactUrl {
            host: None,
            path: path_from(artifact, auth_end),
        });
    }
    if host.contains(':') {
        return parse_with_url_crate(artifact);
    }

    Some(ArtifactUrl {
        host: Some(host.to_ascii_lowercase()),
        path: path_from(artifact, auth_end),
    })
}

fn nonempty_host(host: String) -> Option<String> {
    if host.is_empty() { None } else { Some(host) }
}

fn path_from(artifact: &str, auth_end: usize) -> String {
    if auth_end >= artifact.len() {
        return "/".to_string();
    }
    match artifact.as_bytes()[auth_end] {
        b'/' => {
            let rest = &artifact[auth_end..];
            let end = rest.find(['?', '#']).unwrap_or(rest.len());
            rest[..end].to_string()
        }
        _ => "/".to_string(),
    }
}

fn parse_with_url_crate(artifact: &str) -> Option<ArtifactUrl> {
    let parsed = url::Url::parse(artifact).ok()?;
    Some(ArtifactUrl {
        host: parsed.host_str().map(str::to_string),
        path: parsed.path().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExpansionState, ValidationRequest};

    fn crate_url(artifact: &str) -> Option<(Option<String>, String)> {
        let spans = detect_macro_spans(artifact);
        let sanitized = if spans.is_empty() {
            artifact.to_string()
        } else {
            sanitize_macro_spans(artifact, &spans)
        };
        let parsed = url::Url::parse(&sanitized).ok()?;
        Some((
            parsed.host_str().map(str::to_string),
            parsed.path().to_string(),
        ))
    }

    #[test]
    fn scan_url_matches_the_url_crate_on_pixel_shapes() {
        let cases = [
            "https://www.facebook.com/tr?id=1",
            "https://WWW.FACEBOOK.COM/tr?id=1",
            "https://www.facebook.com:443/tr",
            "https://user:pass@www.facebook.com/tr?id=1",
            "https://example.com",
            "https://example.com?q=1",
            "https://example.com#frag",
            "https://[2001:db8::1]/pixel?id=1",
            "https://[2001:db8::1]:8443/pixel",
            "https:///pixel?id=1",
            "not a url",
            "https://ad.doubleclick.net/ddm/activity/src=1;type=x",
            "http://example.com/path%20x",
            "https://127.0.0.1/pixel",
            "https://localhost:8443/pixel?id=1",
            r#"<script src="https://www.facebook.com/tr?id=1"></script>"#,
            "HTTPS://example.com/tr",
            "https://www.facebook.com/tr?id=[PIXEL_ID]&ev=Purchase",
            "https://example.com/pixel?cb=${CACHEBUSTING}",
            "ftp://example.com/pixel?id=1",
        ];

        for artifact in cases {
            let cheap = parse_artifact_url(artifact).map(|parsed| (parsed.host, parsed.path));
            assert_eq!(cheap, crate_url(artifact), "{artifact}");
        }
    }

    #[test]
    fn host_suffix_respects_label_boundaries() {
        assert!(host_matches_suffix("example.com", "example.com"));
        assert!(host_matches_suffix("a.example.com", "example.com"));
        assert!(!host_matches_suffix("notexample.com", "example.com"));
        assert!(!host_matches_suffix("example.com.evil.test", "example.com"));
        assert!(!host_matches_suffix("com", "example.com"));
    }

    #[test]
    fn prepared_json_parses_once() {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::JsonPayload,
            artifact: r#"{"event":"x"}"#.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let prepared = PreparedArtifact::from_request(&request);
        let first = prepared.json().expect("json");
        let second = prepared.json().expect("json");
        assert!(first.is_ok());
        assert!(std::ptr::eq(first, second));
    }
}
