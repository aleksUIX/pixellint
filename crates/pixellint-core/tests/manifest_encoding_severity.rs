//! Encoded-source syntax severity, guarded representations and original targets.

use pixellint_core::{
    ArtifactKind, ExpansionState, ManifestRulePack, Severity, ValidationRequest, ValidatorPlugin,
    ViolationTargetComponent,
};
use serde_json::{Value, json};

fn pack(body: Value, native: bool) -> ManifestRulePack {
    let mut matcher = json!({"hosts":["example.test"]});
    if native {
        matcher["json_paths"] = json!(["payload"]);
    }
    ManifestRulePack::from_json(
        &json!({"id":"custom/encoding", "display_name":"Encoding severity", "description":"Encoded source syntax checks.",
            "source_level":"heuristic", "match":matcher,
            "params":[{"name":"payload"},{"name":"mode"}], "body":body})
        .to_string(),
    )
    .unwrap()
}

fn request(artifact: String, native: bool) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: if native {
            ArtifactKind::JsonPayload
        } else {
            ArtifactKind::Url
        },
        artifact,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

fn url(payload: &str, mode: &str) -> String {
    url::Url::parse_with_params(
        "https://example.test/pixel",
        [("payload", payload), ("mode", mode)],
    )
    .unwrap()
    .to_string()
}

#[test]
fn url_syntax_defaults_to_error_and_accepts_explicit_advisory_levels() {
    for (encoding, invalid) in [
        ("json", "{\"n\":}"),
        ("base64_json", "@@@"),
        ("base64_json", "e2JhZH0="),
        ("base64_latin1_json", "e2JhZH0="),
    ] {
        for (configured, expected) in [
            (None, Severity::Error),
            (Some("warning"), Severity::Warning),
            (Some("info"), Severity::Info),
        ] {
            let mut body = json!({"source_param":"payload","encoding":encoding});
            if let Some(severity) = configured {
                body["encoding_severity"] = json!(severity);
            }
            let artifact = url(invalid, "test");
            let report = pack(body, false).validate(&request(artifact.clone(), false));
            assert_eq!(report.violations.len(), 1);
            let finding = &report.violations[0];
            assert_eq!(finding.code, "custom.encoding.body.payload.invalid");
            assert_eq!(finding.severity, expected);
            let target = &finding.targets[0];
            assert_eq!(target.component, ViolationTargetComponent::QueryParam);
            assert_eq!(target.name.as_deref(), Some("payload"));
            assert_eq!(target.value.as_deref(), Some(invalid));
            assert!(artifact[target.start..target.end].starts_with("payload="));
        }
    }
}

#[test]
fn strongest_active_url_severity_is_order_independent_and_encoding_specific() {
    let warning = json!({"source_param":"payload","encoding_severity":"warning"});
    let guarded = json!({"source_param":"payload","encoding_severity":"error",
        "condition":{"kind":"value_in","param":"mode","values":["strict"]}});
    let other_encoding = json!({"source_param":"payload","encoding":"base64_json",
        "encoding_severity":"error","condition":{"kind":"value_in","param":"mode","values":["base64"]}});
    for bodies in [
        json!([warning, guarded, other_encoding]),
        json!([other_encoding, guarded, warning]),
    ] {
        let pack = pack(bodies, false);
        for (mode, count, expected) in [
            ("advisory", 1, Severity::Warning),
            ("strict", 1, Severity::Error),
            ("base64", 2, Severity::Error),
        ] {
            let report = pack.validate(&request(url("@@@", mode), false));
            assert_eq!(report.violations.len(), count);
            assert!(
                report
                    .violations
                    .iter()
                    .any(|finding| finding.severity == expected)
            );
            if count == 1 {
                assert_eq!(report.violations[0].severity, expected);
            }
        }
    }
    let guarded_only = pack(json!([guarded, other_encoding]), false);
    assert!(
        guarded_only
            .validate(&request(url("@@@", "inactive"), false))
            .violations
            .is_empty()
    );
}

