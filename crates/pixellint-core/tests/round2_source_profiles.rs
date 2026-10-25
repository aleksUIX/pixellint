//! Expectations are authored from pinned producer source before engine execution.
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};
use serde_json::Value;
use std::{fs, path::Path};

#[test]
fn source_profiles_match_independent_fixture_expectations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let engine = Engine::default();
    let mut checked = 0;
    for slug in [
        "fullstory-settings",
        "fullstory-page",
        "podscribe-prx-impression",
        "shareasale-legacy",
        "clipflip-vast-tracking",
    ] {
        let directory = root.join(format!("fixtures/vendor-{slug}"));
        let cases: Vec<Value> =
            serde_json::from_str(&fs::read_to_string(directory.join("manifest.json")).unwrap())
                .unwrap();
        for case in cases {
            let name = case["id"].as_str().unwrap();
            let label = format!("{slug}/{name}");
            let kind = match case["kind"].as_str().unwrap() {
                "url" => ArtifactKind::Url,
                "request" => ArtifactKind::NetworkRequest,
                "vast" => ArtifactKind::VastTracker,
                other => panic!("unexpected fixture kind {other}"),
            };
            let request = ValidationRequest {
                artifact: fs::read_to_string(directory.join(case["fixture"].as_str().unwrap()))
                    .unwrap(),
                artifact_kind: kind,
                expansion_state: ExpansionState::Unknown,
                claimed_vendor: None,
            };
            let summary = engine
                .validate_at(&request, &ValidationOptions::default(), 1792886400)
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            let plugins: Vec<_> = summary
                .reports
                .iter()
                .map(|r| r.plugin_id.as_str())
                .collect();
            let expected_plugins: Vec<_> = case["expected_plugins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(plugins, expected_plugins, "{label}: selected packs");
            let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
            let mut codes: Vec<_> = findings.iter().map(|f| f.code.as_str()).collect();
            let mut expected_codes: Vec<_> = case["expected_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            codes.sort_unstable();
            expected_codes.sort_unstable();
            assert_eq!(codes, expected_codes, "{label}: findings");
            for (severity, key) in [
                (Severity::Error, "expected_errors"),
                (Severity::Warning, "expected_warnings"),
                (Severity::Info, "expected_infos"),
            ] {
                assert_eq!(
                    findings.iter().filter(|f| f.severity == severity).count(),
                    case[key].as_u64().unwrap() as usize,
                    "{label}: {key}"
                );
            }
            for report in &summary.reports {
                if report.plugin_id == format!("vendor/{slug}") {
                    for finding in &report.violations {
                        assert_eq!(finding.severity, Severity::Warning, "{label}");
                        assert_eq!(
                            finding.source.level,
                            RuleSourceLevel::OfficialTemplate,
                            "{label}"
                        );
                        assert!(finding.source.reference.is_some(), "{label}");
                    }
                }
            }
            assert_eq!(
                summary.is_ok(),
                case["expected_ok"].as_bool().unwrap(),
                "{label}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 130);
}
