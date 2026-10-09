//! Wire codecs and array relationships used by the new destination contracts.

use pixellint_core::{
    ArtifactKind, ExpansionState, ManifestRulePack, ValidationRequest, ValidatorPlugin,
};
use serde_json::{Value, json};

fn request(artifact: &str, kind: ArtifactKind) -> ValidationRequest {
    ValidationRequest {
        artifact: artifact.into(),
        artifact_kind: kind,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn browser_string_limits_count_utf16_units_after_json_decoding() {
    let pack=ManifestRulePack::from_json(&json!({
        "id":"custom/utf16","display_name":"Browser strings","description":"JavaScript string boundary.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"params":[{"name":"data","json_type":"string","max_utf16_length":4}]}
    }).to_string()).unwrap();
    for text in ["abcd", "😀😀", "éééé", "[LONG_TEMPLATE]"] {
        assert!(
            pack.validate(&request(
                &json!({"data":text}).to_string(),
                ArtifactKind::JsonPayload
            ))
            .violations
            .is_empty()
        );
    }
    assert!(
        pack.validate(&request(
            r#"{"data":"\ud83d\ude00\ud83d\ude00"}"#,
            ArtifactKind::JsonPayload
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&request(
        &json!({"data":"😀😀a"}).to_string(),
        ArtifactKind::JsonPayload,
    ));
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].message.contains("contains 5"));
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data")
    );
}

fn tuple_pack(max_bytes: usize) -> ManifestRulePack {
    ManifestRulePack::from_json(&json!({
        "id":"custom/tuple", "display_name":"Tuple codec", "description":"Independent tuple records.",
        "source_level":"heuristic", "match":{"hosts":["example.test"]},
        "params":[{"name":"row"}],
        "body":{"source_param":"row","encoding":"pipe_delimited_json",
            "params":[{"name":"","json_type":"array","min_items":3,"max_items":3},
                      {"name":"[1]","allow_empty":true,"format":{"kind":"integer"}}],
            "rules":[{"code":"custom.tuple.size","kind":"max_body_bytes","max_bytes":max_bytes,"severity":"error","message":"Tuple byte budget."}]}
    }).to_string()).unwrap()
}

#[test]
fn pipe_tuples_preserve_empty_positions_and_evaluate_repeated_parameters_independently() {
    let pack = tuple_pack(100);
    for text in [
        "event%7C1%7C",
        "event%7C%7C",
        "%7C1%7C",
        "%22quoted%22%7C2%7Ccaf%C3%A9",
        "event%7C%5BTIME%5D%7C",
    ] {
        let artifact = format!("https://example.test/event?row={text}");
        assert!(
            pack.validate(&request(&artifact, ArtifactKind::Url))
                .violations
                .is_empty(),
            "{text}"
        );
    }
    let artifact = "https://example.test/event?row=ok%7C1%7C&row=bad%7Coops%7C";
    let report = pack.validate(&request(artifact, ArtifactKind::Url));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].field.as_deref(), Some("param.row.[1]"));
    assert!(
        report.violations[0].targets[0]
            .value
            .as_deref()
            .unwrap()
            .starts_with("bad|")
    );
    let target = &report.violations[0].targets[0];
    assert!(artifact[target.start..target.end].contains("bad%7C"));
    assert_eq!(
        pack.validate(&request(
            "https://example.test/event?row=event%7C1",
            ArtifactKind::Url
        ))
        .violations
        .len(),
        1
    );
}

