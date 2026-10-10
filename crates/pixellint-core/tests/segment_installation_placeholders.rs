//! Source keys are opaque. Recognized installation tokens only earn a warning.

use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest,
};
use serde::Deserialize;
use serde_json::json;
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    code: String,
    expected_code_count: usize,
    expected_severity: Severity,
    expected_source_level: RuleSourceLevel,
    source_urls: Vec<String>,
    source_statement: String,
}

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/segment.json"))
        .unwrap();
    engine
}

#[test]
fn recognized_body_and_basic_tokens_have_bounded_heuristic_warnings() {
    let engine = engine();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor-segment");
    let cases: Vec<SourceCase> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-segment/tester-followup-cases.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 13);
    for case in cases {
        assert!(
            case.source_urls
                .iter()
                .all(|url| url.starts_with("https://"))
        );
        assert!(!case.source_statement.is_empty());
        let summary = engine
            .validate(
                &ValidationRequest {
                    artifact_kind: match case.kind.as_str() {
                        "json" => ArtifactKind::JsonPayload,
                        "request" => ArtifactKind::NetworkRequest,
                        other => panic!("unsupported source case kind {other}"),
                    },
                    artifact: fs::read_to_string(root.join(case.fixture)).unwrap(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
            )
            .unwrap();
        let report = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == "vendor/segment")
            .unwrap();
        let findings: Vec<_> = report
            .violations
            .iter()
            .filter(|violation| violation.code == case.code)
            .collect();
        assert_eq!(findings.len(), case.expected_code_count, "{}", case.id);
        assert!(summary.is_ok(), "{}", case.id);
        assert_eq!(report.violations.len(), findings.len(), "{}", case.id);
        for finding in findings {
            assert_eq!(finding.severity, case.expected_severity, "{}", case.id);
            assert_eq!(
                finding.source.level, case.expected_source_level,
                "{}",
                case.id
            );
            if case.kind == "json" {
                assert!(!finding.targets.is_empty(), "{}", case.id);
            }
        }
    }
}

#[test]
fn a_literal_installation_token_is_reported_before_and_after_expansion() {
    let engine = engine();
    for expansion_state in [
        ExpansionState::Unknown,
        ExpansionState::Template,
        ExpansionState::Fired,
    ] {
        let summary = engine
            .validate(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::JsonPayload,
                    artifact: include_str!(
                        "../../../fixtures/vendor-segment/placeholder-compact-key.txt"
                    )
                    .into(),
                    claimed_vendor: None,
                    expansion_state,
                },
                &ValidationOptions::default(),
            )
            .unwrap();
        let findings: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .collect();
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].code,
            "vendor.segment.body.write_key_placeholder"
        );
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(summary.is_ok());
    }
}

#[test]
fn another_endpoint_does_not_inherit_segment_placeholder_checks_from_its_body() {
    let summary = engine()
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: json!({
                    "url": "https://collector.example.test/events",
                    "method": "POST",
                    "headers": {"Content-Type": "application/json"},
                    "body": r#"{"writeKey":"YOUR_WRITEKEY","type":"track","event":"Order Completed","userId":"synthetic-user"}"#,
                })
                .to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        summary
            .reports
            .iter()
            .all(|report| report.plugin_id != "vendor/segment")
    );
}
