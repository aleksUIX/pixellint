//! Source-backed query advice preserves optional carriers and SDK fallbacks.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde_json::Value;

#[test]
fn pinned_sdk_boundary_urls_keep_prewritten_advisory_expectations() {
    let proof: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-cookiebot/source-boundaries.json"
    ))
    .unwrap();
    let engine = Engine::default();
    for case in proof["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::Url,
                    artifact: case["url"].as_str().unwrap().to_string(),
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
            .find(|r| r.plugin_id == "vendor/cookiebot")
            .unwrap();
        let codes: Vec<_> = report.violations.iter().map(|v| v.code.as_str()).collect();
        let expected: Vec<_> = case["expected_vendor_codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(codes, expected, "{id}: source boundary");
        assert!(
            report
                .violations
                .iter()
                .all(|v| v.severity == Severity::Warning),
            "{id}: optional advice"
        );
    }
}

#[test]
fn source_receipt_exposes_first_duplicate_and_blank_fallback_limits() {
    let proof: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-cookiebot/source-boundaries.json"
    ))
    .unwrap();
    let cases = proof["cases"].as_array().unwrap();
    let get = |id: &str| cases.iter().find(|x| x["id"] == id).unwrap();
    assert_eq!(get("blank-fallback")["banner_parameters"]["type"], "optout");
    assert_eq!(get("blank-fallback")["banner_parameters"]["culture"], "es");
    assert_eq!(
        get("valid-then-invalid-duplicate")["settings"]["autoblock"],
        true
    );
    assert_eq!(
        get("invalid-then-valid-duplicate")["settings"]["autoblock"],
        false
    );
    assert_eq!(
        get("bare-keys-fall-back")["banner_parameters"]["level"],
        "strict"
    );
    assert_eq!(get("encoded-key-not-read")["settings"]["autoblock"], false);
    assert_eq!(
        get("data-framework-query-not-read")["settings"]["hasFramework"],
        false
    );
    for (id, value) in [
        ("culture-pt-br", "PT-BR"),
        ("culture-zh-hant", "ZH-HANT"),
        ("culture-x-custom", "x-custom"),
    ] {
        assert_eq!(get(id)["banner_parameters"]["culture"], value);
    }
}
