//! The pinned SDK sends percent-encoded data through a string BodyInit.
//! These producer-specific checks leave ordinary HTTP evidence unchanged.
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde_json::{Value, json};

fn encoded(data: &str) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("data", data)
        .finish()
}
fn findings(
    engine: &Engine,
    capture: Value,
    options: &ValidationOptions,
) -> Vec<(String, Severity)> {
    let mut findings: Vec<_> = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: capture.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            options,
            1_791_648_000,
        )
        .unwrap()
        .reports
        .into_iter()
        .flat_map(|report| report.violations)
        .map(|finding| {
            assert!(
                finding.targets.is_empty(),
                "decoded SDK ranges are not capture ranges"
            );
            (finding.code, finding.severity)
        })
        .collect();
    findings.sort_by(|a, b| a.0.cmp(&b.0));
    findings
}
fn capture(path: &str, body: &str) -> Value {
    json!({"url":format!("https://api-js.mixpanel.com{path}?verbose=1"),"method":"POST", "headers":{"Content-Type":"text/plain;charset=UTF-8"}, "body":body})
}
#[test]
fn builtins_inspect_json_and_base64_sdk_data_at_exact_routes() {
    use base64::Engine as _;
    let engine = Engine::default();
    for (path, data, code) in [
        (
            "/track",
            r#"{"event":"synthetic","properties":{"distinct_id":"user","$insert_id":"event"}}"#,
            "vendor.mixpanel.body.properties.token.missing",
        ),
        (
            "/engage",
            r#"{"$distinct_id":"user","$set":{"name":"synthetic"}}"#,
            "vendor.mixpanel-engage.body.$token.missing",
        ),
        (
            "/groups",
            r#"{"$group_key":"company","$group_id":"synthetic","$set":{"name":"synthetic"}}"#,
            "vendor.mixpanel-groups.body.$token.missing",
        ),
    ] {
        for data in [
            data.to_owned(),
            base64::engine::general_purpose::STANDARD.encode(data),
        ] {
            for route in [path.to_owned(), format!("{path}/")] {
                assert_eq!(
                    findings(
                        &engine,
                        capture(&route, &encoded(&data)),
                        &ValidationOptions::default()
                    ),
                    vec![(code.into(), Severity::Error)]
                );
            }
        }
    }
}
#[test]
fn nearby_routes_get_no_private_sdk_preparation() {
    let engine = Engine::default();
    for path in [
        "/track/other",
        "/tracking",
        "/engage/other",
        "/groups/other",
    ] {
        let results = findings(
            &engine,
            capture(path, "data=e30="),
            &ValidationOptions::default(),
        );
        assert!(
            !results
                .iter()
                .any(|(code, _)| code.starts_with("core.request.sdk_body")
                    || code.contains("body.properties.token")
                    || code.contains("body.$token"))
        );
    }
    let mut request = capture("/track", "data=e30=");
    request["method"] = json!("GET");
    let results = findings(&engine, request, &ValidationOptions::default());
    assert!(
        !results
            .iter()
            .any(|(code, _)| code.starts_with("core.request.sdk_body")
                || code.contains("body.properties.token"))
    );
}
#[test]
fn ambiguous_body_deferred_while_observed_query_checks_continue() {
    let engine = Engine::default();
    let mut request = capture("/track", "data=e30=");
    let query = encoded(
        r#"{"event":"event","properties":{"token":"project","distinct_id":"user","$insert_id":"event"}}"#,
    );
    request["url"] = json!(format!("https://api-js.mixpanel.com/track?ip=2&{query}"));
    assert_eq!(
        findings(&engine, request, &ValidationOptions::default()),
        vec![
            ("core.request.sdk_body_unvalidated".into(), Severity::Info),
            ("vendor.mixpanel.param.ip.invalid".into(), Severity::Error),
        ]
    );
}
#[test]
fn unknown_capture_fields_keep_existing_deferral_codes() {
    let engine = Engine::default();
    for context in [
        json!({"redacted_headers":["Content-Type"]}),
        json!({"unavailable_headers":["Content-Encoding"]}),
        json!({"body":"redacted"}),
    ] {
        let mut request = capture("/track", "data=e30=");
        request["capture"] = context;
        assert_eq!(
            findings(&engine, request, &ValidationOptions::default()),
            vec![("core.request.capture_incomplete".into(), Severity::Info)]
        );
    }
}
#[test]
fn rejected_profile_resources_preserve_observed_query_errors() {
    let engine = Engine::default();
    let mut request = capture(
        "/track",
        &format!("data={{}}{}", " ".repeat(16 * 1024 * 1024)),
    );
    request["url"] = json!("https://api-js.mixpanel.com/track?ip=2");
    assert_eq!(
        findings(&engine, request, &ValidationOptions::default()),
        vec![
            ("core.request.sdk_body_limit".into(), Severity::Info),
            ("vendor.mixpanel.param.ip.invalid".into(), Severity::Error),
        ]
    );
}
#[test]
fn registering_a_same_id_manifest_revokes_builtin_capture_capability() {
    let mut engine = Engine::default();
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/mixpanel.json"))
        .unwrap();
    assert!(
        findings(
            &engine,
            capture("/track", "data=e30="),
            &ValidationOptions::default()
        )
        .is_empty()
    );
    let mut empty = Engine::new();
    empty
        .register_manifest_json(include_str!("../rulepacks/vendor/mixpanel.json"))
        .unwrap();
    assert!(
        findings(
            &empty,
            capture("/track", "data=e30="),
            &ValidationOptions::default()
        )
        .is_empty()
    );
}
#[test]
fn malformed_base64_retains_existing_vendor_error() {
    let engine = Engine::default();
    assert_eq!(
        findings(
            &engine,
            capture("/track", "data=not%3Fbase64"),
            &ValidationOptions::default()
        ),
        vec![("vendor.mixpanel.body.data.invalid".into(), Severity::Error)]
    );
}
