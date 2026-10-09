//! Offline HAR 1.2 request extraction. Responses are never sent or validated.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ArtifactKind, ArtifactOccurrence, CapturedHttpRequest, DocumentArtifactInput,
    DocumentExtractor, DocumentReport, DocumentRequest, Engine, ExpansionState, HttpCaptureContext,
    HttpHeader, HttpHeaders, HttpRequest, TimestampUnit, ValidationOptions,
};

pub use crate::http::HttpBodyAvailability as HarBodyAvailability;

/// Local resource limits, not destination or HAR specification limits.
pub const MAX_HAR_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_HAR_ENTRIES: usize = 50_000;
pub const MAX_HAR_BODY_BYTES: usize = 4 * 1024 * 1024;

/// HAR does not reliably record whether an exporter removed credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HarHeaderPolicy {
    /// Absent Authorization and Cookie are unknown. Other observed headers stay usable.
    #[default]
    Unknown,
    /// Caller establishes that absent headers were also absent on the request.
    Complete,
    /// Caller establishes Chrome's sanitized export policy. Omitted credentials are redacted.
    ChromeSanitized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HarImportOptions {
    #[serde(default)]
    pub header_policy: HarHeaderPolicy,
}

/// Capture facts, including fields that cannot be represented as wire bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarEntryMetadata {
    pub entry_index: usize,
    pub occurrence_id: String,
    pub path: String,
    pub started_date_time: Option<String>,
    pub reference_time_unix_seconds: Option<i64>,
    pub page_ref: Option<String>,
    pub body_size: Option<i64>,
    pub body_availability: HarBodyAvailability,
    pub body_reason: Option<String>,
    /// Original posted-data representation. Parameters never become invented wire text.
    pub post_data: Option<Value>,
    pub capture: HttpCaptureContext,
}

/// Extraction is separate from validation. This object remains local and can contain secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarImport {
    pub document: DocumentRequest,
    pub header_policy: HarHeaderPolicy,
    pub entries: Vec<HarEntryMetadata>,
}

/// The normal document result plus the availability facts for every original entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarReport {
    #[serde(flatten)]
    pub document: DocumentReport,
    pub header_policy: HarHeaderPolicy,
    pub reference_time_override: Option<i64>,
    pub captures: Vec<HarEntryMetadata>,
}

impl HarReport {
    pub fn is_ok(&self) -> bool {
        self.document.is_ok()
    }
}

#[derive(Deserialize)]
struct HarRoot {
    log: HarLog,
}

