//! Independent source examples for endpoint additions discovered through D1.
//! Captured customer identifiers are never used as test data.

use std::{fs, path::PathBuf};

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde::Deserialize;

const PACKS: &[(&str, &str)] = &[
    (
        "amplified",
        include_str!("../rulepacks/vendor/amplified.json"),
    ),
    (
        "the-trade-desk-conversions",
        include_str!("../rulepacks/vendor/the-trade-desk-conversions.json"),
    ),
    (
        "adnami-tracker",
        include_str!("../rulepacks/vendor/adnami-tracker.json"),
    ),
    (
        "the-trade-desk-match",
        include_str!("../rulepacks/vendor/the-trade-desk-match.json"),
    ),
    (
        "the-trade-desk-realtime-id",
        include_str!("../rulepacks/vendor/the-trade-desk-realtime-id.json"),
    ),
    (
        "the-trade-desk",
        include_str!("../rulepacks/vendor/the-trade-desk.json"),
    ),
];

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    artifact: String,
    expected_codes: Vec<String>,
    severity: String,
    source_url: String,
}

#[derive(Deserialize)]
struct GoldenCase {
    id: String,
    kind: String,
    fixture: String,
    expected_plugins: Vec<String>,
    expected_ok: bool,
    expected_errors: usize,
    expected_warnings: usize,
    expected_infos: usize,
    expected_codes: Vec<String>,
}

#[test]
fn owned_endpoint_goldens_select_correctly_with_core() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    for (name, _) in PACKS {
        let folder = root.join(format!("vendor-{name}"));
        let cases: Vec<GoldenCase> =
            serde_json::from_str(&fs::read_to_string(folder.join("manifest.json")).unwrap())
                .unwrap();
        for case in cases {
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact_kind: match case.kind.as_str() {
                            "url" => ArtifactKind::Url,
                            "json" => ArtifactKind::JsonPayload,
                            "request" => ArtifactKind::NetworkRequest,
                            other => panic!("unsupported fixture kind {other}"),
                        },
                        artifact: fs::read_to_string(folder.join(case.fixture)).unwrap(),
                        claimed_vendor: None,
                        expansion_state: ExpansionState::Unknown,
                    },
                    &ValidationOptions::default(),
                    1_800_000_000,
                )
                .unwrap();
            let label = format!("{name}/{}", case.id);
            assert_eq!(
                summary
                    .reports
                    .iter()
                    .map(|r| r.plugin_id.clone())
                    .collect::<Vec<_>>(),
                case.expected_plugins,
                "{label} selection"
            );
            let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
            assert_eq!(summary.is_ok(), case.expected_ok, "{label} validity");
            for (severity, expected) in [
                ("error", case.expected_errors),
                ("warning", case.expected_warnings),
                ("info", case.expected_infos),
            ] {
                assert_eq!(
                    violations
                        .iter()
                        .filter(|v| format!("{:?}", v.severity).to_lowercase() == severity)
                        .count(),
                    expected,
                    "{label} {severity} count"
                );
            }
            assert_eq!(
                violations
                    .iter()
                    .map(|v| v.code.clone())
                    .collect::<Vec<_>>(),
                case.expected_codes,
                "{label} findings"
            );
        }
    }
}

#[test]
fn independently_published_endpoint_contract_examples() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut engine = Engine::new();
    for (_, manifest) in PACKS {
        engine.register_manifest_json(manifest).unwrap();
    }
    let mut checked = 0;
    for (name, _) in PACKS {
        let cases: Vec<SourceCase> = serde_json::from_str(
            &fs::read_to_string(root.join(format!("vendor-{name}/d1-source-cases.json"))).unwrap(),
        )
        .unwrap();
        for case in cases {
            assert!(case.source_url.starts_with("https://"));
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact_kind: match case.kind.as_str() {
                            "url" => ArtifactKind::Url,
                            "json" => ArtifactKind::JsonPayload,
                            "request" => ArtifactKind::NetworkRequest,
                            other => panic!("unsupported source fixture kind {other}"),
                        },
                        artifact: case.artifact,
                        claimed_vendor: None,
                        expansion_state: ExpansionState::Unknown,
                    },
                    &ValidationOptions {
                        only_rulepacks: vec![format!("vendor/{name}")],
                        except_rulepacks: Vec::new(),
                    },
                    1_800_000_000,
                )
                .unwrap();
            let mut actual: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .map(|v| (v.code.clone(), format!("{:?}", v.severity).to_lowercase()))
                .collect();
            let mut expected: Vec<_> = case
                .expected_codes
                .iter()
                .map(|code| (code.clone(), case.severity.clone()))
                .collect();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "{name}/{}", case.id);
            checked += 1;
        }
    }
    assert!(
        checked >= 171,
        "endpoint source corpus unexpectedly reduced: {checked}"
    );
}

#[test]
fn new_endpoint_routes_do_not_select_private_vendor_protocols() {
    let mut engine = Engine::new();
    for (_, manifest) in PACKS {
        engine.register_manifest_json(manifest).unwrap();
    }
    for url in [
        "https://enduser.adsrvr.org/enduser/video/?ve=start",
        "https://insight.adsrvr.org/track/clk?imp=synthetic-impression",
        "https://match.adsrvr.org/track/cmf/partner-specific",
        "https://insight.adsrvr.org/track/up-private",
        "https://functions.adnami.io/api/debug",
        "https://pixel.amplified.co/v1/js/display/organization/campaign/prove/order/line",
        "https://pixel.amplified.co/other/telemetry",
        "https://other.example/v1/vast/event/org/campaign/prove/order/line/session?sdi=imp|-1",
    ] {
        let result = engine.validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::Url,
                artifact: url.into(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
            1_800_000_000,
        );
        assert!(
            matches!(result, Err(pixellint_core::EngineError::NoMatchingPlugin)),
            "unpublished route claimed by new pack: {url}"
        );
    }
}
