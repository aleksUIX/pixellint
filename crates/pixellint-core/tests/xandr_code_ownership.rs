//! Primary mixed-pixel examples and opaque code boundaries stay on exact /px.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, ValidationOptions, ValidationRequest,
};
use serde_json::{Value, json};

#[test]
fn primary_examples_and_boundaries_keep_independent_expectations() {
    let proof: Vec<Value> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-xandr/depth2-source-cases.json"
    ))
    .unwrap();
    let engine = Engine::default();
    for case in proof {
        let id = case["id"].as_str().unwrap();
        assert!(
            case["source_url"]
                .as_str()
                .unwrap()
                .starts_with("https://learn.microsoft.com/")
        );
        assert!(!case["source_location"].as_str().unwrap().is_empty());
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
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(codes, expected, "{id}: prewritten source oracle");
        let plugins: Vec<_> = summary
            .reports
            .iter()
            .map(|r| r.plugin_id.as_str())
            .collect();
        let expected: Vec<_> = case["expected_plugins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(plugins, expected, "{id}: route selection");
        for finding in summary
            .reports
            .iter()
            .filter(|r| r.plugin_id == "vendor/xandr")
            .flat_map(|r| &r.violations)
        {
            assert_eq!(
                finding.source.level,
                RuleSourceLevel::OfficialVendor,
                "{id}: primary evidence"
            );
        }
    }
}

#[test]
fn capture_carriers_and_templates_keep_documented_limits() {
    let mut engine = Engine::new();
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/xandr.json"))
        .unwrap();
    // Form cases expose existing engine extraction, not receiver transport support.
    for (query, mime, body, expected) in [
        (
            "id=532&seg_code=auto",
            "application/json",
            r#"{"member":"234"}"#,
            vec!["vendor.xandr.param.member.missing"],
        ),
        (
            "id=532&seg_code=auto",
            "application/x-www-form-urlencoded",
            "member=234",
            vec![],
        ),
        (
            "id=532",
            "application/x-www-form-urlencoded",
            "seg_code=auto",
            vec!["vendor.xandr.param.member.missing"],
        ),
        (
            "id=532",
            "application/x-www-form-urlencoded",
            "seg_code=auto&member=234",
            vec![],
        ),
        (
            "id=532&seg_code=0",
            "application/json",
            "{}",
            vec!["vendor.xandr.param.member.missing"],
        ),
        (
            "id=532&seg_code=%24%7BCODE%7D&member=%24%7BMEMBER%7D",
            "application/json",
            "{}",
            vec![],
        ),
        (
            "id=532&seg_code=%20&member=opaque%20owner",
            "application/json",
            "{}",
            vec![],
        ),
        ("id=532&member=", "application/json", "{}", vec![]),
    ] {
        let summary = engine.validate_at(&ValidationRequest {
            artifact_kind: ArtifactKind::NetworkRequest,
            artifact: json!({"url":format!("https://ib.adnxs.com/px?{query}"),"method":"POST","headers":{"Content-Type":mime},"body":body}).to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        }, &ValidationOptions::default(), 1_792_886_400).unwrap();
        let codes: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .map(|v| v.code.as_str())
            .collect();
        assert_eq!(
            codes, expected,
            "{query}: existing carrier extraction or opaque template"
        );
    }
}
