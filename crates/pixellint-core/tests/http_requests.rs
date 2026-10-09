//! Complete captures have an independent engine oracle. Vendor fixtures cover
//! the destination-specific contract decisions.

use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, ValidationOptions, ValidationRequest,
    ValidationSummary,
};
use serde_json::{Value, json};

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/alpha", "display_name":"Alpha", "description":"HTTP integration fixture",
        "vendor":"alpha", "docs":"https://example.org/alpha",
        "match":{"hosts":["alpha.example"],"paths":["/events"],"json_paths":["events"]},
        "body":{"params":[{"name":"events","requirement":"required","json_type":"array"}],
            "rules":[{"code":"vendor.alpha.body.wire_limit","kind":"max_body_bytes","max_bytes":64,
                "severity":"error","message":"Wire body exceeds 64 bytes."}]},
        "http":{"params":[
            {"name":"method","requirement":"required","format":{"kind":"enum","values":["POST"]}},
            {"name":"content_type","requirement":"required","format":{"kind":"enum","values":["application/json","application/x-ndjson"]}},
            {"name":"headers.x-key","json_type":"string"},
            {"name":"query.id","json_type":"string"},
            {"name":"basic_auth.username","format":{"kind":"regex","pattern":"^[a-z]+$"}},
            {"name":"authorization_scheme"},
            {"name":"body.left"}, {"name":"body.right"}
        ],"rules":[{"code":"vendor.alpha.http.equal","kind":"equal_values","left":"body.left","right":"body.right",
            "severity":"error","message":"Both populated fields must agree."}]}
    }).to_string()).unwrap();
    engine.register_manifest_json(&json!({
        "id":"vendor/beta", "display_name":"Beta", "description":"Body shape routing fixture",
        "vendor":"beta", "docs":"https://example.org/beta",
        "match":{"hosts":["beta.example"],"paths":["/events"],"json_paths":["events"]},
        "body":{"params":[{"name":"beta_only","requirement":"required"}]}
    }).to_string()).unwrap();
    engine
}

fn capture(body: &str) -> Value {
    json!({"url":"https://alpha.example/events", "method":"POST",
        "headers":{"Content-Type":"Application/JSON; charset=UTF-8"}, "body":body})
}

fn validate(engine: &Engine, value: Value, only: &[&str]) -> ValidationSummary {
    engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: value.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions {
                only_rulepacks: only.iter().map(|id| (*id).into()).collect(),
                except_rulepacks: vec![],
            },
            1_800_000_000,
        )
        .unwrap()
}

fn codes(summary: &ValidationSummary) -> Vec<&str> {
    summary
        .reports
        .iter()
        .flat_map(|report| {
            report
                .violations
                .iter()
                .map(|violation| violation.code.as_str())
        })
        .collect()
}

