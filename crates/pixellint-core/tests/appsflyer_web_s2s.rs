//! Web endpoint ownership and capture evidence have independent integration controls.
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, HarHeaderPolicy, HarImportOptions, Severity,
    ValidationOptions, ValidationRequest, ValidationSummary, import_har,
};
use serde_json::{Value, json};

const EVENT: &str = "vendor/appsflyer-web-event";
const VISIT: &str = "vendor/appsflyer-web-visit";
const CLOCK: i64 = 1_791_648_000;

fn capture(route: &str, body: Value) -> Value {
    json!({
        "url":format!("https://events.appsflyer.com/v2.0/s2s/{route}/app/web/opaque-dashboard-id"),
        "method":"POST",
        "headers":{"Authorization":"Bearer synthetic_s2s_token","Content-Type":"application/json"},
        "body":body.to_string()
    })
}

fn validate(engine: &Engine, value: &Value, options: &ValidationOptions) -> ValidationSummary {
    engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: value.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            options,
            CLOCK,
        )
        .unwrap()
}

fn web_codes(summary: &ValidationSummary) -> Vec<&str> {
    summary
        .reports
        .iter()
        .filter(|report| [EVENT, VISIT].contains(&report.plugin_id.as_str()))
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.as_str())
        .collect()
}

#[test]
fn default_routing_binds_each_empty_body_to_its_endpoint() {
    let engine = Engine::default();
    for (route, owner, member) in [
        ("inapps", EVENT, "event_name"),
        ("visits", VISIT, "event_url"),
    ] {
        let summary = validate(
            &engine,
            &capture(route, json!({})),
            &ValidationOptions::default(),
        );
        let prefix = owner.replace('/', ".");
        assert_eq!(
            web_codes(&summary),
            [
                format!("{prefix}.http.user_id.missing"),
                format!("{prefix}.http.{member}.missing")
            ]
        );
        assert_eq!(
            summary
                .reports
                .iter()
                .filter(|report| [EVENT, VISIT].contains(&report.plugin_id.as_str()))
                .map(|report| report.plugin_id.as_str())
                .collect::<Vec<_>>(),
            [owner]
        );
    }
}

#[test]
fn bare_json_and_unpublished_routes_never_create_a_web_body_owner() {
    let engine = Engine::default();
    let bare = engine.validate_at(&ValidationRequest {
        artifact_kind: ArtifactKind::JsonPayload,
        artifact: json!({"user_id":{"customer_user_id":"synthetic"},"event_name":"user signed up"}).to_string(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Fired,
    }, &ValidationOptions::default(), CLOCK).unwrap();
    assert!(web_codes(&bare).is_empty());
    assert!(
        bare.reports
            .iter()
            .all(|report| ![EVENT, VISIT].contains(&report.plugin_id.as_str()))
    );
    for url in [
        "https://events.appsflyer.com.example.com/v2.0/s2s/inapps/app/web/app",
        "https://other.appsflyer.com/v2.0/s2s/inapps/app/web/app",
        "https://events.appsflyer.com/v2.0/s2s/inapps/app/web/app/extra",
        "https://events.appsflyer.com/v2.0/s2s/inapps/app/web/app/",
    ] {
        let mut envelope = capture("inapps", json!({}));
        envelope["url"] = json!(url);
        let summary = validate(&engine, &envelope, &ValidationOptions::default());
        assert!(
            summary
                .reports
                .iter()
                .all(|report| ![EVENT, VISIT].contains(&report.plugin_id.as_str())),
            "{url}: {summary:?}"
        );
    }
}

#[test]
fn a_bare_url_cannot_establish_missing_http_fields() {
    let engine = Engine::default();
    for route in ["inapps", "visits"] {
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::Url,
                    artifact: format!(
                        "https://events.appsflyer.com/v2.0/s2s/{route}/app/web/opaque-id"
                    ),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Fired,
                },
                &ValidationOptions::default(),
                CLOCK,
            )
            .unwrap();
        assert!(web_codes(&summary).is_empty());
        assert!(summary.is_ok());
    }
}

