//! Parse an artifact once per `Engine::validate` call.
//!
//! Plugin selection and the packs that run used to re-parse the same URL or
//! JSON body. This type holds those parses so the hot path can reuse them.

use std::borrow::Cow;
use std::cell::OnceCell;

use crate::json::{JsonDocument, JsonError};
use crate::manifest::{ParamStyle, RawParam, extract_params};
use crate::{ArtifactKind, MacroSpan, ValidationRequest, detect_macro_spans, sanitize_macro_spans};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactUrl<'a> {
    pub(crate) host: Option<Cow<'a, str>>,
    pub(crate) path: Cow<'a, str>,
    pub(crate) scheme: Cow<'a, str>,
    pub(crate) has_userinfo: bool,
    pub(crate) has_fragment: bool,
    /// True when scheme, host presence, userinfo, and fragment match `Url::parse`.
    pub(crate) core_ready: bool,
}

impl ArtifactUrl<'_> {
    fn into_owned(self) -> ArtifactUrl<'static> {
        ArtifactUrl {
            host: self.host.map(|host| Cow::Owned(host.into_owned())),
            path: Cow::Owned(self.path.into_owned()),
            scheme: Cow::Owned(self.scheme.into_owned()),
            has_userinfo: self.has_userinfo,
            has_fragment: self.has_fragment,
            core_ready: self.core_ready,
        }
    }
}

/// Parsed view of one artifact, shared across plugin selection and validation.
pub struct PreparedArtifact<'a> {
    request: &'a ValidationRequest,
    trimmed: &'a str,
    url: OnceCell<Option<ArtifactUrl<'a>>>,
    json: OnceCell<Result<JsonDocument, JsonError>>,
    macros: OnceCell<Vec<MacroSpan>>,
    params: [OnceCell<Vec<RawParam<'a>>>; 4],
}

impl<'a> PreparedArtifact<'a> {
    pub fn from_request(request: &'a ValidationRequest) -> Self {
        Self {
            request,
            trimmed: request.artifact.trim(),
            url: OnceCell::new(),
            json: OnceCell::new(),
            macros: OnceCell::new(),
            params: Default::default(),
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

    pub(crate) fn macro_spans(&self) -> &[MacroSpan] {
        self.macros.get_or_init(|| detect_macro_spans(self.trimmed))
    }

    pub(crate) fn url(&self) -> Option<&ArtifactUrl<'_>> {
        if self.url.get().is_none() {
            let parsed = {
                let spans = self.macro_spans();
                parse_artifact_url_with_spans(self.trimmed, spans)
            };
            let _ = self.url.set(parsed);
        }
        self.url.get().and_then(Option::as_ref)
    }

    pub(crate) fn json(&self) -> Option<&Result<JsonDocument, JsonError>> {
        if !self.wants_json() {
            return None;
        }
        Some(self.json.get_or_init(|| JsonDocument::parse(self.trimmed)))
    }

    pub(crate) fn params(&self, style: ParamStyle) -> &[RawParam<'_>] {
        self.params[style.cache_index()]
            .get_or_init(|| extract_params(self.trimmed, style))
            .as_slice()
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
#[cfg(test)]
pub(crate) fn parse_artifact_url(artifact: &str) -> Option<ArtifactUrl<'_>> {
    parse_artifact_url_with_spans(artifact, &detect_macro_spans(artifact))
}

fn parse_artifact_url_with_spans<'a>(
    artifact: &'a str,
    spans: &[MacroSpan],
) -> Option<ArtifactUrl<'a>> {
    if spans.is_empty() {
        scan_url(artifact)
    } else {
        scan_url(&sanitize_macro_spans(artifact, spans)).map(ArtifactUrl::into_owned)
    }
}

