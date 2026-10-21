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

fn endpoint() -> &'static str {
    "https://audit.imrworldwide.com/cgi-bin/gn?prd=audit&intid=synthetic&sessionid=Session123&product=dar&createtm=1792600000"
}

fn request(headers: Value, capture: Value, method: &str) -> ValidationSummary {
    check(
        ArtifactKind::NetworkRequest,
        json!({"url":endpoint(),"method":method,"headers":headers,"capture":capture}).to_string(),
    )
}

#[test]
fn reserved_heap_property_keys_target_the_bulk_row() {
    let result=check(ArtifactKind::JsonPayload,json!({"app_id":"123","users":[{"identity":"one","properties":{"role":"member"}},{"identity":"two","properties":{"Id":"synthetic"}}]}).to_string());
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1, "{result:?}");
    assert_eq!(
        findings[0].code,
        "vendor.heap-user-properties.body.properties.Id.forbidden"
    );
    assert_eq!(
        findings[0].field.as_deref(),
        Some("body.users[1].properties.Id")
    );
    assert_eq!(findings[0].targets.len(), 1);
    assert!(result.is_ok());
}

#[test]
fn heap_account_properties_keep_their_distinct_key_contract() {
    let result = check(
        ArtifactKind::JsonPayload,
        json!({"app_id":123,"account_id":"company","properties":{"id":"synthetic"}}).to_string(),
    );
    assert!(
        result
            .reports
            .iter()
            .any(|r| r.plugin_id == "vendor/heap-account-properties")
    );
    assert!(
        !result
            .reports
            .iter()
            .any(|r| r.plugin_id == "vendor/heap-user-properties")
    );
    assert!(
        result.reports.iter().all(|r| r.violations.is_empty()),
        "{result:?}"
    );
}

#[test]
fn unavailable_or_redacted_origin_does_not_invent_cors_context() {
    for (headers, capture) in [
        (
            json!({"Content-Type":"application/json"}),
            json!({"headers_unavailable":true}),
        ),
        (
            json!({"Content-Type":"application/json","Origin":"https://publisher.example.test"}),
            json!({"redacted_headers":["Origin"]}),
        ),
    ] {
        let result = request(headers, capture, "GET");
        assert!(
            result
                .reports
                .iter()
                .filter(|r| r.plugin_id == "vendor/nielsen-audit")
                .all(|r| r.violations.is_empty()),
            "{result:?}"
        );
    }
}

#[test]
fn nielsen_raw_header_bound_uses_latin1_bytes_not_utf8_bytes() {
    for (count, expected) in [(128, 0), (129, 1)] {
        let value = format!("text/plain; x={}", "é".repeat(count - 14));
        assert!(value.len() > 128);
        let result = request(
            json!({"Origin":"https://publisher.example.test","Content-Type":value}),
            json!({}),
            "GET",
        );
        let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(findings.len(), expected, "{result:?}");
        if expected == 1 {
            assert_eq!(
                findings[0].code,
                "vendor.nielsen-audit.http.headers.content-type.invalid"
            );
            assert_eq!(
                findings[0].field.as_deref(),
                Some("http.headers.content-type")
            );
            assert!(
                findings[0].targets.is_empty(),
                "Normalized capture fields must not invent source offsets."
            );
        }
    }
}

#[test]
fn same_origin_skips_cors_advice_and_keeps_known_method_advice() {
    let result = request(
        json!({"Origin":"https://audit.imrworldwide.com","Content-Type":"application/json"}),
        json!({}),
        "POST",
    );
    let codes: Vec<_> = result
        .reports
        .iter()
        .flat_map(|r| r.violations.iter().map(|v| v.code.as_str()))
        .collect();
    assert_eq!(codes, ["vendor.nielsen-audit.http.method.invalid"]);
}

#[test]
fn neighboring_routes_disclose_missing_coverage() {
    for url in [
        "https://heapanalytics.com/api/add_user_properties_extra",
        "https://audit.imrworldwide.com/cgi-bin/gnarly",
    ] {
        let result = check(ArtifactKind::Url, url.into());
        assert!(
            !result
                .reports
                .iter()
                .any(|r| r.plugin_id.starts_with("vendor/")),
            "{result:?}"
        );
        assert!(
            result
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "directory.no_rulepack_coverage")
        );
    }
}
