use std::fs;
use std::path::PathBuf;

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    kind: String,
    fixture: String,
    #[serde(default)]
    expansion_state: Option<String>,
    expected_ok: bool,
    expected_errors: usize,
    expected_warnings: usize,
    expected_infos: usize,
    expected_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViolationCounts {
    errors: usize,
    warnings: usize,
    infos: usize,
}

#[test]
fn golden_core_corpus_matches_expected_findings() {
    let manifest_path = fixture_root().join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).expect("read core fixture manifest");
    let cases: Vec<FixtureCase> =
        serde_json::from_str(&manifest).expect("parse core fixture manifest JSON");
    let engine = Engine::default();

    for case in cases {
        let artifact_path = fixture_root().join(&case.fixture);
        let artifact = fs::read_to_string(&artifact_path)
            .unwrap_or_else(|error| panic!("read fixture {}: {error}", artifact_path.display()));
        let request = ValidationRequest {
            artifact_kind: parse_artifact_kind(&case.kind),
            artifact,
            claimed_vendor: None,
            expansion_state: parse_expansion_state(case.expansion_state.as_deref()),
        };

        let summary = engine
            .validate(&request, &ValidationOptions::default())
            .unwrap_or_else(|error| panic!("validate fixture {}: {error}", case.id));
        let actual_codes = violation_codes(&summary);
        let actual_counts = violation_counts(&summary);

        assert_eq!(summary.reports.len(), 1, "fixture {} should emit exactly one report", case.id);
        assert_eq!(summary.reports[0].plugin_id, "core", "fixture {} should run core", case.id);
        assert_eq!(summary.is_ok(), case.expected_ok, "fixture {} ok mismatch", case.id);
        assert_eq!(actual_counts.errors, case.expected_errors, "fixture {} error count mismatch", case.id);
        assert_eq!(actual_counts.warnings, case.expected_warnings, "fixture {} warning count mismatch", case.id);
        assert_eq!(actual_counts.infos, case.expected_infos, "fixture {} info count mismatch", case.id);
        assert_eq!(actual_codes, case.expected_codes, "fixture {} violation code mismatch", case.id);
    }
}

fn parse_expansion_state(value: Option<&str>) -> ExpansionState {
    match value.unwrap_or("unknown") {
        "unknown" => ExpansionState::Unknown,
        "template" => ExpansionState::Template,
        "fired" => ExpansionState::Fired,
        other => panic!("unknown expansion state in fixture manifest: {other}"),
    }
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/core")
}

fn parse_artifact_kind(value: &str) -> ArtifactKind {
    match value {
        "url" => ArtifactKind::Url,
        "html" => ArtifactKind::HtmlSnippet,
        "js" => ArtifactKind::JavaScriptSnippet,
        "gtm" => ArtifactKind::GtmTemplate,
        "request" => ArtifactKind::NetworkRequest,
        "vast" => ArtifactKind::VastTracker,
        "postback" => ArtifactKind::ServerPostback,
        "unknown" => ArtifactKind::Unknown,
        other => panic!("unknown artifact kind in fixture manifest: {other}"),
    }
}

fn violation_codes(summary: &pixellint_core::ValidationSummary) -> Vec<String> {
    summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
        .map(|violation| violation.code.clone())
        .collect()
}

fn violation_counts(summary: &pixellint_core::ValidationSummary) -> ViolationCounts {
    let mut counts = ViolationCounts {
        errors: 0,
        warnings: 0,
        infos: 0,
    };

    for violation in summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
    {
        match violation.severity {
            Severity::Error => counts.errors += 1,
            Severity::Warning => counts.warnings += 1,
            Severity::Info => counts.infos += 1,
        }
    }

    counts
}