fn scan_url(artifact: &str) -> Option<ArtifactUrl<'_>> {
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

    let scheme = ascii_lower_cow(&artifact[..scheme_end]);
    let bytes = artifact.as_bytes();
    let mut index = scheme_end + 3;
    if index > artifact.len() {
        return None;
    }

    if is_special_scheme(scheme.as_ref()) {
        let mut skipped = index;
        while skipped < artifact.len() && bytes[skipped] == b'/' {
            skipped += 1;
        }
        if skipped > index {
            if skipped == artifact.len() || matches!(bytes[skipped], b'?' | b'#') {
                return parse_with_url_crate(artifact);
            }
            index = skipped;
        }
    }

    if index == artifact.len() {
        return Some(unready_url(scheme, None, Cow::Borrowed("/"), false, false));
    }

    if bytes[index] == b'[' {
        let close = artifact[index..].find(']')?;
        let host_raw = &artifact[index..index + close + 1];
        if !host_raw.is_ascii() {
            return parse_with_url_crate(artifact);
        }
        let host = ascii_lower_cow(host_raw);
        index += close + 1;
        if index < artifact.len() && bytes[index] == b':' {
            index += 1;
            while index < artifact.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
        }
        let has_fragment = artifact.get(index..).is_some_and(|rest| rest.contains('#'));
        return Some(unready_url(
            scheme,
            nonempty_host(host),
            path_from(artifact, index),
            false,
            has_fragment,
        ));
    }

    let auth_end = artifact[index..]
        .find(['/', '?', '#'])
        .map(|offset| index + offset)
        .unwrap_or(artifact.len());
    let authority = &artifact[index..auth_end];
    let has_userinfo = deprecated_userinfo(authority);
    let has_fragment = artifact[auth_end..].contains('#');
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

    let (host, port) = match hostport.rsplit_once(':') {
        Some((name, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => (name, Some(port)),
        _ => (hostport, None),
    };

    if host.is_empty() {
        return Some(unready_url(
            scheme,
            None,
            path_from(artifact, auth_end),
            has_userinfo,
            has_fragment,
        ));
    }
    if host.contains(':') {
        return parse_with_url_crate(artifact);
    }

    Some(ArtifactUrl {
        host: Some(ascii_lower_cow(host)),
        path: path_from(artifact, auth_end),
        scheme,
        has_userinfo,
        has_fragment,
        core_ready: port_is_core_ready(port),
    })
}

fn is_special_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https" | "ws" | "wss" | "ftp")
}

fn deprecated_userinfo(authority: &str) -> bool {
    let Some(at) = authority.rfind('@') else {
        return false;
    };
    let userinfo = &authority[..at];
    !userinfo.is_empty() && userinfo != ":"
}

fn port_is_core_ready(port: Option<&str>) -> bool {
    match port {
        None | Some("") => true,
        Some(port) => port.parse::<u16>().is_ok(),
    }
}

fn ascii_lower_cow(value: &str) -> Cow<'_, str> {
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(value.to_ascii_lowercase())
    } else {
        Cow::Borrowed(value)
    }
}

fn unready_url<'a>(
    scheme: Cow<'a, str>,
    host: Option<Cow<'a, str>>,
    path: Cow<'a, str>,
    has_userinfo: bool,
    has_fragment: bool,
) -> ArtifactUrl<'a> {
    ArtifactUrl {
        host,
        path,
        scheme,
        has_userinfo,
        has_fragment,
        core_ready: false,
    }
}

fn nonempty_host<'a>(host: Cow<'a, str>) -> Option<Cow<'a, str>> {
    if host.is_empty() { None } else { Some(host) }
}

fn path_from(artifact: &str, auth_end: usize) -> Cow<'_, str> {
    if auth_end >= artifact.len() {
        return Cow::Borrowed("/");
    }
    match artifact.as_bytes()[auth_end] {
        b'/' => {
            let rest = &artifact[auth_end..];
            let end = rest.find(['?', '#']).unwrap_or(rest.len());
            Cow::Borrowed(&rest[..end])
        }
        _ => Cow::Borrowed("/"),
    }
}

