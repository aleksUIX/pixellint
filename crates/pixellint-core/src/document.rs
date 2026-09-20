//! Document-level results for a caller that already extracted artifacts.
//!
//! Pixellint core stays single-artifact. This wrapper dedupes extracted URLs,
//! runs [`Engine::validate`] once per unique value, and returns the shape in
//! [`docs/MULTI_ARTIFACT_SCHEMA.md`](../../../docs/MULTI_ARTIFACT_SCHEMA.md).
//! Extraction, VAST XML, HTML, and GTM stay in the caller.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactKind, Engine, EngineError, ExpansionState, Severity, ValidationOptions,
    ValidationRequest, ValidationSummary,
};

/// A caller that extracted the artifacts, when the caller names itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentExtractor {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// One extracted artifact plus the places it appeared.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentArtifactInput {
    #[serde(default = "default_url_kind")]
    pub artifact_kind: ArtifactKind,
    #[serde(alias = "raw_artifact")]
    pub artifact: String,
    #[serde(default)]
    pub claimed_vendor: Option<String>,
    #[serde(default)]
    pub expansion_state: ExpansionState,
    #[serde(default)]
    pub occurrences: Vec<ArtifactOccurrence>,
}

fn default_url_kind() -> ArtifactKind {
    ArtifactKind::Url
}

/// One place an extracted artifact appeared in the source document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactOccurrence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_label: Option<String>,
}

/// Extracted artifacts from one document. The caller already pulled URLs out.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentRequest {
    #[serde(default = "default_document_kind")]
    pub document_kind: String,
    #[serde(default)]
    pub extractor: Option<DocumentExtractor>,
    #[serde(default)]
    pub artifacts: Vec<DocumentArtifactInput>,
}

fn default_document_kind() -> String {
    "list".to_string()
}

/// Counts for a document or for one unique artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindingCounts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_artifacts: Option<usize>,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

/// One unique artifact after dedupe, with every occurrence attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AggregatedArtifact {
    pub artifact_id: String,
    pub dedupe_key: String,
    pub artifact_kind: ArtifactKind,
    pub raw_artifact: String,
    pub normalized_artifact: String,
    pub ok: bool,
    pub summary: FindingCounts,
    pub reports: Vec<crate::ValidationReport>,
    pub occurrences: Vec<ArtifactOccurrence>,
}

/// Document-level result. `validate` is unchanged; this wraps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentReport {
    pub document_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extractor: Option<DocumentExtractor>,
    pub summary: FindingCounts,
    pub artifacts: Vec<AggregatedArtifact>,
}

impl DocumentReport {
    pub fn is_ok(&self) -> bool {
        self.summary.errors == 0
    }
}

/// Why a document-level run could not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    UnsupportedKind { index: usize, kind: ArtifactKind },
    Engine { index: usize, error: EngineError },
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind { index, kind } => write!(
                f,
                "artifact {index}: {} is not a validation kind. Extract tracking URLs from the snippet, then pixellint validate url. Pixellint does not parse HTML, JavaScript, or GTM containers.",
                kind_label(*kind)
            ),
            Self::Engine { index, error } => write!(f, "artifact {index}: {error}"),
        }
    }
}

impl Error for DocumentError {}

fn kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Url => "url",
        ArtifactKind::HtmlSnippet => "html",
        ArtifactKind::JavaScriptSnippet => "js",
        ArtifactKind::GtmTemplate => "gtm",
        ArtifactKind::NetworkRequest => "request",
        ArtifactKind::VastTracker => "vast",
        ArtifactKind::ServerPostback => "postback",
        ArtifactKind::JsonPayload => "json",
        ArtifactKind::Unknown => "unknown",
    }
}

fn kind_rejected(kind: ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::HtmlSnippet | ArtifactKind::JavaScriptSnippet | ArtifactKind::GtmTemplate
    )
}

fn dedupe_key(kind: ArtifactKind, normalized: &str) -> String {
    format!("{}\n{normalized}", kind_label(kind))
}

fn counts_from(summary: &ValidationSummary) -> (usize, usize, usize) {
    let mut errors = 0;
    let mut warnings = 0;
    let mut infos = 0;
    for report in &summary.reports {
        for violation in &report.violations {
            match violation.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }
        }
    }
    (errors, warnings, infos)
}