#[test]
fn caller_selection_and_exclusion_preserve_route_ownership() {
    let engine = Engine::default();
    let options = ValidationOptions {
        only_rulepacks: vec![EVENT.into()],
        except_rulepacks: vec![],
    };
    let summary = validate(&engine, &capture("visits", json!({})), &options);
    assert_eq!(
        web_codes(&summary),
        ["vendor.appsflyer-web-event.endpoint_mismatch"]
    );
    assert_eq!(summary.reports[0].violations[0].severity, Severity::Info);
    assert_eq!(summary.reports[0].detected_vendor, None);
    let excluded = validate(
        &engine,
        &capture("inapps", json!({})),
        &ValidationOptions {
            only_rulepacks: vec!["core".into(), EVENT.into()],
            except_rulepacks: vec![EVENT.into(), "directory".into()],
        },
    );
    assert!(web_codes(&excluded).is_empty());
    assert_eq!(
        excluded
            .reports
            .iter()
            .map(|report| report.plugin_id.as_str())
            .collect::<Vec<_>>(),
        ["core"]
    );
    assert!(excluded.is_ok());
}

#[test]
fn unknown_entities_preserve_observed_method_and_authentication_errors() {
    let engine = Engine::default();
    let mut envelope = capture("inapps", json!({}));
    envelope.as_object_mut().unwrap().remove("body");
    envelope["capture"] = json!({"body":"unavailable"});
    envelope["method"] = json!("GET");
    envelope["headers"]["Authorization"] = json!("Basic c3ludGhldGljOnRva2Vu");
    let summary = validate(&engine, &envelope, &ValidationOptions::default());
    assert_eq!(
        web_codes(&summary),
        [
            "vendor.appsflyer-web-event.http.method.invalid",
            "vendor.appsflyer-web-event.http.headers.authorization.invalid"
        ]
    );
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .any(|finding| finding.code == "core.request.capture_incomplete")
    );
}

#[test]
fn local_decoder_deferral_and_explicit_binary_entities_stay_separate() {
    let engine = Engine::default();
    let ordinary = capture("inapps", json!({}));
    let expected = web_codes(&validate(&engine, &ordinary, &ValidationOptions::default()))
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for coding in ["gzip", "br"] {
        let mut raw = ordinary.clone();
        raw["headers"]["Content-Encoding"] = json!(coding);
        let summary = validate(&engine, &raw, &ValidationOptions::default());
        assert!(web_codes(&summary).is_empty(), "{summary:?}");
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .any(
                    |finding| finding.code == "core.request.unsupported_body_encoding"
                        && finding.severity == Severity::Info
                )
        );
    }
    // Fixed Python gzip.compress(b"{}", mtime=0) and standard padded Base64 output.
    for (body, coding) in [
        ("e30=", None),
        ("H4sIAAAAAAAC/6uuBQBDv6ajAgAAAA==", Some("gzip")),
    ] {
        let mut binary = ordinary.clone();
        binary.as_object_mut().unwrap().remove("body");
        binary["body_base64"] = json!(body);
        if let Some(coding) = coding {
            binary["headers"]["Content-Encoding"] = json!(coding);
        }
        assert_eq!(
            web_codes(&validate(&engine, &binary, &ValidationOptions::default())),
            expected
        );
    }
}

