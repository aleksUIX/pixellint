//! Independent SDK-produced and boundary oracles for the 0.34 endpoint depth pass.

use std::{fs, path::PathBuf};

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde::Deserialize;
use serde_json::json;

const FTRACK: &str = include_str!("../rulepacks/vendor/flashtalking-ftrack.json");
const HUBSPOT: &str = include_str!("../rulepacks/vendor/hubspot-pixel.json");

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    artifact: String,
    expected_codes: Vec<String>,
    severity: String,
    source_url: String,
}

fn request(kind: ArtifactKind, artifact: String) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

fn codec_engine(max_bytes: Option<usize>) -> Engine {
    let mut manifest = json!({
        "id":"custom/percent", "display_name":"Percent JSON", "source_level":"heuristic",
        "description":"One strict URI-component layer over native JSON.",
        "match":{"hosts":["example.test"],"paths":["/collect"]},
        "params":[{"name":"payload","allow_empty":true}],
        "body":{"source_param":"payload","encoding":"percent_encoded_json",
            "encoding_severity":"warning", "params":[
                {"name":"text","json_type":"string","format":{"kind":"regex","pattern":"^café \\+ 50%$"}}
            ]}
    });
    if let Some(max_bytes) = max_bytes {
        manifest["body"]["rules"] = json!([{
            "code":"custom.percent.bytes", "kind":"max_body_bytes", "max_bytes":max_bytes,
            "severity":"error", "message":"Decoded UTF-8 entity exceeds its exact byte budget."
        }]);
    }
    let mut engine = Engine::new();
    engine
        .register_manifest_json(&manifest.to_string())
        .unwrap();
    engine
}

fn codec_capture(inner: &str) -> String {
    json!({"url":"https://example.test/collect", "method":"POST",
        "headers":{"Content-Type":"application/x-www-form-urlencoded"},
        "body":format!("payload={}",inner.bytes().map(|b| format!("%{b:02X}")).collect::<String>())})
    .to_string()
}

