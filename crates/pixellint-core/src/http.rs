//! Complete HTTP captures keep endpoint, transport and body contracts together.
//! The body is a raw string so byte limits use the submitted wire representation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    ArtifactKind, CoreRulePack, DIRECTORY_ID, Engine, EngineError, PreparedArtifact, RuleSource,
    Severity, ValidationOptions, ValidationReport, ValidationRequest, ValidationSummary,
    ValidatorPlugin, Violation, detect_macro_spans,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

/// A list preserves repeated field lines. Object keys remain case insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HttpHeaders {
    Object(BTreeMap<String, String>),
    List(Vec<HttpHeader>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpRequest {
    pub url: String,
    pub method: String,
    pub headers: HttpHeaders,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

struct DecodedRequest {
    normalized: String,
    json_body: Option<String>,
    form: bool,
    multipart_fields: Option<Vec<crate::manifest::RawParam<'static>>>,
    unavailable_form: bool,
    violations: Vec<Violation>,
}

impl Engine {
    pub(crate) fn validate_http_json_at(
        &self,
        request: &ValidationRequest,
        options: &ValidationOptions,
        clock: i64,
    ) -> Result<ValidationSummary, EngineError> {
        self.ensure_known_rulepacks(&options.only_rulepacks)?;
        self.ensure_known_rulepacks(&options.except_rulepacks)?;
        // Reject duplicate capture keys before serde can overwrite headers.
        let capture = crate::json::JsonDocument::parse(&request.artifact)
            .ok()
            .filter(|document| {
                let unique = |path: &str| {
                    let names = document.members(path);
                    names
                        .iter()
                        .map(|(name, _)| name)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == names.len()
                };
                unique("")
                    && unique("headers")
                    && document.expand("headers[]").iter().all(|path| unique(path))
            })
            .and_then(|_| serde_json::from_str::<HttpRequest>(&request.artifact).ok());
        let Some(capture) = capture else {
            return Ok(ValidationSummary {
                reports: vec![core_report(vec![request_violation(
                    "invalid_envelope",
                    None,
                    "A request capture needs string url and method, headers as an object or name/value list, and an optional raw string body.",
                    Severity::Error,
                )])],
            });
        };
        let decoded = decode_request(&capture, clock);
        let url_request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: capture.url.clone(),
            claimed_vendor: request.claimed_vendor.clone(),
            expansion_state: request.expansion_state,
        };
        let mut prepared = PreparedArtifact::from_request_at(&url_request, clock);
        prepared.mark_complete_http_request();
        if decoded.form {
            prepared.set_form_body(capture.body.as_deref().unwrap_or_default());
        }
        if let Some(fields) = &decoded.multipart_fields {
            prepared.set_form_fields(fields.clone());
        }
        if decoded.unavailable_form {
            prepared.mark_unavailable_form_body();
        }
        let body_request = decoded.json_body.as_ref().map(|body| ValidationRequest {
            artifact_kind: ArtifactKind::JsonPayload,
            artifact: body.clone(),
            claimed_vendor: request.claimed_vendor.clone(),
            expansion_state: request.expansion_state,
        });
        let body = body_request
            .as_ref()
            .map(|body| PreparedArtifact::from_request_at(body, clock));
        let excluded: BTreeSet<_> = options
            .except_rulepacks
            .iter()
            .map(String::as_str)
            .collect();
        let explicit = !options.only_rulepacks.is_empty();
        let plugins: Vec<Arc<dyn ValidatorPlugin>> = if explicit {
            options
                .only_rulepacks
                .iter()
                .filter(|id| id.as_str() != DIRECTORY_ID && !excluded.contains(id.as_str()))
                .filter_map(|id| self.plugins.get(id).map(|entry| Arc::clone(&entry.plugin)))
                .collect()
        } else {
            self.candidate_ids(&prepared)
                .into_iter()
                .filter(|id| !excluded.contains(id))
                .filter_map(|id| self.plugins.get(id))
                .filter(|entry| entry.plugin.supports_http(&prepared))
                .map(|entry| Arc::clone(&entry.plugin))
                .collect()
        };
        if plugins.is_empty()
            && !(explicit && options.only_rulepacks.iter().all(|id| id == DIRECTORY_ID))
        {
            return Err(if !explicit && excluded.is_empty() {
                EngineError::NoMatchingPlugin
            } else {
                EngineError::NoRulepacksSelected
            });
        }
        for plugin in &plugins {
            plugin.prepare_vendor_context(&mut prepared);
        }
        let mut reports: Vec<_> = plugins
            .iter()
            .map(|plugin| {
                plugin.validate_http(
                    &prepared,
                    body.as_ref(),
                    capture.body.as_ref().map_or(0, String::len),
                    &decoded.normalized,
                    plugins.iter().any(|plugin| plugin.metadata().id == "core"),
                )
            })
            .collect();
        let mut input_violations = decoded.violations;
        if let Some(body) = &body {
            input_violations.extend(CoreRulePack::default().validate_prepared(body).violations);
        }
        if !input_violations.is_empty() {
            if let Some(core) = reports.iter_mut().find(|report| report.plugin_id == "core") {
                core.violations.extend(input_violations);
            } else {
                reports.insert(0, core_report(input_violations));
            }
        }
        if let Some(report) = self.directory_report(&prepared, options, &reports) {
            reports.push(report);
        }
        // Normalized JSON and decoded-body byte ranges are not capture ranges.
        // Keep field names and source citations, never fabricate input offsets.
        let mut secrets = secret_values(&capture);
        for field in prepared.params(crate::manifest::ParamStyle::Query) {
            let name = field.name.to_ascii_lowercase();
            if name.contains("key")
                || name.contains("token")
                || name.contains("secret")
                || name.contains("password")
            {
                secrets.push(field.value.to_string());
            }
        }
        secrets.retain(|value| !value.is_empty());
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        secrets.dedup();
        for report in &mut reports {
            for violation in &mut report.violations {
                violation.targets.clear();
                for secret in &secrets {
                    violation.message = violation.message.replace(secret, "[redacted]");
                    if let Some(hint) = &mut violation.fix_hint {
                        *hint = hint.replace(secret, "[redacted]");
                    }
                }
            }
        }
        Ok(ValidationSummary { reports })
    }
}

fn core_report(violations: Vec<Violation>) -> ValidationReport {
    ValidationReport {
        plugin_id: "core".into(),
        detected_vendor: None,
        violations,
    }
}

fn request_violation(
    code: &str,
    field: Option<&str>,
    message: &str,
    severity: Severity,
) -> Violation {
    Violation {
        code: format!("core.request.{code}"),
        message: message.into(),
        severity,
        field: field.map(str::to_owned),
        fix_hint: None,
        source: RuleSource::normative(
            "HTTP Semantics (RFC 9110)",
            "https://www.rfc-editor.org/rfc/rfc9110",
        ),
        targets: Vec::new(),
    }
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn insert_repeated(object: &mut Map<String, Value>, name: String, value: String) {
    match object.get_mut(&name) {
        None => {
            object.insert(name, Value::String(value));
        }
        Some(Value::Array(values)) => values.push(Value::String(value)),
        Some(previous) => {
            *previous = Value::Array(vec![previous.take(), Value::String(value)]);
        }
    }
}

fn header_lines(headers: &HttpHeaders) -> Vec<(&str, &str)> {
    match headers {
        HttpHeaders::Object(headers) => headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect(),
        HttpHeaders::List(headers) => headers
            .iter()
            .map(|header| (header.name.as_str(), header.value.as_str()))
            .collect(),
    }
}

fn decode_request(request: &HttpRequest, clock: i64) -> DecodedRequest {
    let mut violations = Vec::new();
    if !token(&request.method) && detect_macro_spans(&request.method).is_empty() {
        violations.push(request_violation(
            "invalid_method",
            Some("method"),
            "HTTP method must be a nonempty token.",
            Severity::Error,
        ));
    }
    let mut headers = Map::new();
    for (name, value) in header_lines(&request.headers) {
        if !token(name) && detect_macro_spans(name).is_empty() {
            violations.push(request_violation(
                "invalid_header",
                Some("headers"),
                "HTTP header names must be nonempty tokens.",
                Severity::Error,
            ));
        }
        if value.bytes().any(|byte| {
            byte == 0
                || byte == b'\r'
                || byte == b'\n'
                || (byte < 32 && byte != b'\t')
                || byte == 127
        }) {
            violations.push(request_violation(
                "invalid_header",
                Some("headers"),
                "HTTP header values cannot contain control characters other than horizontal tab.",
                Severity::Error,
            ));
        }
        insert_repeated(
            &mut headers,
            name.to_ascii_lowercase(),
            value.trim_matches([' ', '\t']).into(),
        );
    }
    let ambiguous_content_type = headers.get("content-type").is_some_and(Value::is_array);
    let content_type = headers
        .get("content-type")
        .and_then(Value::as_str)
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        });
    let encoding = match (request.body.is_some(), content_type.as_deref()) {
        (false, _) => "none",
        (true, Some("application/x-www-form-urlencoded")) => "form",
        (true, Some("application/x-ndjson")) => "ndjson",
        (true, Some("multipart/form-data")) => "multipart",
        (true, Some(mime)) if mime == "application/json" || mime.ends_with("+json") => "json",
        _ => "text",
    };
    let wire = request.body.as_deref();
    let compressed = headers.get("content-encoding").is_some_and(|value| {
        value
            .as_str()
            .is_none_or(|value| !value.eq_ignore_ascii_case("identity"))
    });
    let mut unsupported = wire.is_some()
        && (compressed
            || ambiguous_content_type
            || content_type.as_deref().is_some_and(|mime| {
                mime.starts_with("multipart/") && mime != "multipart/form-data"
            }));
    if unsupported {
        violations.push(request_violation("unsupported_body_encoding", Some("body"), "This capture contains compressed data, repeated Content-Type headers or an unsupported multipart media type. Provide an unambiguous decoded capture for local body checks.", Severity::Info));
    }
    let mut decoded_body = Value::Null;
    let mut json_body = None;
    let mut multipart_fields = None;
    if !unsupported && let Some(body) = wire {
        match encoding {
            "multipart" => {
                let raw_type = headers
                    .get("content-type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match crate::multipart::decode(raw_type, body) {
                    Ok(fields) => {
                        let mut values = Map::new();
                        for field in &fields {
                            insert_repeated(
                                &mut values,
                                field.name.to_string(),
                                field.value.to_string(),
                            );
                        }
                        decoded_body = Value::Object(values);
                        multipart_fields = Some(fields);
                    }
                    Err(problem) => {
                        let (code, severity, message) = match problem {
                            crate::multipart::MultipartError::Invalid(message) => {
                                ("invalid_multipart", Severity::Error, message)
                            }
                            crate::multipart::MultipartError::Unsupported(message) => {
                                ("unsupported_body_encoding", Severity::Info, message)
                            }
                        };
                        let mut finding = request_violation(code, Some("body"), message, severity);
                        finding.source = RuleSource::normative(
                            "Multipart form data (RFC 7578 and RFC 2046)",
                            "https://www.rfc-editor.org/rfc/rfc7578",
                        );
                        violations.push(finding);
                        unsupported = true;
                    }
                }
            }
            "form" if body.trim_start().starts_with(['{', '[']) => {
                // Some official curl examples send raw JSON with curl's form
                // default. Keep the MIME evidence while inspecting the entity.
                json_body = Some(body.into());
                decoded_body = decode_json_entity(body);
            }
            "form" => {
                let mut fields = Map::new();
                for (name, value) in url::form_urlencoded::parse(body.as_bytes()) {
                    insert_repeated(&mut fields, name.into_owned(), value.into_owned());
                }
                decoded_body = Value::Object(fields);
            }
            "ndjson" => {
                let mut values = Vec::new();
                let mut valid = true;
                for line in body.lines().filter(|line| !line.trim().is_empty()) {
                    match serde_json::from_str::<Value>(line) {
                        Ok(value) => values.push(value),
                        Err(_) => {
                            valid = false;
                            break;
                        }
                    }
                }
                if valid {
                    decoded_body = Value::Array(values);
                    json_body = Some(decoded_body.to_string());
                } else {
                    violations.push(request_violation(
                        "invalid_ndjson",
                        Some("body"),
                        "Each nonempty NDJSON line must be valid JSON.",
                        Severity::Error,
                    ));
                }
            }
            _ if encoding == "json" || body.trim_start().starts_with(['{', '[']) => {
                json_body = Some(body.into());
                decoded_body = decode_json_entity(body);
            }
            _ => decoded_body = Value::String(body.into()),
        }
    }
    if json_body
        .as_ref()
        .is_some_and(|body| crate::json::JsonDocument::parse(body).is_err())
    {
        decoded_body = Value::Null;
    }
    let mut normalized = Map::new();
    normalized.insert("url".into(), Value::String(request.url.clone()));
    normalized.insert("method".into(), Value::String(request.method.clone()));
    let url_request = ValidationRequest {
        artifact_kind: ArtifactKind::Url,
        artifact: request.url.clone(),
        claimed_vendor: None,
        expansion_state: crate::ExpansionState::Unknown,
    };
    let parsed = PreparedArtifact::from_request_at(&url_request, clock);
    if let Some(url) = parsed.url() {
        normalized.insert("path".into(), Value::String(url.path.to_string()));
        if let Some(host) = &url.host {
            normalized.insert("host".into(), Value::String(host.to_string()));
        }
    }
    let mut query = Map::new();
    let without_fragment = request.url.split('#').next().unwrap_or_default();
    if let Some((_, query_text)) = without_fragment.split_once('?') {
        for (name, value) in
            url::form_urlencoded::parse(query_text.split('#').next().unwrap_or_default().as_bytes())
        {
            insert_repeated(&mut query, name.into_owned(), value.into_owned());
        }
    }
    normalized.insert("query".into(), Value::Object(query));
    if let Some(content_type) = content_type {
        normalized.insert("content_type".into(), Value::String(content_type));
    }
    if let Some(auth) = headers.get("authorization").and_then(Value::as_str)
        && let Some((scheme, credentials)) = auth.split_once([' ', '\t'])
    {
        normalized.insert(
            "authorization_scheme".into(),
            Value::String(scheme.to_ascii_lowercase()),
        );
        if scheme.eq_ignore_ascii_case("basic")
            && let Ok(decoded) =
                crate::manifest::decode_base64_json(credentials.trim_start_matches([' ', '\t']))
        {
            let (username, password, has_colon) = match decoded.split_once(':') {
                Some((username, password)) => (username, password, true),
                None => (decoded.as_str(), "", false),
            };
            normalized.insert("basic_auth".into(), serde_json::json!({ "username":username, "password":password, "has_colon":has_colon }));
        }
    }
    if ambiguous_content_type {
        normalized.insert("content_type_unavailable".into(), Value::Bool(true));
    }
    normalized.insert("headers".into(), Value::Object(headers));
    normalized.insert("body".into(), decoded_body);
    normalized.insert(
        "body_encoding".into(),
        Value::String(if unsupported { "unsupported" } else { encoding }.into()),
    );
    let unavailable_form = unsupported
        && (ambiguous_content_type
            || encoding == "form"
            || content_type_from_headers(request)
                .is_some_and(|mime| mime.starts_with("multipart/")));
    let form = encoding == "form" && !unsupported && json_body.is_none();
    DecodedRequest {
        normalized: Value::Object(normalized).to_string(),
        json_body,
        form,
        multipart_fields,
        unavailable_form,
        violations,
    }
}