#[test]
fn tuple_byte_limits_measure_original_utf8_and_work_inside_json_string_fields() {
    let pack = tuple_pack(7);
    assert!(
        pack.validate(&request(
            "https://example.test/event?row=%C3%A9%7C1%7C%22",
            ArtifactKind::Url
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&request(
        "https://example.test/event?row=%C3%A9%C3%A9%7C1%7C%22%22",
        ArtifactKind::Url,
    ));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].code, "custom.tuple.size");
    let embedded = ManifestRulePack::from_json(&json!({
        "id":"custom/embedded-tuple","display_name":"Embedded tuple","description":"Tuple string source.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["rows"]},
        "body":[{"params":[{"name":"rows","json_type":"array"},{"name":"rows[]","json_type":"string"}]},
            {"source_field":"rows[]","encoding":"pipe_delimited_json","params":[{"name":"[1]","format":{"kind":"integer"}}]}]
    }).to_string()).unwrap();
    assert!(
        embedded
            .validate(&request(
                r#"{"rows":["é|1|","quote\"|2|"]}"#,
                ArtifactKind::JsonPayload
            ))
            .violations
            .is_empty()
    );
    let report = embedded.validate(&request(
        r#"{"rows":["é|1|","event|wrong|"]}"#,
        ArtifactKind::JsonPayload,
    ));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("rows[1]")
    );
}

fn uniqueness_pack(field: &str) -> ManifestRulePack {
    ManifestRulePack::from_json(&json!({
        "id":"custom/unique","display_name":"Unique members","description":"Each event owns its own privacy entries.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["events"]},
        "body":{"scope":"events[]","params":[{"name":"settings","json_type":"array"}],
            "rules":[{"code":"custom.unique.member","kind":"unique_array_by","param":"settings","field":field,"severity":"error","message":"Repeated member value."}]}
    }).to_string()).unwrap()
}

#[test]
fn array_member_uniqueness_is_scoped_to_each_event_and_targets_duplicate_members() {
    let pack = uniqueness_pack("type");
    for settings in [
        json!([{"type":"Gdpr"},{"type":"Dpo"}]),
        json!([{}, {"type":"[TYPE]"}, {"type":"[TYPE]"}]),
        json!([{"type":null},{"type":null}]),
    ] {
        let artifact =
            json!({"events":[{"settings":settings.clone()},{"settings":settings}]}).to_string();
        assert!(
            pack.validate(&request(&artifact, ArtifactKind::JsonPayload))
                .violations
                .is_empty()
        );
    }
    let artifact = r#"{"events":[{"settings":[{"type":"Gdpr"}]},{"settings":[{"type":"Gdpr"},{"type":"Gdpr"}]}]}"#;
    let report = pack.validate(&request(artifact, ArtifactKind::JsonPayload));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].targets.len(), 2);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("events[1].settings[0].type")
    );
    assert_eq!(
        report.violations[0].targets[1].name.as_deref(),
        Some("events[1].settings[1].type")
    );
}

#[test]
fn unique_array_members_compare_exact_native_values_and_literal_member_names() {
    let pack = uniqueness_pack("type.code");
    let artifact = r#"{"events":[{"settings":[{"type.code":1},{"type.code":1e0},{"type.code":"1"},{"type.code":true}]}]}"#;
    let report = pack.validate(&request(artifact, ArtifactKind::JsonPayload));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].targets.len(), 2);
    assert!(
        report.violations[0].targets[0]
            .name
            .as_deref()
            .unwrap()
            .ends_with("[\"type.code\"]")
    );
    let artifact=json!({"events":[{"settings":[{"type.code": "Gdpr"},{"type.code":"gdpr"},{"type":{"code":"Gdpr"}}]}]}).to_string();
    assert!(
        pack.validate(&request(&artifact, ArtifactKind::JsonPayload))
            .violations
            .is_empty()
    );
    let artifact = r#"{"events":[{"settings":[{"type.code":9007199254740992},{"type.code":9007199254740993}]}]}"#;
    assert!(
        pack.validate(&request(artifact, ArtifactKind::JsonPayload))
            .violations
            .is_empty()
    );
}

#[test]
fn unique_array_by_requires_a_declared_array_and_nonempty_member_name() {
    for (param, field) in [("unknown", "type"), ("settings", "")] {
        let manifest: Value = json!({"id":"custom/unique","display_name":"Invalid","description":"Invalid relationship.",
            "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["events"]},
            "body":{"params":[{"name":"settings","json_type":"array"}],
                "rules":[{"code":"custom.unique.member","kind":"unique_array_by","param":param,"field":field,"severity":"error","message":"Invalid."}]}});
        assert!(ManifestRulePack::from_json(&manifest.to_string()).is_err());
    }
}