#[test]
fn embedded_syntax_uses_selected_severity_and_preserves_json_string_span() {
    for encoding in ["json", "base64_json", "base64_latin1_json"] {
        let invalid = if encoding == "json" {
            "{\"n\":}"
        } else {
            "@@@"
        };
        let warning =
            json!({"source_field":"payload","encoding":encoding,"encoding_severity":"warning"});
        let error = json!({"source_field":"payload","encoding":encoding});
        for (body, expected) in [
            (json!([warning]), Severity::Warning),
            (json!([warning, error]), Severity::Error),
            (json!([error, warning]), Severity::Error),
        ] {
            let artifact = json!({"payload":invalid}).to_string();
            let report = pack(body, true).validate(&request(artifact.clone(), true));
            assert_eq!(report.violations.len(), 1);
            let finding = &report.violations[0];
            assert_eq!(finding.severity, expected);
            assert_eq!(finding.field.as_deref(), Some("body.payload"));
            let target = &finding.targets[0];
            assert_eq!(target.component, ViolationTargetComponent::BodyField);
            assert_eq!(target.name.as_deref(), Some("payload"));
            assert_eq!(
                serde_json::from_str::<String>(&artifact[target.start..target.end]).unwrap(),
                invalid
            );
        }
    }
}

#[test]
fn chained_decode_severity_keeps_outer_parameter_target_at_every_stage() {
    for encoding in ["json", "base64_json", "base64_latin1_json"] {
        let invalid = if encoding == "json" {
            "{\"n\":}"
        } else {
            "@@@"
        };
        let pack = pack(
            json!({"source_param":"payload","source_field":"inner",
                "field_encoding":encoding,"encoding_severity":"warning"}),
            false,
        );
        for payload in ["{\"n\":}".to_string(), json!({"inner":invalid}).to_string()] {
            let artifact = url(&payload, "advisory");
            let report = pack.validate(&request(artifact.clone(), false));
            assert_eq!(report.violations.len(), 1);
            assert_eq!(report.violations[0].severity, Severity::Warning);
            let target = &report.violations[0].targets[0];
            assert_eq!(target.component, ViolationTargetComponent::QueryParam);
            assert_eq!(target.name.as_deref(), Some("payload"));
            assert_eq!(target.value.as_deref(), Some(payload.as_str()));
            assert!(artifact[target.start..target.end].starts_with("payload="));
        }
    }
}

#[test]
fn nested_decoding_ignores_inactive_error_specs_and_combines_active_severities() {
    let warning = json!({"source_param":"payload","source_field":"inner",
        "decoded_source_field":"content","encoding_severity":"warning"});
    let error = json!({"source_param":"payload","source_field":"inner",
        "decoded_source_field":"content","encoding_severity":"error",
        "decoded_source_condition":{"path":"kind","pattern":"^strict$"}});
    for bodies in [json!([warning, error]), json!([error, warning])] {
        let pack = pack(bodies, false);
        for (kind, expected) in [("advisory", Severity::Warning), ("strict", Severity::Error)] {
            let inner = json!({"kind":kind,"content":"{\"n\":}"}).to_string();
            let payload = json!({"inner":inner}).to_string();
            let artifact = url(&payload, "test");
            let report = pack.validate(&request(artifact.clone(), false));
            assert_eq!(report.violations.len(), 1);
            assert_eq!(report.violations[0].severity, expected);
            let target = &report.violations[0].targets[0];
            assert_eq!(target.component, ViolationTargetComponent::QueryParam);
            assert_eq!(target.name.as_deref(), Some("payload"));
            assert_eq!(target.value.as_deref(), Some(payload.as_str()));
            assert!(artifact[target.start..target.end].starts_with("payload="));
        }
    }
}

#[test]
fn advisory_decoding_preserves_field_severities_and_macro_deferral() {
    let pack = pack(
        json!({"source_param":"payload","encoding_severity":"warning",
        "params":[{"name":"count","json_type":"number"}]}),
        false,
    );
    let report = pack.validate(&request(url(r#"{"count":"wrong"}"#, "test"), false));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].code,
        "custom.encoding.body.count.invalid"
    );
    assert_eq!(report.violations[0].severity, Severity::Error);
    assert!(
        pack.validate(&request(url("[CACHEBUSTER]", "test"), false))
            .violations
            .is_empty()
    );
}

#[test]
fn encoding_severity_rejects_unknown_values_and_specs_without_encoded_sources() {
    for (body, reason) in [
        (
            json!({"source_param":"payload","encoding_severity":"fatal"}),
            "fatal",
        ),
        (
            json!({"encoding_severity":"warning"}),
            "encoding_severity requires source_param or source_field",
        ),
    ] {
        let manifest = json!({"id":"custom/encoding","display_name":"Invalid severity", "description":"Invalid decoder declarations.",
            "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["payload"]},
            "params":[{"name":"payload"}],"body":body});
        let error = ManifestRulePack::from_json(&manifest.to_string())
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{error}");
    }
}