impl Engine {
    /// Validates every extracted artifact. Identical values (same kind and
    /// trimmed text) run once. Occurrences stay on the unique result.
    pub fn validate_many(
        &self,
        request: &DocumentRequest,
        options: &ValidationOptions,
    ) -> Result<DocumentReport, DocumentError> {
        let mut order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Group> = HashMap::new();

        for (index, artifact) in request.artifacts.iter().enumerate() {
            if kind_rejected(artifact.artifact_kind) {
                return Err(DocumentError::UnsupportedKind {
                    index,
                    kind: artifact.artifact_kind,
                });
            }

            let trimmed = artifact.artifact.trim();
            let key = dedupe_key(artifact.artifact_kind, trimmed);
            let occurrences = if artifact.occurrences.is_empty() {
                vec![ArtifactOccurrence {
                    occurrence_id: None,
                    source_kind: None,
                    path: None,
                    line: None,
                    column: None,
                    context_label: None,
                }]
            } else {
                artifact.occurrences.clone()
            };
            if let Some(group) = groups.get_mut(&key) {
                group.occurrences.extend(occurrences);
                continue;
            }

            let (raw_artifact, normalized) = if trimmed.len() == artifact.artifact.len() {
                (None, artifact.artifact.clone())
            } else {
                (Some(artifact.artifact.clone()), trimmed.to_string())
            };

            order.push(key.clone());
            groups.insert(
                key,
                Group {
                    first_index: index,
                    artifact_kind: artifact.artifact_kind,
                    raw_artifact,
                    normalized,
                    claimed_vendor: artifact.claimed_vendor.clone(),
                    expansion_state: artifact.expansion_state,
                    occurrences,
                },
            );
        }

        let mut artifacts = Vec::with_capacity(order.len());
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        for (artifact_number, key) in order.into_iter().enumerate() {
            let group = groups.remove(&key).expect("group exists for ordered key");
            let validation = ValidationRequest {
                artifact_kind: group.artifact_kind,
                artifact: group.normalized.clone(),
                claimed_vendor: group.claimed_vendor,
                expansion_state: group.expansion_state,
            };
            let summary =
                self.validate(&validation, options)
                    .map_err(|error| DocumentError::Engine {
                        index: group.first_index,
                        error,
                    })?;

            let (artifact_errors, artifact_warnings, artifact_infos) = counts_from(&summary);
            errors += artifact_errors;
            warnings += artifact_warnings;
            infos += artifact_infos;

            let normalized_artifact = group.normalized;
            let raw_artifact = group.raw_artifact.unwrap_or(validation.artifact);

            artifacts.push(AggregatedArtifact {
                artifact_id: format!("artifact-{}", artifact_number + 1),
                dedupe_key: key,
                artifact_kind: group.artifact_kind,
                raw_artifact,
                normalized_artifact,
                ok: summary.is_ok(),
                summary: FindingCounts {
                    artifacts_total: None,
                    unique_artifacts: None,
                    errors: artifact_errors,
                    warnings: artifact_warnings,
                    infos: artifact_infos,
                },
                reports: summary.reports,
                occurrences: group.occurrences,
            });
        }

        // Number occurrences globally in document order so ids stay unique.
        let mut occ = 1;
        for artifact in &mut artifacts {
            for occurrence in &mut artifact.occurrences {
                occurrence.occurrence_id = Some(format!("occ-{occ}"));
                occ += 1;
            }
        }

        Ok(DocumentReport {
            document_kind: if request.document_kind.trim().is_empty() {
                "list".to_string()
            } else {
                request.document_kind.clone()
            },
            extractor: request.extractor.clone(),
            summary: FindingCounts {
                artifacts_total: Some(request.artifacts.len()),
                unique_artifacts: Some(artifacts.len()),
                errors,
                warnings,
                infos,
            },
            artifacts,
        })
    }
}

struct Group {
    first_index: usize,
    artifact_kind: ArtifactKind,
    /// Original text when it differs from `normalized`. `None` means the
    /// first-seen artifact was already trimmed, so one copy is enough.
    raw_artifact: Option<String>,
    normalized: String,
    claimed_vendor: Option<String>,
    expansion_state: ExpansionState,
    occurrences: Vec<ArtifactOccurrence>,
}

/// Parses a document request from JSON. A JSON array of URL strings is a
/// `list` of `url` artifacts. Objects use the documented wrapper shape.
pub fn document_request_from_json(raw: &str) -> Result<DocumentRequest, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid document JSON: {error}"))?;
    document_request_from_value(value)
}

