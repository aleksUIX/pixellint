use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest, ValidationSummary,
};
use serde_json::{Value, json};

fn check(kind: ArtifactKind, artifact: String) -> ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Template,
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

fn quora(capture: Value) -> ValidationSummary {
    check(ArtifactKind::NetworkRequest, capture.to_string())
}

#[test]
fn unavailable_quora_capture_fields_do_not_become_vendor_errors() {
    let result = quora(json!({
        "url":"https://api.quora.com/ads/v0/conversion",
        "method":"POST", "headers":{},
        "capture":{"headers_unavailable":true,"body":"unavailable"}
    }));
    assert!(
        result
            .reports
            .iter()
            .any(|r| r.plugin_id == "vendor/quora-conversions-api")
    );
    assert!(
        result
            .reports
            .iter()
            .filter(|r| r.plugin_id == "vendor/quora-conversions-api")
            .all(|r| r.violations.is_empty()),
        "{result:?}"
    );
    assert!(
        result
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code.starts_with("core.request."))
    );
}

#[test]
fn redacted_quora_auth_does_not_hide_a_known_wrong_method() {
    let result = quora(json!({
        "url":"https://api.quora.com/ads/v0/conversion", "method":"GET",
        "headers":{"Content-Type":"application/json","Authorization":"[REDACTED]"},
        "body":"{\"account_id\":123,\"conversion\":{\"event_name\":\"Purchase\",\"click_id\":\"synthetic\"},\"user\":{}}",
        "capture":{"redacted_headers":["Authorization"]}
    }));
    let codes: Vec<_> = result
        .reports
        .iter()
        .filter(|r| r.plugin_id == "vendor/quora-conversions-api")
        .flat_map(|r| r.violations.iter().map(|v| v.code.as_str()))
        .collect();
    assert_eq!(codes, ["vendor.quora-conversions-api.http.method.invalid"]);
    assert!(result.is_ok());
}

#[test]
fn quora_json_payload_does_not_invent_http_capture_evidence() {
    let result = check(ArtifactKind::JsonPayload,
        json!({"account_id":123,"conversion":{"event_name":"Purchase","click_id":"synthetic"},"user":{}}).to_string());
    assert!(
        result
            .reports
            .iter()
            .any(|r| r.plugin_id == "vendor/quora-conversions-api")
    );
    assert!(
        result.reports.iter().all(|r| r.violations.is_empty()),
        "{result:?}"
    );
}

#[test]
fn cm360_duplicate_warnings_target_each_assigned_occurrence() {
    for values in [
        "dc_lat=0;dc_lat=1",
        "dc_lat=;dc_lat=",
        "dc_lat=[LAT];%64c_lat=[LAT]",
    ] {
        let result = check(
            ArtifactKind::Url,
            format!(
                "https://ad.doubleclick.net/ddm/trackimp/N1.B1;dc_trk_aid=123;dc_trk_cid=456;{values};"
            ),
        );
        let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(findings.len(), 1, "{result:?}");
        assert_eq!(
            findings[0].code,
            "vendor.cm360-tracking-ad.duplicate.dc_lat"
        );
        assert_eq!(findings[0].targets.len(), 2, "{result:?}");
        assert!(result.is_ok());
    }
}

#[test]
fn unsupported_quora_encoding_remains_an_explicit_capture_limit() {
    let result = quora(json!({
        "url":"https://api.quora.com/ads/v0/conversion", "method":"POST",
        "headers":{"Content-Type":"application/json","Authorization":"Bearer synthetic","Content-Encoding":"custom"},
        "body":"opaque encoded bytes"
    }));
    let codes: Vec<_> = result
        .reports
        .iter()
        .flat_map(|r| r.violations.iter().map(|v| v.code.as_str()))
        .collect();
    assert!(
        codes.iter().any(|c| c.starts_with("core.request.")),
        "{result:?}"
    );
    assert!(
        !codes.contains(&"vendor.quora-conversions-api.http.body_encoding.invalid"),
        "{result:?}"
    );
}

#[test]
fn neighboring_routes_keep_explicit_uncovered_status() {
    for url in [
        "https://ad.doubleclick.net/ddm/trackimposter/N1.B1",
        "https://ad.doubleclick.net/other/trackimp/N1.B1",
        "https://api.quora.com/ads/v0/conversion/extra",
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