#[test]
fn har_policies_do_not_turn_sanitized_credentials_into_absent_credentials() {
    let engine = Engine::default();
    let envelope = capture(
        "inapps",
        json!({"user_id":{"appsflyer_id":"synthetic"},"event_name":"af_purchase"}),
    );
    let body = envelope["body"].as_str().unwrap();
    let har = json!({"log":{"version":"1.2","entries":[{"startedDateTime":"2026-10-10T16:00:00Z","request":{
        "method":"POST","url":envelope["url"],"headers":[{"name":"Content-Type","value":"application/json"}],
        "bodySize":body.len(),"postData":{"mimeType":"application/json","text":body}
    }}]}}).to_string();
    for policy in [
        HarHeaderPolicy::Unknown,
        HarHeaderPolicy::ChromeSanitized,
        HarHeaderPolicy::Complete,
    ] {
        let imported = import_har(
            &har,
            &HarImportOptions {
                header_policy: policy,
            },
        )
        .unwrap();
        let report = engine
            .validate_har(&imported, &ValidationOptions::default())
            .unwrap();
        let web = report
            .document
            .artifacts
            .iter()
            .flat_map(|artifact| &artifact.reports)
            .filter(|report| [EVENT, VISIT].contains(&report.plugin_id.as_str()))
            .flat_map(|report| &report.violations)
            .map(|finding| finding.code.as_str())
            .collect::<Vec<_>>();
        if policy == HarHeaderPolicy::Complete {
            assert_eq!(
                web,
                ["vendor.appsflyer-web-event.http.headers.authorization.missing"]
            );
        } else {
            assert!(web.is_empty(), "{report:?}");
        }
    }
}

#[test]
fn web_identity_and_event_grammar_remain_distinct_from_mobile() {
    let engine = Engine::default();
    for user in [
        json!({"customer_user_id":"customer"}),
        json!({"appsflyer_id":"web"}),
        json!({"customer_user_id":"customer","appsflyer_id":"web"}),
    ] {
        for name in ["user signed up", "\"signup", " ", "\n", "é😀"] {
            let summary = validate(
                &engine,
                &capture("inapps", json!({"user_id":user,"event_name":name})),
                &ValidationOptions::default(),
            );
            assert!(web_codes(&summary).is_empty(), "{summary:?}");
        }
    }
    let mobile = json!({"url":"https://api3.appsflyer.com/inappevent/com.example.app","method":"POST","headers":{"authentication":"synthetic_mobile_token","Content-Type":"application/json"},"body":json!({"appsflyer_id":"synthetic","eventName":"af_purchase","eventValue":""}).to_string()});
    let summary = validate(&engine, &mobile, &ValidationOptions::default());
    assert!(web_codes(&summary).is_empty());
    assert!(
        summary
            .reports
            .iter()
            .any(|report| report.plugin_id == "vendor/appsflyer")
    );
    assert!(summary.is_ok(), "{summary:?}");
}

#[test]
fn int64_limits_are_checked_without_floating_point_rounding() {
    let engine = Engine::default();
    for (number, valid) in [
        ("-9223372036854775808", true),
        ("9223372036854775807", true),
        ("-9223372036854775809", false),
        ("9223372036854775808", false),
        ("9223372036854775807.0", true),
    ] {
        let mut envelope = capture("inapps", json!({}));
        envelope["body"] = json!(format!(
            "{{\"user_id\":{{\"appsflyer_id\":\"synthetic\"}},\"event_name\":\"af_purchase\",\"timestamp\":{number}}}"
        ));
        let summary = validate(&engine, &envelope, &ValidationOptions::default());
        assert_eq!(
            web_codes(&summary).is_empty(),
            valid,
            "{number}: {summary:?}"
        );
        if !valid {
            assert_eq!(
                web_codes(&summary),
                ["vendor.appsflyer-web-event.http.timestamp.invalid"]
            );
        }
    }
}

#[test]
fn visit_unknown_fields_and_custom_parameters_are_open() {
    let engine = Engine::default();
    let summary = validate(
        &engine,
        &capture(
            "visits",
            json!({
                "user_id":{"customer_user_id":"synthetic"},"event_url":"https://example.com/page",
                "event_name":"a+b","event_revenue":"unconstrained","event_revenue_currency":"ZZZ",
                "event_value":{"custom_parameters":{"event_name":"a+b","ip":{},"email_hashed":false}},"unknown":{"n":[null,true]}
            }),
        ),
        &ValidationOptions::default(),
    );
    assert!(web_codes(&summary).is_empty(), "{summary:?}");
    assert!(summary.is_ok());
}
