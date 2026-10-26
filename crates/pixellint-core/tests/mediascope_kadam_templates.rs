//! Published path templates and native producer controls keep their boundaries.

use pixellint_core::{
    ArtifactKind, Engine, EngineError, ExpansionState, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, VendorDirectory,
};
use serde_json::Value;

const MED: &str = include_str!("../rulepacks/vendor/mediascope-counter.json");
const KAD: &str = include_str!("../rulepacks/vendor/kadam-cookie-sync.json");

fn request(case: &Value) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: match case["kind"].as_str().unwrap() {
            "url" => ArtifactKind::Url,
            "request" => ArtifactKind::NetworkRequest,
            _ => unreachable!(),
        },
        artifact: case["artifact"].as_str().unwrap().to_string(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

fn cases(source: &str) -> Vec<Value> {
    serde_json::from_str(source).unwrap()
}

#[test]
fn source_cases_preserve_independently_declared_findings_and_selection() {
    let mut failures = Vec::new();
    for (manifest, pack, source) in [
        (
            MED,
            "vendor/mediascope-counter",
            include_str!("../../../fixtures/vendor-mediascope-counter/source-cases.json"),
        ),
        (
            KAD,
            "vendor/kadam-cookie-sync",
            include_str!("../../../fixtures/vendor-kadam-cookie-sync/source-cases.json"),
        ),
    ] {
        let mut engine = Engine::new();
        engine.register_manifest_json(manifest).unwrap();
        for case in cases(source) {
            let id = case["id"].as_str().unwrap();
            let summary = match engine.validate_at(
                &request(&case),
                &ValidationOptions::default(),
                1_792_886_400,
            ) {
                Ok(summary) => summary,
                Err(EngineError::NoMatchingPlugin) if case["selected"] == false => continue,
                Err(error) => panic!("{id}: {error}"),
            };
            let selected = summary.reports.iter().any(|r| r.plugin_id == pack);
            let actual: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .map(|v| v.code.as_str())
                .collect();
            let expected: Vec<_> = case["expected_vendor_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            if actual != expected || selected != case["selected"].as_bool().unwrap() {
                failures.push(format!("{pack}/{id}: selected={selected}, expected={}, codes={actual:?}, expected={expected:?}",case["selected"]));
            }
            for finding in summary
                .reports
                .iter()
                .filter(|r| r.plugin_id == pack)
                .flat_map(|r| &r.violations)
            {
                assert_eq!(finding.severity, Severity::Warning, "{id}");
                let official_naming = pack == "vendor/mediascope-counter"
                    && (finding.code.contains(".account.") || finding.code.contains(".tmsec."));
                assert_eq!(
                    finding.source.level,
                    if official_naming {
                        RuleSourceLevel::OfficialVendor
                    } else {
                        RuleSourceLevel::OfficialTemplate
                    },
                    "{id}"
                );
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn path_names_cannot_be_supplied_by_query_or_form_and_targets_stay_physical() {
    for (manifest, pack, source, id, literal) in [
        (
            MED,
            "vendor/mediascope-counter",
            include_str!("../../../fixtures/vendor-mediascope-counter/source-cases.json"),
            "path-account-spoof",
            "Bad_ru",
        ),
        (
            KAD,
            "vendor/kadam-cookie-sync",
            include_str!("../../../fixtures/vendor-kadam-cookie-sync/source-cases.json"),
            "ssp-path-spoof",
            "",
        ),
    ] {
        let mut engine = Engine::new();
        engine.register_manifest_json(manifest).unwrap();
        let case = cases(source).into_iter().find(|v| v["id"] == id).unwrap();
        let request = request(&case);
        let summary = engine
            .validate_at(&request, &ValidationOptions::default(), 1_792_886_400)
            .unwrap();
        let report = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == pack)
            .unwrap();
        assert_eq!(report.violations.len(), 1);
        let target = &report.violations[0].targets[0];
        assert!(target.start <= request.artifact.find('?').unwrap());
        assert_eq!(&request.artifact[target.start..target.end], literal);
    }
}

#[test]
fn native_proof_covers_emission_omission_and_every_saved_capture() {
    let proof: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-mediascope-counter/source-proof.json"
    ))
    .unwrap();
    let source = cases(include_str!(
        "../../../fixtures/vendor-mediascope-counter/source-cases.json"
    ));
    assert_eq!(proof["network_calls"], 0);
    assert_eq!(proof["scenario_count"], 46);
    assert_eq!(proof["capture_count"], 42);
    for scenario in proof["results"].as_array().unwrap() {
        let id = scenario["id"].as_str().unwrap();
        if scenario["captures"].as_array().unwrap().is_empty() {
            assert!(id.starts_with("sdk-autohit-"));
        } else {
            assert!(source.iter().any(|case| case["producer_id"] == id), "{id}");
        }
    }
    let proof: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-kadam-cookie-sync/source-proof.json"
    ))
    .unwrap();
    assert_eq!(proof["network_calls"], 0);
    assert_eq!(proof["results"].as_array().unwrap().len(), 14);
    assert_eq!(
        proof["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["url"]
                .as_str()
                .unwrap()
                .starts_with("https://d.uuidksinc.net/"))
            .count(),
        7
    );
}

