//! Capture uncertainty must preserve known facts without inventing body evidence.
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, HarImportOptions, ValidationOptions, ValidationRequest,
    import_har,
};
use serde_json::{Value, json};

fn xdr_capture(id: &str) -> Value {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-flashtalking-ftrack-depth/source-cases.json"
    ))
    .unwrap();
    serde_json::from_str(
        cases.iter().find(|case| case["id"] == id).unwrap()["artifact"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn matomo_capture() -> Value {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-matomo-php-scalar-depth/source-cases.json"
    ))
    .unwrap();
    serde_json::from_str(
        cases
            .iter()
            .find(|case| case["id"] == "php-executed-01")
            .unwrap()["artifact"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn assert_codes(envelope: &Value, expected: &[&str]) {
    let mut found = codes(envelope);
    found.sort();
    let mut expected = expected.to_vec();
    expected.sort();
    assert_eq!(found, expected, "capture: {envelope}");
}

fn codes(envelope: &Value) -> Vec<String> {
    Engine::default()
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: envelope.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
            1_800_000_000,
        )
        .unwrap()
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.clone())
        .collect()
}

#[test]
fn observed_absent_mime_keeps_xdr_body_checks_with_partial_credential_capture() {
    for capture in [
        json!({"body":"available"}),
        json!({"body":"available","unavailable_headers":["authorization","cookie"]}),
    ] {
        let mut envelope = xdr_capture("xdr-observed-missing-device-time");
        envelope["capture"] = capture.clone();
        let expected = if capture.get("unavailable_headers").is_some() {
            vec![
                "core.request.capture_incomplete",
                "vendor.flashtalking-ftrack.body.D9_1.missing",
            ]
        } else {
            vec!["vendor.flashtalking-ftrack.body.D9_1.missing"]
        };
        assert_eq!(codes(&envelope), expected);
    }
}

#[test]
fn unknown_or_redacted_mime_never_supplies_a_headerless_form_hint() {
    for capture in [
        json!({"body":"available","unavailable_headers":["CONTENT-TYPE"]}),
        json!({"body":"available","redacted_headers":["Content-Type"]}),
        json!({"body":"available","headers_unavailable":true}),
    ] {
        let mut envelope = xdr_capture("xdr-observed-missing-device-time");
        envelope["capture"] = capture;
        assert_eq!(codes(&envelope), ["core.request.capture_incomplete"]);
    }
    // Unknown body context cannot suppress an observed method violation.
    let mut envelope = xdr_capture("xdr-observed-missing-device-time");
    envelope["capture"] = json!({"body":"available","unavailable_headers":["content-type"]});
    envelope["method"] = json!("GET");
    assert_eq!(
        codes(&envelope),
        [
            "core.request.capture_incomplete",
            "vendor.flashtalking-ftrack.http.method.invalid"
        ]
    );
}

#[test]
fn unavailable_redacted_and_compressed_xdr_entities_remain_explicit() {
    for state in ["unavailable", "redacted"] {
        let mut envelope = xdr_capture("xdr-observed-missing-device-time");
        envelope["capture"] = json!({"body":state});
        assert_eq!(codes(&envelope), ["core.request.capture_incomplete"]);
    }
    let mut compressed = xdr_capture("compressed-xdr-entity-remains-unvalidated");
    compressed["capture"] = json!({"body":"available"});
    assert_eq!(
        codes(&compressed),
        ["core.request.unsupported_body_encoding"]
    );
}

#[test]
fn har_default_credential_uncertainty_keeps_observed_missing_xdr_mime_usable() {
    let capture = xdr_capture("xdr-observed-missing-device-time");
    let body = capture["body"].as_str().unwrap();
    let archive = json!({"log":{"version":"1.2","entries":[{"startedDateTime":"2027-01-15T08:00:00Z", "request":{
        "method":capture["method"],"url":capture["url"],"headers":[],"bodySize":body.len(),
        "postData":{"mimeType":"application/x-www-form-urlencoded","text":body}}}]}});
    let imported = import_har(&archive.to_string(), &HarImportOptions::default()).unwrap();
    let report = Engine::default()
        .validate_har_at(&imported, &ValidationOptions::default(), 1_800_000_000)
        .unwrap();
    let found: Vec<_> = report
        .document
        .artifacts
        .iter()
        .flat_map(|artifact| &artifact.reports)
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.as_str())
        .collect();
    assert_eq!(
        found,
        [
            "core.request.capture_incomplete",
            "vendor.flashtalking-ftrack.body.D9_1.missing"
        ]
    );
    assert_eq!(
        report.captures[0].capture.unavailable_headers,
        ["authorization", "cookie"]
    );
}