fn parse_with_url_crate(artifact: &str) -> Option<ArtifactUrl<'static>> {
    let parsed = url::Url::parse(artifact).ok()?;
    Some(ArtifactUrl {
        host: parsed.host_str().map(|host| Cow::Owned(host.to_string())),
        path: Cow::Owned(parsed.path().to_string()),
        scheme: Cow::Owned(parsed.scheme().to_string()),
        has_userinfo: !parsed.username().is_empty() || parsed.password().is_some(),
        has_fragment: parsed.fragment().is_some(),
        core_ready: true,
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
            "https://example.com:/pixel",
            "https:///example.com/pixel",
            "https://user@example.com/pixel",
            "https://example.com#frag",
            "https://www.google-analytics.com/g/collect?v=2&tid=G-XXX",
        ];

        for artifact in cases {
            let cheap = parse_artifact_url(artifact).map(|parsed| {
                (
                    parsed.host.map(|host| host.into_owned()),
                    parsed.path.into_owned(),
                )
            });
            assert_eq!(cheap, crate_url(artifact), "{artifact}");
        }
    }

    #[test]
    fn typical_pixels_are_core_ready_without_the_url_crate() {
        for artifact in [
            "https://www.facebook.com/tr?id=1",
            "https://example.com:/pixel",
            "https:///example.com/pixel",
            "HTTP://example.com/pixel",
            "https://user:pass@example.com/pixel?id=1#frag",
        ] {
            let parsed = parse_artifact_url(artifact).expect(artifact);
            assert!(parsed.core_ready, "{artifact}");
            let crate_parsed = url::Url::parse(artifact).unwrap();
            assert_eq!(parsed.scheme, crate_parsed.scheme());
            assert_eq!(
                parsed.has_userinfo,
                !crate_parsed.username().is_empty() || crate_parsed.password().is_some(),
                "{artifact}"
            );
            assert_eq!(
                parsed.has_fragment,
                crate_parsed.fragment().is_some(),
                "{artifact}"
            );
        }
        let ipv6 = parse_artifact_url("https://[2001:db8::1]/pixel?id=1").unwrap();
        assert!(!ipv6.core_ready);
        assert_eq!(ipv6.host.as_deref(), Some("[2001:db8::1]"));
    }

    #[test]
    fn typical_https_host_path_and_scheme_borrow_the_artifact() {
        let artifact = "https://www.facebook.com/tr?id=1";
        let parsed = parse_artifact_url(artifact).unwrap();
        assert!(matches!(parsed.scheme, Cow::Borrowed("https")));
        assert!(matches!(
            parsed.host.as_ref(),
            Some(Cow::Borrowed("www.facebook.com"))
        ));
        assert!(matches!(parsed.path, Cow::Borrowed("/tr")));
    }

    #[test]
    fn uppercase_host_and_scheme_are_copied() {
        let parsed = parse_artifact_url("HTTPS://WWW.FACEBOOK.COM/tr").unwrap();
        assert!(matches!(parsed.scheme, Cow::Owned(_)));
        assert!(matches!(parsed.host, Some(Cow::Owned(_))));
        assert!(matches!(parsed.path, Cow::Borrowed("/tr")));
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

    #[test]
    fn prepared_macro_spans_are_reused_for_url_parse() {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: "https://example.com/pixel?id=[PIXEL_ID]".to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let prepared = PreparedArtifact::from_request(&request);
        let first = prepared.macro_spans();
        let _ = prepared.url();
        let second = prepared.macro_spans();
        assert_eq!(first.len(), 1);
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn prepared_query_params_are_reused() {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: "https://example.com/pixel?id=1&ev=Purchase".to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let prepared = PreparedArtifact::from_request(&request);
        let first = prepared.params(ParamStyle::Query);
        let second = prepared.params(ParamStyle::Query);
        assert_eq!(first.len(), 2);
        assert!(std::ptr::eq(first, second));
    }
}
