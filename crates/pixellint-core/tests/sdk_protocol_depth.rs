//! Source-produced captures and independent oracles for SDK protocol depth.
use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    code: String,
    severity: String,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    kind: String,
    artifact: String,
    expected_findings: Vec<Finding>,
    evidence_kind: String,
    source_url: String,
    source_statement: String,
    reference_time: i64,
}

fn check_cases(pack: &str, cases: &str) {
    let mut engine = Engine::new();
    engine.register_manifest_json(pack).unwrap();
    let cases: Vec<Case> = serde_json::from_str(cases).unwrap();
    for mut case in cases {
        assert!(case.source_url.starts_with("https://"));
        assert!(!case.source_statement.is_empty());
        assert!(!case.evidence_kind.is_empty());
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: match case.kind.as_str() {
                        "request" => ArtifactKind::NetworkRequest,
                        "url" => ArtifactKind::Url,
                        other => panic!("unknown source artifact kind: {other}"),
                    },
                    artifact: case.artifact,
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                case.reference_time,
            )
            .unwrap();
        let mut actual: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .map(|v| Finding {
                code: v.code.clone(),
                severity: format!("{:?}", v.severity).to_lowercase(),
            })
            .collect();
        actual.sort();
        case.expected_findings.sort();
        assert_eq!(actual, case.expected_findings, "{}: {summary:?}", case.id);
    }
}

#[test]
fn ftrack_pinned_sdk_variants_and_independent_transport_boundaries() {
    check_cases(
        include_str!("../rulepacks/vendor/flashtalking-ftrack.json"),
        include_str!("../../../fixtures/vendor-flashtalking-ftrack-depth/source-cases.json"),
    );
}

#[test]
fn matomo_php_scalar_queries_and_explicit_deferred_controls() {
    check_cases(
        include_str!("../rulepacks/vendor/matomo.json"),
        include_str!("../../../fixtures/vendor-matomo-php-scalar-depth/source-cases.json"),
    );
}

#[test]
fn ftrack_headerless_hint_stays_bound_to_endpoint_and_pack_selection() {
    let engine = Engine::default();
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-flashtalking-ftrack-depth/source-cases.json"
    ))
    .unwrap();
    let mut capture: serde_json::Value = serde_json::from_str(
        &cases
            .iter()
            .find(|c| c.id == "xdr-observed-missing-device-time")
            .unwrap()
            .artifact,
    )
    .unwrap();
    for url in [
        "https://unrelated.example/lgc",
        "https://d9.flashtalking.com/lgc-other",
    ] {
        capture["url"] = url.into();
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: capture.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        assert!(
            !summary
                .reports
                .iter()
                .any(|r| r.plugin_id == "vendor/flashtalking-ftrack")
        );
        assert!(
            !summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code.starts_with("vendor.flashtalking-ftrack"))
        );
    }
    capture["url"] = "https://d9.flashtalking.com/lgc".into();
    let summary = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: capture.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions {
                except_rulepacks: vec!["vendor/flashtalking-ftrack".into()],
                ..ValidationOptions::default()
            },
            1_800_000_000,
        )
        .unwrap();
    assert!(
        !summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code.starts_with("vendor.flashtalking-ftrack"))
    );
}