#[derive(Deserialize)]
struct HarLog {
    version: String,
    entries: Vec<HarEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HarEntry {
    #[serde(default)]
    started_date_time: Option<String>,
    #[serde(default, rename = "pageref")]
    page_ref: Option<String>,
    request: HarRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HarRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Option<Vec<HarHeader>>,
    #[serde(default)]
    body_size: Option<i64>,
    #[serde(default)]
    post_data: Option<HarPostData>,
}

#[derive(Deserialize)]
struct HarHeader {
    name: String,
    value: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarPostData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    params: Option<Vec<Value>>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

fn capture_clock(value: &str) -> Option<i64> {
    // HAR's timestamp has a calendar date, a time and an explicit timezone.
    // Do not accept the engine's broader local-date or compact vendor formats.
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let has_zone = value.ends_with('Z')
        || (bytes.len() >= 25
            && matches!(bytes[bytes.len() - 6], b'+' | b'-')
            && bytes[bytes.len() - 3] == b':');
    has_zone
        .then(|| crate::timestamp::timestamp_millis(value, TimestampUnit::Datetime))?
        .map(|millis| millis.div_euclid(1000))
}

fn body_availability(
    request: &HarRequest,
    headers: &[HttpHeader],
) -> (HarBodyAvailability, Option<String>) {
    let Some(post) = &request.post_data else {
        return if request.body_size == Some(0) {
            (HarBodyAvailability::Absent, None)
        } else {
            (
                HarBodyAvailability::Unavailable,
                Some("HAR has no postData and does not establish a zero-byte body.".into()),
            )
        };
    };
    if post.extra.get("_redacted").and_then(Value::as_bool) == Some(true) {
        return (
            HarBodyAvailability::Redacted,
            Some("Caller/exporter explicitly marks postData._redacted.".into()),
        );
    }
    let unavailable = |reason: &str| (HarBodyAvailability::Unavailable, Some(reason.to_string()));
    if post.extra.contains_key("encoding") || post.extra.contains_key("_encoding") {
        return unavailable(
            "HAR 1.2 does not specify request-body encoding. Encoded postData extensions are preserved without claiming decoded wire bytes.",
        );
    }
    let Some(text) = post.text.as_deref() else {
        return unavailable(
            "HAR postData has no raw text. Parameter lists and file metadata cannot establish original wire bytes.",
        );
    };
    if text.len() > MAX_HAR_BODY_BYTES {
        return unavailable("HAR request text exceeds the local 4 MiB body inspection limit.");
    }
    if post.params.is_some() {
        return unavailable(
            "HAR postData contains both text and params. The original body representation is ambiguous.",
        );
    }
    if request
        .body_size
        .is_some_and(|size| size >= 0 && usize::try_from(size).ok() != Some(text.len()))
    {
        return unavailable(
            "HAR bodySize differs from the UTF-8 text byte count. Original body bytes are unavailable.",
        );
    }
    for mime in headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("content-type"))
        .map(|header| header.value.as_str())
    {
        for parameter in mime.split(';').skip(1) {
            if let Some((name, charset)) = parameter.split_once('=')
                && name.trim().eq_ignore_ascii_case("charset")
                && !matches!(
                    charset
                        .trim()
                        .trim_matches('"')
                        .to_ascii_lowercase()
                        .as_str(),
                    "utf-8" | "utf8"
                )
                && !(charset
                    .trim()
                    .trim_matches('"')
                    .eq_ignore_ascii_case("us-ascii")
                    && text.is_ascii())
            {
                return unavailable(
                    "HAR text declares a non-UTF-8 charset. Original body bytes cannot be established from a Unicode string.",
                );
            }
        }
    }
    (HarBodyAvailability::Available, None)
}

/// Extracts HAR request fields without fetching URLs or rebuilding query/body bytes.
/// This intentionally is a request extractor, not a complete HAR schema validator.
pub fn import_har(raw: &str, options: &HarImportOptions) -> Result<HarImport, String> {
    if raw.len() > MAX_HAR_BYTES {
        return Err("HAR exceeds the local 64 MiB input limit".into());
    }
    let root: HarRoot =
        serde_json::from_str(raw).map_err(|error| format!("invalid HAR request data: {error}"))?;
    if root.log.version != "1.2" {
        return Err("HAR request import requires log.version 1.2".into());
    }
    if root.log.entries.len() > MAX_HAR_ENTRIES {
        return Err("HAR exceeds the local 50000-entry limit".into());
    }
    let mut artifacts = Vec::with_capacity(root.log.entries.len());
    let mut entries = Vec::with_capacity(root.log.entries.len());
    for (index, entry) in root.log.entries.into_iter().enumerate() {
        if entry.request.body_size.is_some_and(|size| size < -1) {
            return Err(format!(
                "HAR entry {index}: bodySize must be -1 or a nonnegative byte count"
            ));
        }
        let headers_present = entry.request.headers.is_some();
        let headers: Vec<HttpHeader> = entry
            .request
            .headers
            .as_ref()
            .into_iter()
            .flatten()
            .map(|header| HttpHeader {
                name: header.name.clone(),
                value: header.value.clone(),
            })
            .collect();
        let (body_availability, body_reason) = body_availability(&entry.request, &headers);
        let mut capture = HttpCaptureContext {
            headers_unavailable: !headers_present,
            body: Some(body_availability),
            ..Default::default()
        };
        if headers_present && options.header_policy != HarHeaderPolicy::Complete {
            for name in ["authorization", "cookie"] {
                if !headers
                    .iter()
                    .any(|header| header.name.eq_ignore_ascii_case(name))
                {
                    if options.header_policy == HarHeaderPolicy::ChromeSanitized {
                        capture.redacted_headers.push(name.into());
                    } else {
                        capture.unavailable_headers.push(name.into());
                    }
                }
            }
        }
        let body = entry
            .request
            .post_data
            .as_ref()
            .and_then(|post| post.text.clone());
        let post_data = entry
            .request
            .post_data
            .as_ref()
            .map(|post| serde_json::to_value(post).expect("HAR posted-data serialization"));
        let request = CapturedHttpRequest {
            request: HttpRequest {
                url: entry.request.url,
                method: entry.request.method,
                headers: HttpHeaders::List(headers),
                body,
            },
            capture: capture.clone(),
        };
        let occurrence_id = format!("har-entry-{index}");
        let path = format!("/log/entries/{index}/request");
        artifacts.push(DocumentArtifactInput {
            artifact_kind: ArtifactKind::NetworkRequest,
            artifact: serde_json::to_string(&request).expect("HTTP capture serialization"),
            claimed_vendor: None,
            expansion_state: ExpansionState::Fired,
            occurrences: vec![ArtifactOccurrence {
                occurrence_id: Some(occurrence_id.clone()),
                source_kind: Some("json-pointer".into()),
                path: Some(path.clone()),
                line: None,
                column: None,
                context_label: Some(format!("HAR request entry {index}")),
            }],
        });
        entries.push(HarEntryMetadata {
            entry_index: index,
            occurrence_id,
            path,
            reference_time_unix_seconds: entry.started_date_time.as_deref().and_then(capture_clock),
            started_date_time: entry.started_date_time,
            page_ref: entry.page_ref,
            body_size: entry.request.body_size,
            body_availability,
            body_reason,
            post_data,
            capture,
        });
    }
    Ok(HarImport {
        document: DocumentRequest {
            document_kind: "har".into(),
            extractor: Some(DocumentExtractor {
                id: "pixellint/har".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            artifacts,
        },
        header_policy: options.header_policy,
        entries,
    })
}

impl Engine {
    /// Replays captured requests at their recorded clocks, without network activity.
    /// Missing/invalid capture clocks require an explicit override through `validate_har_at`.
    pub fn validate_har(
        &self,
        imported: &HarImport,
        options: &ValidationOptions,
    ) -> Result<HarReport, String> {
        let clocks: Vec<_> = imported
            .entries
            .iter()
            .map(|entry| entry.reference_time_unix_seconds)
            .collect();
        if let Some(index) = clocks.iter().position(Option::is_none) {
            return Err(format!(
                "HAR entry {index}: startedDateTime is missing or not an explicit-zone HAR timestamp; supply an explicit reference-time override"
            ));
        }
        let document = self
            .validate_many_with_clocks(&imported.document, options, 0, &clocks)
            .map_err(|error| error.to_string())?;
        Ok(HarReport {
            document,
            header_policy: imported.header_policy,
            reference_time_override: None,
            captures: imported.entries.clone(),
        })
    }

    /// Replays all requests against a caller-supplied clock. This overrides HAR timestamps.
    pub fn validate_har_at(
        &self,
        imported: &HarImport,
        options: &ValidationOptions,
        reference_time_unix_seconds: i64,
    ) -> Result<HarReport, String> {
        let document = self
            .validate_many_at(&imported.document, options, reference_time_unix_seconds)
            .map_err(|error| error.to_string())?;
        Ok(HarReport {
            document,
            header_policy: imported.header_policy,
            reference_time_override: Some(reference_time_unix_seconds),
            captures: imported.entries.clone(),
        })
    }
}
