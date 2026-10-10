//! Static source expectations and independently produced captured wire bytes.

use pixellint_core::{
    ArtifactKind, BinaryHttpRequest, CoreRulePack, Engine, ExpansionState, HttpBodyAvailability,
    HttpCaptureContext, HttpHeaders, HttpRequest, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    code: String,
    severity: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    artifact: String,
    expected_findings: Vec<Finding>,
    reference_unix_seconds: i64,
    rulepacks: Vec<String>,
    source_url: String,
}

#[test]
fn independent_compressed_capture_source_cases() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-mixpanel-import/source-cases.json"
    ))
    .unwrap();
    let engine = Engine::default();
    assert!(cases.len() >= 60);
    for case in cases {
        assert!(case.source_url.starts_with("https://"));
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: case.artifact,
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions {
                    only_rulepacks: case.rulepacks,
                    except_rulepacks: vec![],
                },
                case.reference_unix_seconds,
            )
            .unwrap();
        let mut actual: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .map(|finding| Finding {
                code: finding.code.clone(),
                severity: match finding.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "info",
                }
                .into(),
            })
            .collect();
        let mut expected = case.expected_findings;
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{}: {summary:?}", case.id);
        for finding in summary.reports.iter().flat_map(|report| &report.violations) {
            assert!(
                finding.targets.is_empty(),
                "Capture offsets must not be synthesized"
            );
            assert!(!finding.message.contains("TEST_ONLY_PROJECT_TOKEN"));
            assert!(!finding.message.contains("TEST_ONLY_PASSWORD"));
        }
    }
}

fn run(engine: &Engine, capture: Value, packs: &[&str]) -> Vec<String> {
    let summary = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: capture.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions {
                only_rulepacks: packs.iter().map(|pack| (*pack).into()).collect(),
                except_rulepacks: vec![],
            },
            1791600000,
        )
        .unwrap();
    let serialized = serde_json::to_string(&summary).unwrap();
    for marker in ["PRIVATE_FORM_SECRET", "PRIVATE_JSON_SECRET"] {
        assert!(
            !serialized.contains(marker),
            "Finding output leaked a synthetic credential"
        );
    }
    summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| finding.code.clone())
        .collect()
}

#[test]
fn public_raw_struct_literals_and_binary_serializer_remain_independent() {
    let raw = HttpRequest {
        url: "https://example.org/".into(),
        method: "POST".into(),
        headers: HttpHeaders::Object(Default::default()),
        body: Some("original".into()),
    };
    let raw_value = serde_json::to_value(raw).unwrap();
    assert_eq!(raw_value["body"], "original");
    assert!(raw_value.get("body_base64").is_none());
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-mixpanel-import/source-cases.json"
    ))
    .unwrap();
    let value: Value = serde_json::from_str(
        &cases
            .iter()
            .find(|case| case.id == "gzip_json_python")
            .unwrap()
            .artifact,
    )
    .unwrap();
    let mut binary: BinaryHttpRequest = serde_json::from_value(value).unwrap();
    binary.capture = Some(HttpCaptureContext {
        body: Some(HttpBodyAvailability::Available),
        ..Default::default()
    });
    let encoded = serde_json::to_value(binary).unwrap();
    assert!(encoded.get("body").is_none());
    assert!(
        run(
            &Engine::default(),
            encoded,
            &["core", "vendor/mixpanel-import"]
        )
        .is_empty()
    );
}

