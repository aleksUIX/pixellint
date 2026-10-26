//! Public measurement templates and explicit Lucid consent advice preserve opaque values.

use std::{fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, VendorDirectory,
};
use serde_json::Value;

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    for manifest in [
        include_str!("../rulepacks/vendor/cint-lucid-pixel.json"),
        include_str!("../rulepacks/vendor/dynata-pixel.json"),
    ] {
        engine.register_manifest_json(manifest).unwrap();
    }
    engine
        .merge_directory(
            VendorDirectory::from_json(include_str!(
                "../../../fixtures/vendor-cint-lucid-pixel/directory-overlay.json"
            ))
            .unwrap(),
        )
        .unwrap();
    engine
}

fn directory(family: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(format!("vendor-{family}"))
}

#[test]
fn independent_template_and_boundary_expectations() {
    let engine = engine();
    let mut checked = 0;
    for family in ["cint-lucid-pixel", "dynata-pixel"] {
        let dir = directory(family);
        let cases: Vec<Value> =
            serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
        let sources: Vec<Value> =
            serde_json::from_str(&fs::read_to_string(dir.join("source-cases.json")).unwrap())
                .unwrap();
        assert_eq!(cases.len(), sources.len());
        for (case, source) in cases.iter().zip(&sources) {
            let label = format!("{family}/{}", case["id"].as_str().unwrap());
            assert_eq!(case["id"], source["id"]);
            assert!(
                source["source_url"]
                    .as_str()
                    .unwrap()
                    .starts_with("https://")
            );
            assert!(!source["source_location"].as_str().unwrap().is_empty());
            let kind = match case["kind"].as_str().unwrap() {
                "url" => ArtifactKind::Url,
                "vast" => ArtifactKind::VastTracker,
                "request" => ArtifactKind::NetworkRequest,
                other => panic!("unsupported fixture kind {other}"),
            };
            let expansion_state = match case["expansion_state"].as_str().unwrap() {
                "template" => ExpansionState::Template,
                "fired" => ExpansionState::Fired,
                _ => ExpansionState::Unknown,
            };
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact_kind: kind,
                        artifact: fs::read_to_string(dir.join(case["fixture"].as_str().unwrap()))
                            .unwrap(),
                        claimed_vendor: None,
                        expansion_state,
                    },
                    &ValidationOptions::default(),
                    1_792_886_400,
                )
                .unwrap();
            let codes: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .map(|finding| finding.code.as_str())
                .collect();
            let expected: Vec<_> = case["expected_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|code| code.as_str().unwrap())
                .collect();
            assert_eq!(codes, expected, "{label}: independent source expectation");
            let plugins: Vec<_> = summary
                .reports
                .iter()
                .map(|report| report.plugin_id.as_str())
                .collect();
            let expected: Vec<_> = case["expected_plugins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|plugin| plugin.as_str().unwrap())
                .collect();
            assert_eq!(plugins, expected, "{label}: exact endpoint selection");
            assert_eq!(
                summary.is_ok(),
                case["expected_ok"].as_bool().unwrap(),
                "{label}"
            );
            for (severity, name) in [
                (Severity::Error, "expected_errors"),
                (Severity::Warning, "expected_warnings"),
                (Severity::Info, "expected_infos"),
            ] {
                let count = summary
                    .reports
                    .iter()
                    .flat_map(|r| &r.violations)
                    .filter(|v| v.severity == severity)
                    .count();
                assert_eq!(
                    count as u64,
                    case[name].as_u64().unwrap(),
                    "{label}: {name}"
                );
            }
            for finding in summary
                .reports
                .iter()
                .filter(|r| r.plugin_id.starts_with("vendor/"))
                .flat_map(|r| &r.violations)
            {
                assert_eq!(
                    finding.severity,
                    Severity::Warning,
                    "{label}: advisory scope"
                );
                assert!(matches!(
                    finding.source.level,
                    RuleSourceLevel::OfficialVendor | RuleSourceLevel::OfficialTemplate
                ));
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 117);
}

#[test]
fn official_reference_codec_choices_are_independent_of_the_validator() {
    let proof: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-cint-lucid-pixel/tcf-reference-output.json"
    ))
    .unwrap();
    assert_eq!(proof["reference_package"], "@iabtcf/core@1.5.6");
    let cases = proof["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 9);
    for case in cases {
        let consent = case["decoded"]["vendor508"].as_bool().unwrap();
        let purposes = case["decoded"]["purposeConsents"].as_array().unwrap();
        let expected = consent
            && [1, 7, 8, 9]
                .iter()
                .all(|p| purposes.contains(&Value::from(*p)));
        assert_eq!(
            expected,
            case["expected_profile_satisfied"].as_bool().unwrap()
        );
        assert!(!case["signal"].as_str().unwrap().is_empty());
    }
}

#[test]
fn url_and_vast_inputs_share_source_scope_without_podcast_headers() {
    let engine = engine();
    for (url, plugin, code) in [
        (
            "https://tracker.samplicio.us/tracker/documentation-demo/pixel.gif?pid=placement-demo&crid=creative-demo&cachebuster=opaque",
            "vendor/cint-lucid-pixel",
            "vendor.cint-lucid-pixel.param.sid.missing",
        ),
        (
            "https://tag.researchnow.com/t/beacon?pr=project-demo&adn=network-demo&ca=campaign-demo&si=publisher-demo&pl=placement-demo",
            "vendor/dynata-pixel",
            "vendor.dynata-pixel.param.cr.missing",
        ),
    ] {
        for artifact_kind in [ArtifactKind::Url, ArtifactKind::VastTracker] {
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact_kind,
                        artifact: url.to_owned(),
                        claimed_vendor: None,
                        expansion_state: ExpansionState::Unknown,
                    },
                    &ValidationOptions::default(),
                    1_792_886_400,
                )
                .unwrap();
            let report = summary
                .reports
                .iter()
                .find(|r| r.plugin_id == plugin)
                .unwrap();
            assert_eq!(report.violations.len(), 1);
            assert_eq!(report.violations[0].code, code);
            assert_eq!(report.violations[0].severity, Severity::Warning);
            assert!(summary.is_ok());
        }
    }
}