/// Same as [`document_request_from_json`] for an already-parsed value.
pub fn document_request_from_value(value: serde_json::Value) -> Result<DocumentRequest, String> {
    if let Some(items) = value.as_array() {
        let mut artifacts = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            if let Some(url) = item.as_str() {
                artifacts.push(DocumentArtifactInput {
                    artifact_kind: ArtifactKind::Url,
                    artifact: url.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                    occurrences: Vec::new(),
                });
                continue;
            }
            artifacts.push(
                serde_json::from_value(item.clone()).map_err(|error| {
                    format!("invalid document JSON at artifact {index}: {error}")
                })?,
            );
        }
        return Ok(DocumentRequest {
            document_kind: "list".to_string(),
            extractor: None,
            artifacts,
        });
    }

    serde_json::from_value(value).map_err(|error| format!("invalid document JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;

    fn engine() -> Engine {
        Engine::default()
    }

    fn options() -> ValidationOptions {
        ValidationOptions::default()
    }

    #[test]
    fn duplicate_urls_validate_once_and_keep_occurrences() {
        let request = document_request_from_json(
            r#"{
                "document_kind": "vast",
                "extractor": { "id": "vastlint", "version": "0.4.16" },
                "artifacts": [
                    {
                        "artifact": "https://example.com/pixel?id=1#frag",
                        "occurrences": [{ "source_kind": "xpath", "path": "/VAST/Ad[1]/InLine/Impression[1]" }]
                    },
                    {
                        "raw_artifact": "https://example.com/pixel?id=1#frag",
                        "occurrences": [{ "source_kind": "xpath", "path": "/VAST/Ad[1]/InLine/Tracking[3]" }]
                    }
                ]
            }"#,
        )
        .unwrap();

        let report = engine().validate_many(&request, &options()).unwrap();
        assert_eq!(report.document_kind, "vast");
        assert_eq!(report.extractor.as_ref().unwrap().id, "vastlint");
        assert_eq!(report.summary.artifacts_total, Some(2));
        assert_eq!(report.summary.unique_artifacts, Some(1));
        assert_eq!(report.artifacts[0].occurrences.len(), 2);
        assert_eq!(
            report.artifacts[0].raw_artifact,
            "https://example.com/pixel?id=1#frag"
        );
        assert_eq!(
            report.artifacts[0].normalized_artifact,
            "https://example.com/pixel?id=1#frag"
        );
        assert_eq!(
            report.artifacts[0].occurrences[0].path.as_deref(),
            Some("/VAST/Ad[1]/InLine/Impression[1]")
        );
        assert!(
            report.artifacts[0]
                .reports
                .iter()
                .flat_map(|item| &item.violations)
                .any(|violation| violation.code == "core.url.fragment_ignored")
        );
        assert_eq!(report.summary.warnings, 1);
        assert!(report.is_ok());
    }

    #[test]
    fn a_url_array_is_a_list_of_url_artifacts() {
        let request = document_request_from_json(
            r#"["https://www.facebook.com/tr?id=1234567890123456&ev=PageView", "https://www.facebook.com/tr?ev=PageView"]"#,
        )
        .unwrap();
        assert_eq!(request.document_kind, "list");
        let report = engine().validate_many(&request, &options()).unwrap();
        assert_eq!(report.summary.unique_artifacts, Some(2));
        assert!(!report.is_ok());
        assert_eq!(report.summary.errors, 1);
    }

    #[test]
    fn a_url_array_keeps_one_occurrence_per_row() {
        let request = document_request_from_json(
            r#"["https://example.com/pixel?id=1#frag","https://example.com/pixel?id=1#frag"]"#,
        )
        .unwrap();
        let report = engine().validate_many(&request, &options()).unwrap();
        assert_eq!(report.summary.artifacts_total, Some(2));
        assert_eq!(report.summary.unique_artifacts, Some(1));
        assert_eq!(report.artifacts[0].occurrences.len(), 2);
    }

    #[test]
    fn whitespace_duplicates_keep_the_first_raw_and_trimmed_normalized() {
        let request = DocumentRequest {
            document_kind: "list".to_string(),
            extractor: None,
            artifacts: vec![
                DocumentArtifactInput {
                    artifact_kind: ArtifactKind::Url,
                    artifact: "  https://example.com/pixel?id=1  ".to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                    occurrences: Vec::new(),
                },
                DocumentArtifactInput {
                    artifact_kind: ArtifactKind::Url,
                    artifact: "https://example.com/pixel?id=1".to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                    occurrences: Vec::new(),
                },
            ],
        };
        let report = engine().validate_many(&request, &options()).unwrap();
        assert_eq!(report.summary.artifacts_total, Some(2));
        assert_eq!(report.summary.unique_artifacts, Some(1));
        assert_eq!(
            report.artifacts[0].raw_artifact,
            "  https://example.com/pixel?id=1  "
        );
        assert_eq!(
            report.artifacts[0].normalized_artifact,
            "https://example.com/pixel?id=1"
        );
        assert_eq!(report.artifacts[0].occurrences.len(), 2);
    }

    #[test]
    fn html_extracted_items_are_rejected() {
        let request = document_request_from_json(
            r#"[{"artifact_kind":"html","artifact":"<script src=https://example.com/px.js></script>"}]"#,
        )
        .unwrap();
        let error = engine().validate_many(&request, &options()).unwrap_err();
        assert!(
            error.to_string().contains("html is not a validation kind"),
            "{error}"
        );
    }
}