#[test]
fn complete_request_binds_native_body_to_endpoint() {
    let engine = engine();
    let summary = validate(&engine, capture(r#"{"events":[]}"#), &[]);
    assert!(summary.is_ok(), "{summary:?}");
    assert_eq!(
        summary
            .reports
            .iter()
            .map(|report| report.plugin_id.as_str())
            .collect::<Vec<_>>(),
        ["core", "vendor/alpha"]
    );
    let bare = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::JsonPayload,
                artifact: r#"{"events":[]}"#.into(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            1_800_000_000,
        )
        .unwrap();
    assert!(codes(&bare).contains(&"vendor.beta.body.beta_only.missing"));
    assert!(!codes(&bare).iter().any(|code| code.contains(".http.")));
}

#[test]
fn explicit_wrong_endpoint_never_contracts_its_body() {
    let summary = validate(&engine(), capture(r#"{"events":[]}"#), &["vendor/beta"]);
    assert_eq!(codes(&summary), ["vendor.beta.endpoint_mismatch"]);
}

#[test]
fn header_case_mime_parameters_and_decoded_query_are_normalized() {
    let mut request = capture(r#"{"events":[]}"#);
    request["headers"]["X-Key"] = json!("valid");
    request["url"] = json!("https://alpha.example/events?id=a%2Bb");
    assert!(validate(&engine(), request, &[]).is_ok());
}

#[test]
fn repeated_headers_and_queries_do_not_silently_overwrite() {
    let mut request = capture(r#"{"events":[]}"#);
    request["headers"] = json!([
        {"name":"Content-Type","value":"application/json"},
        {"name":"X-Key","value":"first"}, {"name":"x-key","value":"second"}
    ]);
    request["url"] = json!("https://alpha.example/events?id=one&id=two");
    let summary = validate(&engine(), request, &[]);
    assert!(
        codes(&summary).contains(&"vendor.alpha.http.headers.x-key.invalid"),
        "{summary:?}"
    );
    assert!(codes(&summary).contains(&"vendor.alpha.http.query.id.invalid"));
}

#[test]
fn invalid_capture_shape_and_duplicate_object_keys_are_input_errors() {
    let engine = engine();
    for raw in [
        "[]",
        r#"{"url":"https://alpha.example/events","method":"POST"}"#,
        r#"{"url":"https://alpha.example/events","method":"POST","headers":{},"body":{}}"#,
        r#"{"url":"https://alpha.example/events","method":"POST","headers":{"Authorization":"first","Authorization":"second"}}"#,
        r#"{"url":"https://alpha.example/events","method":"POST","headers":{},"typo":true}"#,
    ] {
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: raw.into(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        assert_eq!(codes(&summary), ["core.request.invalid_envelope"], "{raw}");
    }
}

#[test]
fn header_control_characters_and_method_whitespace_are_invalid() {
    let mut request = capture(r#"{"events":[]}"#);
    request["method"] = json!("POST /bad");
    request["headers"]["X-Test"] = json!("ok\r\nInjected: header");
    let summary = validate(&engine(), request, &[]);
    assert!(codes(&summary).contains(&"core.request.invalid_method"));
    assert!(codes(&summary).contains(&"core.request.invalid_header"));
}

#[test]
fn malformed_native_json_is_checked_with_only_vendor_selection() {
    let summary = validate(&engine(), capture("{broken}"), &["vendor/alpha"]);
    assert!(codes(&summary).contains(&"core.json.parse_error"));
    assert_eq!(summary.reports[0].plugin_id, "core");
}

#[test]
fn body_limit_uses_wire_bytes_including_whitespace_and_unicode() {
    let mut request = capture(&format!("{}{{\"events\":[]}}", " ".repeat(60)));
    assert!(
        codes(&validate(&engine(), request.clone(), &[])).contains(&"vendor.alpha.body.wire_limit")
    );
    request["body"] = json!(r#"{"events":[],"text":"é"}"#);
    assert!(!codes(&validate(&engine(), request, &[])).contains(&"vendor.alpha.body.wire_limit"));
    // A large capture header cannot inflate the vendor's raw body limit.
    let mut small = capture(r#"{"events":[]}"#);
    small["headers"]["X-Unrelated"] = json!("x".repeat(1000));
    assert!(!codes(&validate(&engine(), small, &[])).contains(&"vendor.alpha.body.wire_limit"));
}

#[test]
fn ndjson_limits_use_original_wire_not_compacted_array() {
    let mut request = capture(&format!("{}{{\"events\":[]}}\n", " ".repeat(60)));
    request["headers"]["Content-Type"] = json!("application/x-ndjson");
    let summary = validate(&engine(), request, &[]);
    assert!(codes(&summary).contains(&"vendor.alpha.body.wire_limit"));
    let mut malformed = capture("{}\n{broken}\n");
    malformed["headers"]["Content-Type"] = json!("application/x-ndjson");
    assert!(codes(&validate(&engine(), malformed, &[])).contains(&"core.request.invalid_ndjson"));
}

#[test]
fn basic_auth_and_capture_findings_never_echo_secrets_or_synthetic_offsets() {
    for authorization in [
        "Basic VG9wU2VjcmV0Og==",
        "Basic\tVG9wU2VjcmV0Og==",
        "  Basic  VG9wU2VjcmV0Og==\t",
    ] {
        let mut request = capture(r#"{"events":[],"left":"one","right":"two"}"#);
        // Base64("TopSecret:") makes the lowercase-only username fixture fail.
        request["headers"]["Authorization"] = json!(authorization);
        let summary = validate(&engine(), request, &[]);
        let rendered = serde_json::to_string(&summary).unwrap();
        assert!(!rendered.contains("TopSecret"), "{rendered}");
        assert!(!rendered.contains("VG9wU2VjcmV0Og=="));
        assert!(codes(&summary).contains(&"vendor.alpha.http.equal"));
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .all(|violation| violation.targets.is_empty())
        );
    }
}

#[test]
fn missing_or_macro_scalar_does_not_claim_cross_field_mismatch() {
    for body in [
        r#"{"events":[],"left":"one"}"#,
        r#"{"events":[],"left":"[VALUE]","right":"two"}"#,
    ] {
        assert!(
            !codes(&validate(&engine(), capture(body), &[])).contains(&"vendor.alpha.http.equal")
        );
    }
}

#[test]
fn form_fallback_preserves_query_alias_and_path_authority() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/form", "display_name":"Form", "description":"Form precedence fixture",
        "docs":"https://example.org/form", "match":{"hosts":["form.example"],"path_prefixes":["/event/"]},
        "path_pattern":"^/event/(?P<id>[0-9]+)$",
        "params":[{"name":"id","aliases":["event_id"],"requirement":"required","format":{"kind":"regex","pattern":"^[0-9]+$"}},
            {"name":"key","aliases":["api_key"],"requirement":"required","format":{"kind":"regex","pattern":"^query$"}},
            {"name":"item","requirement":"required","format":{"kind":"enum","values":["one"]}}]
    }).to_string()).unwrap();
    let summary = validate(
        &engine,
        json!({
            "url":"https://form.example/event/123?api_key=query", "method":"POST",
            "headers":{"Content-Type":"application/x-www-form-urlencoded; charset=UTF-8"},
            "body":"id=invalid&key=body&item=one&item=two"
        }),
        &[],
    );
    assert!(
        !codes(&summary).contains(&"vendor.form.param.id.invalid"),
        "{summary:?}"
    );
    assert!(!codes(&summary).contains(&"vendor.form.param.key.invalid"));
    assert!(codes(&summary).contains(&"vendor.form.param.item.invalid"));
    let missing_path = validate(
        &engine,
        json!({
            "url":"https://form.example/event/?api_key=query", "method":"POST",
            "headers":{"Content-Type":"application/x-www-form-urlencoded"},
            "body":"id=123&item=one"
        }),
        &[],
    );
    assert!(codes(&missing_path).contains(&"vendor.form.param.id.missing"));
    for (url, body) in [
        (
            "https://form.example/event/?api_key=query",
            "event_id=123&item=one",
        ),
        (
            "https://form.example/event/?api_key=query&event_id=123",
            "item=one",
        ),
        (
            "https://form.example/event/?api_key=query&id=123",
            "item=one",
        ),
    ] {
        let summary = validate(
            &engine,
            json!({"url":url,"method":"POST",
            "headers":{"Content-Type":"application/x-www-form-urlencoded"},"body":body}),
            &[],
        );
        assert!(
            codes(&summary).contains(&"vendor.form.param.id.missing"),
            "{summary:?}"
        );
    }
}

#[test]
fn form_encoded_source_contracts_run_without_native_json_root_checks() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/encoded", "display_name":"Encoded", "description":"Encoded form fixture",
        "docs":"https://example.org/encoded", "match":{"hosts":["encoded.example"],"json_paths":["native_only"]},
        "params":[{"name":"data","requirement":"required"}],
        "body":[{"params":[{"name":"native_only","requirement":"required"}]},
            {"source_param":"data","encoding":"json","params":[{"name":"event","requirement":"required"}]}]
    }).to_string()).unwrap();
    let summary = validate(
        &engine,
        json!({"url":"https://encoded.example/track", "method":"POST",
        "headers":{"Content-Type":"application/x-www-form-urlencoded"}, "body":"data=%7B%7D"}),
        &[],
    );
    assert!(codes(&summary).contains(&"vendor.encoded.body.event.missing"));
    assert!(!codes(&summary).contains(&"vendor.encoded.body.native_only.missing"));
}

#[test]
fn json_entity_under_text_or_curl_form_mime_retains_native_checks() {
    for mime in ["text/plain", "application/x-www-form-urlencoded"] {
        let mut request = capture(r#"{"left":"one","right":"two"}"#);
        request["headers"]["Content-Type"] = json!(mime);
        let summary = validate(&engine(), request, &[]);
        assert!(
            codes(&summary).contains(&"vendor.alpha.body.events.missing"),
            "{summary:?}"
        );
        assert!(codes(&summary).contains(&"vendor.alpha.http.equal"));
        assert!(codes(&summary).contains(&"vendor.alpha.http.content_type.invalid"));
    }
}

#[test]
fn unavailable_body_does_not_invent_missing_fields_but_checks_known_path() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/unavailable", "display_name":"Unavailable", "description":"Partial capture fixture",
        "docs":"https://example.org/unavailable", "match":{"hosts":["form.example"],"path_prefixes":["/event/"]},
        "path_pattern":"^/event/(?P<id>[0-9]+)$",
        "params":[{"name":"id","requirement":"required"},{"name":"key","requirement":"required"}],
        "http":{"params":[{"name":"method","format":{"kind":"enum","values":["POST"]}},
            {"name":"body.secret","requirement":"required"},{"name":"body.left"},{"name":"body.right"}],
            "rules":[{"code":"vendor.unavailable.http.equal","kind":"equal_values","left":"body.left","right":"body.right",
                "severity":"error","message":"Observable values must agree."}]}
    }).to_string()).unwrap();
    let valid = json!({"url":"https://form.example/event/123", "method":"POST",
        "headers":{"Content-Type":"multipart/form-data; boundary=boundary"}, "body":"opaque"});
    let summary = validate(&engine, valid.clone(), &[]);
    assert_eq!(codes(&summary), ["core.request.unsupported_body_encoding"]);
    let mut bad_path = valid;
    bad_path["url"] = json!("https://form.example/event/");
    let summary = validate(&engine, bad_path, &[]);
    assert!(codes(&summary).contains(&"vendor.unavailable.param.id.missing"));
    assert!(!codes(&summary).contains(&"vendor.unavailable.param.key.missing"));
    assert!(!codes(&summary).contains(&"vendor.unavailable.http.body.secret.missing"));
}