#[test]
fn compressed_ndjson_uses_entity_bytes_and_preserves_original_content_length() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-mixpanel-import/source-cases.json"
    ))
    .unwrap();
    let mut capture: Value = serde_json::from_str(
        &cases
            .iter()
            .find(|case| case.id == "gzip_ndjson")
            .unwrap()
            .artifact,
    )
    .unwrap();
    let oracle: Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-mixpanel-import/compression-oracle.json"
    ))
    .unwrap();
    let entry = oracle["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == "gzip_ndjson")
        .unwrap();
    let wire = entry["wire_bytes"].as_u64().unwrap();
    let entity = entry["entity_bytes"].as_u64().unwrap();
    assert!(
        wire < entity - 1,
        "The independent fixture distinguishes compressed and entity lengths"
    );
    capture["headers"]["Content-Length"] = json!(wire.to_string());
    for (bound, expected) in [
        (entity, vec![]),
        (
            entity - 1,
            vec!["vendor.oracle.body.entity_limit".to_string()],
        ),
    ] {
        let mut engine = Engine::new();
        engine.register(CoreRulePack::default());
        engine.register_manifest_json(&json!({
            "id":"vendor/oracle", "display_name":"Independent entity accounting", "description":"Local byte accounting assertion fixture",
            "docs":"https://www.rfc-editor.org/rfc/rfc1952",
            "match":{"hosts":["api.mixpanel.com"], "paths":["/import"], "json_paths":["[].event"]},
            "body":{"rules":[{"code":"vendor.oracle.body.entity_limit", "kind":"max_body_bytes", "max_bytes":bound, "severity":"error", "message":"Synthetic entity byte bound"}]},
            "http":{"params":[
                {"name":"wire_body_bytes", "requirement":"required", "json_type":"integer", "format":{"kind":"enum", "values":[wire.to_string()]}},
                {"name":"entity_body_bytes", "requirement":"required", "json_type":"integer", "format":{"kind":"enum", "values":[entity.to_string()]}},
                {"name":"headers.content-length", "requirement":"required", "format":{"kind":"enum", "values":[wire.to_string()]}},
                {"name":"headers.content-encoding", "requirement":"required", "format":{"kind":"enum", "values":["gzip"]}}
            ]}
        }).to_string()).unwrap();
        assert_eq!(
            run(&engine, capture.clone(), &["core", "vendor/oracle"]),
            expected
        );
    }
}

#[test]
fn decoded_form_preserves_query_precedence_and_credential_redaction() {
    // Python gzip.compress of required=body&api_key=PRIVATE_FORM_SECRET.
    let capture = json!({"url":"https://form.example/event?required=query", "method":"POST", "headers":{"Content-Type":"application/x-www-form-urlencoded", "Content-Encoding":"gzip"},
        "body_base64":"H4sIAAAAAAAC/ytKLSzNLEpNsU3KT6lUSyzIjM9OrbQNCPIMcwxxjXfzD/KND3Z1DnINAQD6Q61fKQAAAA=="});
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({"id":"vendor/form", "display_name":"Form fixture", "description":"Query and decoded form precedence",
        "docs":"https://www.rfc-editor.org/rfc/rfc1952",
        "match":{"hosts":["form.example"], "paths":["/event"]}, "params":[
            {"name":"required", "requirement":"required", "format":{"kind":"enum", "values":["query"]}},
            {"name":"api_key", "requirement":"required", "format":{"kind":"enum", "values":["allowed"]}}]}).to_string()).unwrap();
    assert_eq!(
        run(&engine, capture, &["core", "vendor/form"]),
        ["vendor.form.param.api_key.invalid"]
    );
}

#[test]
fn newly_decoded_json_credentials_are_redacted_from_custom_findings() {
    // Python gzip.compress of {"api_key":"PRIVATE_JSON_SECRET"}.
    let capture = json!({"url":"https://json.example/event", "method":"POST", "headers":{"Content-Type":"application/json", "Content-Encoding":"gzip"},
        "body_base64":"H4sIAAAAAAAC/6tWSizIjM9OrVSyUgoI8gxzDHGN9wr294sPdnUOcg1RqgUAeiMh5SEAAAA="});
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register_manifest_json(&json!({"id":"vendor/json", "display_name":"JSON fixture", "description":"Newly decoded credential redaction",
        "docs":"https://www.rfc-editor.org/rfc/rfc1952",
        "match":{"hosts":["json.example"], "paths":["/event"], "json_paths":["api_key"]},
        "body":{"params":[{"name":"api_key", "requirement":"required", "format":{"kind":"enum", "values":["allowed"]}}]}}).to_string()).unwrap();
    assert_eq!(
        run(&engine, capture, &["core", "vendor/json"]),
        ["vendor.json.body.api_key.invalid"]
    );
}
