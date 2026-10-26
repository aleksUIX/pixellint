//! Stable CustomVariables processor boundaries retain independent expectations.

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde_json::Value;

#[test]
fn stable_processor_numeric_trim_and_alternative_boundaries() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-matomo-custom-variable-depth/source-cases.json"
    ))
    .unwrap();
    let mut engine = Engine::new();
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/matomo.json"))
        .unwrap();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        assert!(case["source_url"].as_str().unwrap().contains("6ae495b4"));
        assert!(!case["source_location"].as_str().unwrap().is_empty());
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::Url,
                    artifact: case["artifact"].as_str().unwrap().to_owned(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let mut actual: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .map(|v| {
                (
                    v.code.clone(),
                    format!("{:?}", v.severity).to_lowercase(),
                    serde_json::to_value(v.source.level)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect();
        let mut expected: Vec<_> = case["expected_findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["code"].as_str().unwrap().to_owned(),
                    v["severity"].as_str().unwrap().to_owned(),
                    v["level"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{id}: independent processor boundary");
        assert_eq!(summary.reports.len(), 1, "{id}: existing pack selection");
    }
    assert_eq!(cases.len(), 74);
}

#[test]
fn proof_records_declared_runtime_seams_and_stable_agreement() {
    let proof: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-matomo-custom-variable-depth/processor-proof.json"
    ))
    .unwrap();
    assert_eq!(proof["input_cases"], 37);
    assert_eq!(proof["source_files_unmodified"], true);
    assert_eq!(proof["network_requests"], 0);
    assert!(proof["method"].as_str().unwrap().contains("stubs"));
    let versions = proof["outputs"].as_array().unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["results"], versions[1]["results"]);
    for version in versions {
        assert_eq!(version["php_version"], "8.3.33");
        assert_eq!(version["mb_internal_encoding"], "UTF-8");
        assert_eq!(version["results"].as_array().unwrap().len(), 74);
        for row in version["results"].as_array().unwrap() {
            assert!(row.get("error").is_none(), "{row:?}");
        }
        for (id, expected) in [
            ("numeric-value", "53"),
            ("numeric-name-value", "0"),
            ("padded-under-budget-after-trim", "value"),
            ("php-default-trim-byte-padding", "value"),
            ("whitespace-only", ""),
            ("long-json-number", "1.0E+300"),
        ] {
            let rows: Vec<_> = version["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["id"] == id)
                .collect();
            assert_eq!(rows.len(), 2);
            for row in rows {
                assert_eq!(row["output"]["custom_var_v1"], expected);
            }
        }
    }
}