#[test]
fn request_body_time_windows_share_explicit_validation_clock() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/clock", "display_name":"Clock", "description":"Request clock fixture",
        "docs":"https://example.org/clock", "match":{"hosts":["alpha.example"],"json_paths":["time"]},
        "body":{"params":[{"name":"time"}],"rules":[{"code":"vendor.clock.body.age","kind":"time_window",
            "param":"time","unit":"seconds","max_age_seconds":60,"severity":"error","message":"Event too old."}]},
        "http":{"params":[{"name":"body.time"}],"rules":[{"code":"vendor.clock.http.age","kind":"time_window",
            "param":"body.time","unit":"seconds","max_age_seconds":60,"severity":"error","message":"Event too old."}]}
    }).to_string()).unwrap();
    assert!(validate(&engine, capture(r#"{"time":1799999940}"#), &[]).is_ok());
    let summary = validate(&engine, capture(r#"{"time":1799999939}"#), &[]);
    assert_eq!(
        codes(&summary),
        ["vendor.clock.body.age", "vendor.clock.http.age"]
    );
}

#[test]
fn fragment_question_mark_cannot_supply_http_query_discriminators() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/query", "display_name":"Query", "description":"Query boundary fixture",
        "docs":"https://example.org/query", "match":{"hosts":["alpha.example"]},
        "http":{"condition":{"kind":"present","param":"query.id"},"params":[
            {"name":"query.id"},{"name":"method","format":{"kind":"enum","values":["GET"]}}]}
    }).to_string()).unwrap();
    let mut request = capture("{}");
    request["url"] = json!("https://alpha.example/events#?id=fragment-only");
    let summary = validate(&engine, request.clone(), &[]);
    assert!(
        !codes(&summary).contains(&"vendor.query.http.method.invalid"),
        "{summary:?}"
    );
    request["url"] = json!("https://alpha.example/events?id=real-query#fragment");
    assert!(codes(&validate(&engine, request, &[])).contains(&"vendor.query.http.method.invalid"));
}

