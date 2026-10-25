//! Source transport profiles keep opaque values and directory limits intact.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};
use serde_json::Value;

#[test]
fn source_transport_and_directory_expectations_are_preserved() {
    for (fixture, pack, level) in [
        (
            "vendor-aniview-default-track",
            "vendor/aniview-default-track",
            RuleSourceLevel::OfficialTemplate,
        ),
        (
            "vendor-adglare-native-events",
            "vendor/adglare-native-events",
            RuleSourceLevel::OfficialVendor,
        ),
    ] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(fixture);
        let cases: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
                .unwrap();
        let engine = Engine::default();
        for case in cases {
            let id = case["id"].as_str().unwrap();
            let kind = match case["kind"].as_str().unwrap() {
                "url" => ArtifactKind::Url,
                "vast" => ArtifactKind::VastTracker,
                "postback" => ArtifactKind::ServerPostback,
                "request" => ArtifactKind::NetworkRequest,
                "json" => ArtifactKind::JsonPayload,
                _ => unreachable!(),
            };
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact_kind: kind,
                        artifact: std::fs::read_to_string(
                            root.join(case["fixture"].as_str().unwrap()),
                        )
                        .unwrap(),
                        claimed_vendor: None,
                        expansion_state: ExpansionState::Unknown,
                    },
                    &ValidationOptions::default(),
                    1_792_886_400,
                )
                .unwrap();
            let actual_plugins: Vec<_> = summary
                .reports
                .iter()
                .map(|r| r.plugin_id.as_str())
                .collect();
            let expected_plugins: Vec<_> = case["expected_plugins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect();
            assert_eq!(actual_plugins, expected_plugins, "{id}: selection");
            let actual_codes: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|r| r.violations.iter())
                .map(|f| f.code.as_str())
                .collect();
            let expected_codes: Vec<_> = case["expected_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect();
            assert_eq!(
                actual_codes, expected_codes,
                "{id}: prewritten source oracle"
            );
            assert_eq!(
                summary.is_ok(),
                case["expected_ok"].as_bool().unwrap(),
                "{id}: ok"
            );
            for finding in summary
                .reports
                .iter()
                .filter(|r| r.plugin_id == pack)
                .flat_map(|r| &r.violations)
            {
                assert_eq!(
                    finding.severity,
                    Severity::Warning,
                    "{id}: advisory severity"
                );
                assert_eq!(finding.source.level, level, "{id}: evidence level");
            }
        }
    }
}