fn content_type_from_headers(request: &HttpRequest) -> Option<String> {
    header_lines(&request.headers)
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
}

fn decode_json_entity(body: &str) -> Value {
    // Preserve the engine's bounded entity parser. A rejected deep entity must
    // not also prevent observable method and header contracts from running.
    if crate::json::JsonDocument::parse(body).is_err() {
        return Value::Null;
    }
    serde_json::from_str(body).unwrap_or(Value::Null)
}

fn secret_values(request: &HttpRequest) -> Vec<String> {
    let mut values = Vec::new();
    for (name, value) in header_lines(&request.headers) {
        let name = name.to_ascii_lowercase();
        if name.contains("authorization")
            || name.contains("cookie")
            || name.contains("key")
            || name.contains("token")
            || name == "authentication"
        {
            values.push(value.to_owned());
            let value = value.trim_matches([' ', '\t']);
            values.push(value.to_owned());
            if let Some((scheme, credential)) = value.split_once([' ', '\t']) {
                let credential = credential.trim_start_matches([' ', '\t']);
                values.push(credential.to_owned());
                if scheme.eq_ignore_ascii_case("basic")
                    && let Ok(decoded) = crate::manifest::decode_base64_json(credential)
                {
                    values.push(decoded.clone());
                    if let Some((username, password)) = decoded.split_once(':') {
                        values.extend([username.to_owned(), password.to_owned()]);
                    }
                }
            }
        }
    }
    values.retain(|value| !value.is_empty());
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
}