#[test]
fn normalized_wrapper_does_not_reduce_the_entity_parser_depth_limit() {
    let body = format!(
        r#"{{"events":[],"nested":{}0{}}}"#,
        "[".repeat(63),
        "]".repeat(63)
    );
    let mut request = capture(&body);
    request["method"] = json!("GET");
    let summary = validate(&engine(), request, &[]);
    assert!(!codes(&summary).contains(&"core.json.parse_error"));
    assert!(
        codes(&summary).contains(&"vendor.alpha.http.method.invalid"),
        "{summary:?}"
    );
    let too_deep = format!(
        r#"{{"events":[],"nested":{}0{}}}"#,
        "[".repeat(65),
        "]".repeat(65)
    );
    let mut request = capture(&too_deep);
    request["method"] = json!("GET");
    let summary = validate(&engine(), request, &[]);
    assert!(codes(&summary).contains(&"core.json.parse_error"));
    assert!(codes(&summary).contains(&"vendor.alpha.http.method.invalid"));
}

#[test]
fn bulk_query_items_reuse_url_rules_and_keep_independent_inheritance() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/bulk", "display_name":"Bulk", "description":"Bulk query fixture",
        "docs":"https://example.org/bulk", "match":{"hosts":["bulk.example"]},
        "params":[{"name":"id","requirement":"required","format":{"kind":"integer"}},
            {"name":"key","aliases":["api_key"],"requirement":"required","format":{"kind":"enum","values":["fallback"]}}],
        "http_queries":[{"source_field":"body.requests[]","inherited_params":{"key":"body.key"}}],
        "http":{"params":[{"name":"body.requests","requirement":"required","json_type":"array"},
            {"name":"body.requests[]","json_type":"string"}]}
    }).to_string()).unwrap();
    let capture = json!({"url":"https://bulk.example/hit?key=outer-query", "method":"POST",
        "headers":{"Content-Type":"application/json"},
        "body":json!({"key":"fallback","requests":["?id=1","id=2&api_key=wrong","?id=bad","?key=fallback","?id=3&key=fallback"]}).to_string()});
    let summary = validate(&engine, capture, &[]);
    let findings: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .collect();
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding.code == "vendor.bulk.param.id.missing")
            .count(),
        1
    );
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding.code == "vendor.bulk.param.id.invalid")
            .count(),
        1
    );
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding.code == "vendor.bulk.param.key.invalid")
            .count(),
        1
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.field.as_deref() == Some("http.body.requests[1].param.key"))
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.code == "vendor.bulk.param.key.missing")
    );
    for requests in [json!([]), json!(false)] {
        let summary = validate(
            &engine,
            json!({"url":"https://bulk.example/hit", "method":"POST",
            "headers":{"Content-Type":"application/json"}, "body":json!({"requests":requests}).to_string()}),
            &[],
        );
        assert!(!codes(&summary).contains(&"vendor.bulk.param.id.missing"));
    }
}