#[test]
fn forced_selection_keeps_the_endpoint_mismatch() {
    for (manifest, pack, artifact) in [
        (
            MED,
            "vendor/mediascope-counter",
            "https://www.tns-counter.ru/tcounter.js",
        ),
        (
            KAD,
            "vendor/kadam-cookie-sync",
            "https://d.uuidksinc.net/sync/opaque/?remote_uid=opaque",
        ),
    ] {
        let mut engine = Engine::new();
        engine.register_manifest_json(manifest).unwrap();
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: artifact.into(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let summary = engine
            .validate_at(
                &request,
                &ValidationOptions {
                    only_rulepacks: vec![pack.into()],
                    except_rulepacks: vec![],
                },
                1_792_886_400,
            )
            .unwrap();
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code.ends_with(".endpoint_mismatch"))
        );
    }
}

#[test]
fn full_core_directory_and_vendor_goldens_match_before_registration() {
    let mut engine = Engine::default();
    engine.register_manifest_json(MED).unwrap();
    engine.register_manifest_json(KAD).unwrap();
    // Local directory overlay mirrors the coordinator-owned exact roots.
    engine.set_directory(VendorDirectory::from_json(r#"{"entries":[{"vendor":"mediascope","display_name":"Mediascope","category":"analytics","hosts":["www.tns-counter.ru"],"rulepack":"vendor/mediascope-counter"},{"vendor":"kadam","display_name":"Kadam","category":"advertising","hosts":["s.uuidksinc.net","d.uuidksinc.net"],"rulepack":"vendor/kadam-cookie-sync"}]}"#).unwrap());
    for pack in ["mediascope-counter", "kadam-cookie-sync"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(format!("vendor-{pack}"));
        let golden: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
                .unwrap();
        for case in golden {
            let mut source = case.clone();
            source["artifact"] =
                std::fs::read_to_string(root.join(case["fixture"].as_str().unwrap()))
                    .unwrap()
                    .into();
            let summary = engine
                .validate_at(
                    &request(&source),
                    &ValidationOptions::default(),
                    1_792_886_400,
                )
                .unwrap();
            let plugins: Vec<_> = summary
                .reports
                .iter()
                .map(|r| r.plugin_id.as_str())
                .collect();
            let expected: Vec<_> = case["expected_plugins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(plugins, expected, "{}: selection", case["id"]);
            let codes: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .map(|v| v.code.as_str())
                .collect();
            let expected: Vec<_> = case["expected_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(codes, expected, "{}: findings", case["id"]);
            for (severity, key) in [
                (Severity::Error, "expected_errors"),
                (Severity::Warning, "expected_warnings"),
                (Severity::Info, "expected_infos"),
            ] {
                assert_eq!(
                    summary
                        .reports
                        .iter()
                        .flat_map(|r| &r.violations)
                        .filter(|v| v.severity == severity)
                        .count() as u64,
                    case[key].as_u64().unwrap(),
                    "{}: {key}",
                    case["id"]
                );
            }
        }
    }
}