#[test]
fn unknown_matomo_bulk_body_never_creates_missing_single_event_fields() {
    let base = matomo_capture();
    assert_codes(&base, &["vendor.matomo.param.idsite.invalid"]);
    for state in ["unavailable", "redacted"] {
        for retain_raw in [true, false] {
            let mut envelope = base.clone();
            envelope["capture"] = json!({"body":state});
            if !retain_raw {
                envelope.as_object_mut().unwrap().remove("body");
            }
            assert_codes(&envelope, &["core.request.capture_incomplete"]);
        }
    }
}

#[test]
fn unknown_matomo_body_keeps_observed_method_and_query_errors() {
    for state in ["unavailable", "redacted"] {
        let mut envelope = matomo_capture();
        envelope["capture"] = json!({"body":state});
        envelope["method"] = json!("DELETE");
        assert_codes(
            &envelope,
            &[
                "core.request.capture_incomplete",
                "vendor.matomo.http.method.invalid",
            ],
        );
        envelope["method"] = json!("POST");
        envelope["url"] =
            json!("https://example.matomo.cloud/matomo.php?idsite=0&rec=no&url=relative");
        assert_codes(
            &envelope,
            &[
                "core.request.capture_incomplete",
                "vendor.matomo.param.idsite.invalid",
                "vendor.matomo.param.rec.invalid",
                "vendor.matomo.param.url.invalid",
            ],
        );
    }
}

#[test]
fn unknown_content_encoding_never_interprets_matomo_or_xdr_payloads() {
    for capture in [
        json!({"unavailable_headers":["Content-Encoding"]}),
        json!({"redacted_headers":["CONTENT-ENCODING"]}),
        json!({"headers_unavailable":true}),
    ] {
        for mut envelope in [
            matomo_capture(),
            xdr_capture("xdr-observed-missing-device-time"),
        ] {
            envelope["capture"] = capture.clone();
            assert_codes(&envelope, &["core.request.capture_incomplete"]);
        }
    }
    for mut envelope in [
        matomo_capture(),
        xdr_capture("xdr-observed-missing-device-time"),
    ] {
        envelope["headers"]["Content-Encoding"] = json!("identity");
        envelope["capture"] = json!({"redacted_headers":["content-encoding"]});
        assert_codes(&envelope, &["core.request.capture_incomplete"]);
    }
}

#[test]
fn unknown_encoding_keeps_observed_url_method_and_header_errors() {
    let mut matomo = matomo_capture();
    matomo["capture"] = json!({"unavailable_headers":["Content-Encoding"]});
    matomo["method"] = json!("DELETE");
    matomo["url"] = json!("https://example.matomo.cloud/matomo.php?idsite=0&rec=no&url=relative");
    assert_codes(
        &matomo,
        &[
            "core.request.capture_incomplete",
            "vendor.matomo.http.method.invalid",
            "vendor.matomo.param.idsite.invalid",
            "vendor.matomo.param.rec.invalid",
            "vendor.matomo.param.url.invalid",
        ],
    );
    let mut xdr = xdr_capture("xdr-observed-missing-device-time");
    xdr["capture"] = json!({"unavailable_headers":["content-encoding"]});
    xdr["method"] = json!("GET");
    xdr["headers"]["Content-Type"] = json!("application/json");
    xdr["headers"]["X-Observed"] = json!("bad\r\nvalue");
    assert_codes(
        &xdr,
        &[
            "core.request.capture_incomplete",
            "core.request.invalid_header",
            "vendor.flashtalking-ftrack.http.method.invalid",
            "vendor.flashtalking-ftrack.http.content_type.invalid",
        ],
    );
}

#[test]
fn populated_unavailable_encoding_remains_observed() {
    for capture in [
        json!({"unavailable_headers":["Content-Encoding"]}),
        json!({"headers_unavailable":true}),
    ] {
        let mut matomo = matomo_capture();
        matomo["headers"]["Content-Encoding"] = json!("identity");
        matomo["capture"] = capture.clone();
        let incomplete = capture.get("headers_unavailable").is_some();
        if incomplete {
            assert_codes(
                &matomo,
                &[
                    "core.request.capture_incomplete",
                    "vendor.matomo.param.idsite.invalid",
                ],
            );
        } else {
            assert_codes(&matomo, &["vendor.matomo.param.idsite.invalid"]);
        }
        // All headers unavailable also leaves absent Content-Type unknown.
        if incomplete {
            continue;
        }
        let mut xdr = xdr_capture("xdr-observed-missing-device-time");
        xdr["headers"]["Content-Encoding"] = json!("identity");
        xdr["capture"] = capture.clone();
        assert_codes(&xdr, &["vendor.flashtalking-ftrack.body.D9_1.missing"]);
        for mut envelope in [matomo, xdr] {
            envelope["headers"]["Content-Encoding"] = json!("gzip");
            assert_codes(&envelope, &["core.request.unsupported_body_encoding"]);
        }
    }
}
