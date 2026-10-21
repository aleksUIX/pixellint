use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, ValidationOptions, ValidationRequest,
};
use serde_json::json;

fn check(artifact: String, kind: ArtifactKind) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

#[test]
fn signed_async_values_and_unlabeled_remarketing_remain_clean() {
    for url in [
        "https://www.googleadservices.com/pagead/conversion/123/?label=synthetic&cv=9&async=1&value=-2.5",
        "https://www.googleadservices.com/pagead/conversion/123/?label=synthetic&cv=9&async=1&value=1e%2B21",
        "https://googleads.g.doubleclick.net/pagead/viewthroughconversion/123/?cv=9&async=1",
    ] {
        let result = check(url.into(), ArtifactKind::Url);
        assert!(
            result.reports.iter().all(|r| r.violations.is_empty()),
            "{result:?}"
        );
    }
}

#[test]
fn multiplier_interval_boundaries_keep_precision_and_targets() {
    for kind in [
        ArtifactKind::Url,
        ArtifactKind::VastTracker,
        ArtifactKind::ServerPostback,
    ] {
        let result = check(
            "https://e-123.adzerk.net/i.gif?e=x&s=y&eventMultiplier=100000000.0000000000000001"
                .into(),
            kind,
        );
        let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].code,
            "vendor.kevel.param.eventMultiplier.invalid"
        );
        assert!(
            findings[0]
                .targets
                .iter()
                .any(|t| t.name.as_deref() == Some("eventMultiplier"))
        );
        assert!(result.is_ok());
    }
}

#[test]
fn http_profile_guards_preserve_legacy_transport_and_unknown_markers() {
    for query in [
        "label=synthetic",
        "label=synthetic&cv=10&async=1",
        "label=synthetic&cv=[CV]&async=1",
    ] {
        let result=check(json!({"url":format!("https://www.googleadservices.com/pagead/conversion/123/?{query}"),"method":"POST","headers":{}}).to_string(),ArtifactKind::NetworkRequest);
        assert!(
            result.reports.iter().all(|r| r.violations.is_empty()),
            "{result:?}"
        );
    }
}

#[test]
fn numeric_interval_schema_rejects_invalid_bounds_and_empty_ranges() {
    for ranges in [json!([]), json!([{"minimum":2,"maximum":1}])] {
        let pack = json!({"id":"custom/interval","display_name":"Intervals","description":"Test","source_level":"heuristic","match":{"hosts":["example.test"]},"params":[{"name":"v","format":{"kind":"numeric_ranges","ranges":ranges}}]});
        assert!(ManifestRulePack::from_json(&pack.to_string()).is_err());
    }
}

#[test]
fn numeric_interval_formats_are_usable_in_rules_and_json_contracts() {
    let format = json!({"kind":"numeric_ranges","ranges":[{"minimum":-2,"maximum":-1},{"minimum":1,"maximum":2}]});
    let mut engine = Engine::default();
    let pack = json!({"id":"custom/interval","display_name":"Intervals","description":"Test","source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["v"]},"body":{"params":[{"name":"v","json_type":["number","string"],"format":format}],"rules":[{"code":"custom.interval.range","kind":"format","param":"v","format":format,"severity":"warning","message":"Range"}]}});
    engine.register_manifest_json(&pack.to_string()).unwrap();
    for (value, expected) in [
        (json!(1.5), 0),
        (json!("-1e0"), 0),
        (json!("1.00000000000000000000001"), 0),
        (json!("0.99999999999999999999999"), 2),
        (json!("2.00000000000000000000001"), 2),
        (json!("NaN"), 2),
        (json!("[VALUE]"), 0),
    ] {
        let result = engine
            .validate(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::JsonPayload,
                    artifact: json!({"v":value}).to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
            )
            .unwrap();
        assert_eq!(
            result.reports.iter().flat_map(|r| &r.violations).count(),
            expected,
            "{result:?}"
        );
    }
}