#[test]
fn percent_json_decodes_one_strict_utf8_layer_and_preserves_plus() {
    let engine = codec_engine(None);
    for encoded in [
        "%7B%22text%22%3A%22caf%C3%A9%20%2B%2050%25%22%7D",
        "%7b%22text%22%3a%22café%20+%2050%25%22%7d",
    ] {
        let summary = engine
            .validate_at(
                &request(ArtifactKind::NetworkRequest, codec_capture(encoded)),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        assert!(
            summary.reports.iter().all(|r| r.violations.is_empty()),
            "{encoded}: {summary:?}"
        );
    }
    for encoded in [
        "%",
        "%2",
        "%GG",
        "%FF",
        "%C0%AF",
        "%ED%A0%80",
        "%F4%90%80%80",
        "%257B%2522text%2522%253A%2522bad%2522%257D",
        "%7Bbroken",
    ] {
        let summary = engine
            .validate_at(
                &request(ArtifactKind::NetworkRequest, codec_capture(encoded)),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(violations.len(), 1, "{encoded}: {summary:?}");
        assert_eq!(violations[0].code, "custom.percent.body.payload.invalid");
        assert_eq!(format!("{:?}", violations[0].severity), "Warning");
    }
}

#[test]
fn percent_json_budget_counts_decoded_utf8_and_plain_json_stays_plain() {
    let encoded = "%7B%22text%22%3A%22caf%C3%A9%20%2B%2050%25%22%7D";
    let bytes = "{\"text\":\"café + 50%\"}".len();
    for (budget, expected) in [(bytes, 0), (bytes - 1, 1)] {
        let summary = codec_engine(Some(budget))
            .validate_at(
                &request(ArtifactKind::NetworkRequest, codec_capture(encoded)),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(violations.len(), expected);
        if expected == 1 {
            assert_eq!(violations[0].code, "custom.percent.bytes");
        }
    }
    let mut engine = Engine::new();
    engine
        .register_manifest_json(
            &json!({
                "id":"custom/plain", "display_name":"Plain JSON", "source_level":"heuristic",
                "description":"Plain JSON does not implicitly unwrap percent layers.",
                "match":{"hosts":["example.test"]},"params":[{"name":"payload"}],
                "body":{"source_param":"payload","params":[{"name":"text","json_type":"string"}]}
            })
            .to_string(),
        )
        .unwrap();
    let summary = engine
        .validate_at(
            &request(ArtifactKind::NetworkRequest, codec_capture(encoded)),
            &ValidationOptions::default(),
            1_800_000_000,
        )
        .unwrap();
    assert_eq!(
        summary.reports[0].violations[0].code,
        "custom.plain.body.payload.invalid"
    );
}

#[test]
fn percent_json_macro_deferral_requires_a_valid_encoding_in_every_source() {
    let mut embedded = Engine::new();
    embedded.register_manifest_json(&json!({
        "id":"custom/embedded-percent", "display_name":"Embedded percent JSON",
        "source_level":"heuristic", "description":"Encoded native string fields.",
        "match":{"hosts":["example.test"],"json_paths":[{"path":"vendor","pattern":"^synthetic-percent$"},"payload"]},
        "body":{"source_field":"payload","encoding":"percent_encoded_json",
            "encoding_severity":"warning","params":[{"name":"text","json_type":"string"}]}
    }).to_string()).unwrap();
    let mut nested = Engine::new();
    nested
        .register_manifest_json(
            &json!({
                "id":"custom/nested-percent", "display_name":"Nested percent JSON",
                "source_level":"heuristic", "description":"Encoded strings inside query JSON.",
                "match":{"hosts":["example.test"]},"params":[{"name":"payload"}],
                "body":{"source_param":"payload","source_field":"inner",
                    "field_encoding":"percent_encoded_json","encoding_severity":"warning",
                    "params":[{"name":"text","json_type":"string"}]}
            })
            .to_string(),
        )
        .unwrap();
    let percent = |text: &str| {
        text.bytes()
            .map(|b| format!("%{b:02X}"))
            .collect::<String>()
    };
    for (inner, expected) in [
        ("%5BPAYLOAD%5D", 0),
        ("[PAYLOAD]", 0),
        ("%5BPAYLOAD%5D%GG", 1),
        ("[PAYLOAD]%GG", 1),
        ("%5BPAYLOAD%5D%FF", 1),
    ] {
        let outer = json!({"inner":inner}).to_string();
        let captures = [
            (
                codec_engine(None),
                request(
                    ArtifactKind::Url,
                    format!("https://example.test/collect?payload={}", percent(inner)),
                ),
            ),
            (
                codec_engine(None),
                request(ArtifactKind::NetworkRequest, codec_capture(inner)),
            ),
        ];
        for (engine, capture) in captures {
            let summary = engine
                .validate_at(&capture, &ValidationOptions::default(), 1_800_000_000)
                .unwrap();
            assert_eq!(
                summary.reports.iter().flat_map(|r| &r.violations).count(),
                expected,
                "{}",
                capture.artifact
            );
        }
        for (engine, capture) in [
            (
                &embedded,
                request(
                    ArtifactKind::JsonPayload,
                    json!({"vendor":"synthetic-percent","payload":inner}).to_string(),
                ),
            ),
            (
                &nested,
                request(
                    ArtifactKind::Url,
                    format!("https://example.test/collect?payload={}", percent(&outer)),
                ),
            ),
        ] {
            let summary = engine
                .validate_at(&capture, &ValidationOptions::default(), 1_800_000_000)
                .unwrap();
            assert_eq!(
                summary.reports.iter().flat_map(|r| &r.violations).count(),
                expected,
                "{}",
                capture.artifact
            );
        }
    }
}

#[test]
fn query_json_preserves_form_strings_repeats_literal_keys_and_wire_budget() {
    let raw = "email=one%2Btag%40example.org&email=two%40example.org&name=caf%C3%A9+%2B+50%25&%E5%90%8D%E5%AD%97=%E6%9D%B1%E4%BA%AC&x%5B%5D=true&a.b=42&empty=";
    let mut manifest = json!({
        "id":"custom/query-json", "display_name":"Query JSON", "source_level":"heuristic",
        "description":"URI query serialization preserves native strings and literal keys.",
        "match":{"hosts":["example.test"]},"params":[{"name":"payload","allow_empty":true}],
        "body":[{"source_param":"payload","encoding":"query_params_json","encoding_severity":"warning",
            "params":[{"name":"","json_type":"object"},
                {"name":"email","json_type":"array","min_items":2,"max_items":2},
                {"name":"name","json_type":"string","format":{"kind":"regex","pattern":"^café \\+ 50%$"}},
                {"name":"名字","json_type":"string","format":{"kind":"regex","pattern":"^東京$"}},
                {"name":"[\"x[]\"]","json_type":"string","format":{"kind":"enum","values":["true"]}},
                {"name":"[\"a.b\"]","json_type":"string","format":{"kind":"enum","values":["42"]}},
                {"name":"empty","json_type":"string","allow_empty":true}],
            "rules":[{"code":"custom.query-json.bytes","kind":"max_body_bytes","max_bytes":raw.len(),
                "severity":"error","message":"Original inner query exceeds the byte limit."}]},
            {"source_param":"payload","encoding":"query_params_json","encoding_severity":"warning",
                "scope":"email[]","params":[{"name":"","json_type":"string","format":{"kind":"regex","pattern":"^[^\\s@]+@[^\\s@]+$"}}]}]
    });
    for (budget, expected) in [(raw.len(), 0), (raw.len() - 1, 1)] {
        manifest["body"][0]["rules"][0]["max_bytes"] = json!(budget);
        let mut engine = Engine::new();
        engine
            .register_manifest_json(&manifest.to_string())
            .unwrap();
        let summary = engine
            .validate_at(
                &request(ArtifactKind::NetworkRequest, codec_capture(raw)),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(violations.len(), expected, "{summary:?}");
        if expected == 1 {
            assert_eq!(violations[0].code, "custom.query-json.bytes");
        }
    }
    let mut engine = Engine::new();
    engine.register_manifest_json(&json!({
        "id":"custom/query-invalid", "display_name":"Invalid query JSON", "source_level":"heuristic",
        "description":"Strict escapes, UTF8 and unknown macros.",
        "match":{"hosts":["example.test"]},"params":[{"name":"payload"}],
        "body":{"source_param":"payload","encoding":"query_params_json","encoding_severity":"warning",
            "params":[{"name":"email","json_type":"string","format":{"kind":"regex","pattern":"^[^\\s@]+@[^\\s@]+$"}}]}
    }).to_string()).unwrap();
    for (raw, expected) in [
        ("email=%GG", 1),
        ("%FF=value", 1),
        ("email=%C0%AF", 1),
        ("email=%5BEMAIL%5D", 0),
        ("%5BIDENTITY%5D", 0),
    ] {
        let summary = engine
            .validate_at(
                &request(ArtifactKind::NetworkRequest, codec_capture(raw)),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(violations.len(), expected, "{raw}: {summary:?}");
        if expected == 1 {
            assert_eq!(format!("{:?}", violations[0].severity), "Warning");
        }
    }
}

#[test]
fn ftrack_sdk_examples_and_independent_boundary_cases() {
    let folder =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor-flashtalking-ftrack");
    let cases: Vec<SourceCase> =
        serde_json::from_str(&fs::read_to_string(folder.join("source-cases.json")).unwrap())
            .unwrap();
    let mut engine = Engine::new();
    engine.register_manifest_json(FTRACK).unwrap();
    for case in &cases {
        assert_eq!(case.source_url, "https://d9.flashtalking.com/d9core");
        let summary = engine
            .validate_at(
                &request(
                    match case.kind.as_str() {
                        "request" => ArtifactKind::NetworkRequest,
                        "url" => ArtifactKind::Url,
                        other => panic!("unknown artifact kind {other}"),
                    },
                    case.artifact.clone(),
                ),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let mut actual: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .map(|v| (v.code.clone(), format!("{:?}", v.severity).to_lowercase()))
            .collect();
        let mut expected: Vec<_> = case
            .expected_codes
            .iter()
            .map(|code| (code.clone(), case.severity.clone()))
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{}", case.id);
    }
    assert_eq!(cases.len(), 83);
}

#[test]
fn ftrack_exact_route_does_not_claim_other_private_collectors() {
    let mut engine = Engine::new();
    engine.register_manifest_json(FTRACK).unwrap();
    for url in [
        "https://d9.flashtalking.com/lgc-other",
        "https://d9.flashtalking.com/lgc/subpath",
        "https://d9.flashtalking.com/img/img.png?D9r.DeviceID=true",
        "https://ad-events.flashtalking.com/ft.stat?opaque=signed",
        "https://other.example/lgc",
    ] {
        let result = engine.validate_at(
            &request(ArtifactKind::Url, url.into()),
            &ValidationOptions::default(),
            1_800_000_000,
        );
        assert!(
            matches!(result, Err(pixellint_core::EngineError::NoMatchingPlugin)),
            "{url}: {result:?}"
        );
    }
}

#[test]
fn hubspot_sdk_field_maps_and_independent_conditional_boundaries() {
    let folder =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor-hubspot-pixel");
    let cases: Vec<SourceCase> =
        serde_json::from_str(&fs::read_to_string(folder.join("source-cases.json")).unwrap())
            .unwrap();
    let mut engine = Engine::new();
    engine.register_manifest_json(HUBSPOT).unwrap();
    for case in &cases {
        assert!(
            case.source_url.starts_with("https://js.hs-analytics.net/")
                || case
                    .source_url
                    .starts_with("https://developers.hubspot.com/")
        );
        let summary = engine
            .validate_at(
                &request(
                    match case.kind.as_str() {
                        "request" => ArtifactKind::NetworkRequest,
                        "url" => ArtifactKind::Url,
                        other => panic!("unknown artifact kind {other}"),
                    },
                    case.artifact.clone(),
                ),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let mut actual: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .map(|v| (v.code.clone(), format!("{:?}", v.severity).to_lowercase()))
            .collect();
        let mut expected: Vec<_> = case
            .expected_codes
            .iter()
            .map(|code| (code.clone(), case.severity.clone()))
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{}", case.id);
    }
    assert_eq!(cases.len(), 114);
    for url in [
        "https://track.hubspot.com/__pto.gif",
        "https://track.hubspot.com/__ptq.gif/other",
        "https://other.example/__ptq.gif",
    ] {
        assert!(
            matches!(
                engine.validate_at(
                    &request(ArtifactKind::Url, url.into()),
                    &ValidationOptions::default(),
                    1_800_000_000
                ),
                Err(pixellint_core::EngineError::NoMatchingPlugin)
            ),
            "{url}"
        );
    }
}