#[test]
fn bulk_url_queries_preserve_fragment_binding_and_envelope_precedence() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/bulk", "display_name":"Bulk", "description":"Bulk URL source fixture",
        "docs":"https://example.org/bulk", "match":{"hosts":["bulk.example"]},
        "params":[{"name":"id","requirement":"required","format":{"kind":"integer"}},
            {"name":"key","aliases":["api_key"],"requirement":"required","format":{"kind":"enum","values":["fallback"]}}],
        "http_queries":[{"source_field":"body.requests[]","encoding":"url_query","inherited_overrides":true,"inherited_params":{"key":"body.key"}}],
        "http":{"params":[{"name":"body.requests","json_type":"array"},{"name":"body.requests[]","json_type":["string","object"],"allow_empty":true}]}
    }).to_string()).unwrap();
    let summary = validate(
        &engine,
        json!({"url":"https://bulk.example/hit", "method":"POST", "headers":{"Content-Type":"application/json"},
        "body":json!({"key":"fallback","requests":["https://other.example/path?id=1&api_key=wrong#id=broken","?id=2","", "id=ignored", {}]}).to_string()}),
        &[],
    );
    assert!(summary.is_ok(), "{summary:?}");
    assert_eq!(
        codes(&summary),
        ["vendor.bulk.http.query_source.unvalidated"]
    );
    assert!(
        summary
            .reports
            .iter()
            .all(|report| report.plugin_id != "vendor/beta")
    );
    let request = ValidationRequest { artifact_kind:ArtifactKind::NetworkRequest,
        artifact:json!({"url":"https://bulk.example/hit","method":"POST","headers":{"Content-Type":"application/json"},
            "body":json!({"key":"fallback","requests":["?id=1&gdpr=1"]}).to_string()}).to_string(), claimed_vendor:None, expansion_state:ExpansionState::Fired };
    let with_core = engine
        .validate_at(&request, &ValidationOptions::default(), 1_800_000_000)
        .unwrap();
    assert!(codes(&with_core).contains(&"core.privacy.gdpr_consent_missing"));
    let without_core = engine
        .validate_at(
            &request,
            &ValidationOptions {
                only_rulepacks: vec![],
                except_rulepacks: vec!["core".into()],
            },
            1_800_000_000,
        )
        .unwrap();
    assert!(!codes(&without_core).contains(&"core.privacy.gdpr_consent_missing"));
}

#[test]
fn url_path_bound_excludes_long_domain_query_and_fragment() {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({
        "id":"vendor/path", "display_name":"Path", "description":"Path bound fixture",
        "docs":"https://example.org/path", "match":{"hosts":["path.example"],"json_paths":["url"]},
        "body":{"params":[{"name":"url"}],"rules":[{"code":"vendor.path.length","kind":"url_path_length",
            "param":"url","max_length":4,"severity":"error","message":"Path too long."}]}
    }).to_string()).unwrap();
    for (url, invalid) in [
        ("https://long.example/abc?verylongquery=123#fragment", false),
        ("app://screens/abc", false),
        ("/abcd", true),
        ("/ééé", false),
        ("/éééé", true),
    ] {
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::JsonPayload,
                    artifact: json!({"url":url}).to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        assert_eq!(
            codes(&summary).contains(&"vendor.path.length"),
            invalid,
            "{url}: {summary:?}"
        );
    }
}
