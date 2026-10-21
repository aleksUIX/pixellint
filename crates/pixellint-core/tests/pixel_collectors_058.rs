use pixellint_core::{
    ArtifactKind, Engine, ValidationOptions, ValidationRequest, ValidationSummary,
};
use serde_json::{Value, json};

fn check(kind: ArtifactKind, artifact: String) -> ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact,
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

fn plausible(extra: Value) -> ValidationSummary {
    let mut body = json!({"name":"pageview","url":"https://publisher.example.test/","domain":"publisher.example.test"});
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    check(ArtifactKind::JsonPayload, body.to_string())
}

#[test]
fn ignored_property_aliases_do_not_produce_advice() {
    for first in [json!({"label":"valid"}), json!("")] {
        let result =
            plausible(json!({"m":first,"meta":{"bad":[]},"p":{"bad":[]},"props":{"bad":[]}}));
        assert!(
            result.reports.iter().all(|r| r.violations.is_empty()),
            "{result:?}"
        );
    }
}

#[test]
fn property_byte_advice_preserves_the_actual_field_target() {
    let result = plausible(json!({"p":{"label":"é".repeat(1001)}}));
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1, "{result:?}");
    assert_eq!(findings[0].field.as_deref(), Some("body.p.*"));
    assert_eq!(findings[0].targets.len(), 1);
    assert_eq!(findings[0].targets[0].name.as_deref(), Some("p.label"));
    assert!(result.is_ok());
}

#[test]
fn encoded_revenue_advice_points_to_the_original_json_string() {
    let encoded = r#"{"currency":"ZZZ","amount":12}"#;
    let result = plausible(json!({"revenue":encoded}));
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1, "{result:?}");
    assert_eq!(findings[0].field.as_deref(), Some("body.revenue.currency"));
    assert_eq!(findings[0].targets.len(), 1);
    assert_eq!(findings[0].targets[0].name.as_deref(), Some("revenue"));
    assert_eq!(findings[0].targets[0].value.as_deref(), Some(encoded));
    assert!(result.is_ok());
}

#[test]
fn engagement_advice_uses_short_name_precedence_and_keeps_macros_unknown() {
    for name in ["pageview", "{{EVENT_NAME}}"] {
        let result = plausible(json!({"name":"engagement","n":name,"e":"invalid","sd":-1}));
        assert!(
            result
                .reports
                .iter()
                .filter(|r| r.plugin_id == "vendor/plausible")
                .all(|r| r.violations.is_empty()),
            "{result:?}"
        );
    }
}

#[test]
fn amplitude_operation_limit_identifies_the_bulk_row() {
    let properties: serde_json::Map<String, Value> = (0..1025)
        .map(|i| (format!("key{i}"), json!("synthetic")))
        .collect();
    let identification = json!([
        {"group_type":"company","group_value":"one","group_properties":{"$set":{"label":"valid"}}},
        {"group_type":"company","group_value":"two","group_properties":{"$set":properties}}
    ])
    .to_string();
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("api_key", "synthetic")
        .append_pair("identification", &identification)
        .finish();
    let result = check(ArtifactKind::NetworkRequest, json!({"url":"https://api2.amplitude.com/groupidentify","method":"POST","headers":{"Content-Type":"application/x-www-form-urlencoded"},"body":body}).to_string());
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1, "{result:?}");
    assert_eq!(
        findings[0].code,
        "vendor.amplitude-group-identify.body.group_properties.$set.invalid"
    );
    assert!(
        findings[0]
            .field
            .as_deref()
            .unwrap()
            .contains("[1].group_properties.$set"),
        "{result:?}"
    );
    assert!(
        findings[0].targets.is_empty(),
        "Normalized form captures have no original query offsets."
    );
}

#[test]
fn neighboring_routes_and_non_ingestion_hosts_are_not_contracted() {
    for endpoint in [
        "https://plausible.io/api/event-extra",
        "https://api2.amplitude.com/groupidentify/extra",
        "https://analytics.amplitude.com/groupidentify",
    ] {
        let result = check(ArtifactKind::Url, endpoint.into());
        assert!(
            result
                .reports
                .iter()
                .all(|r| !r.plugin_id.starts_with("vendor/")),
            "{result:?}"
        );
    }
}
