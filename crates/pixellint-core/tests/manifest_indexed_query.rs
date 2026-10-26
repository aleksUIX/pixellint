use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, Severity, ValidationOptions,
    ValidationReport, ValidationRequest, ValidatorPlugin,
};
use serde_json::{Value, json};

fn manifest(prefix: &str) -> Value {
    json!({
        "id":"custom/indexed-query", "display_name":"Indexed query", "description":"Independent indexed query namespaces.", "source_level":"heuristic",
        "match":{"hosts":["unit.example.test"],"paths":["/collect"]},
        "query_scopes":[{
            "source":"indexed_query", "key_prefix":prefix,
            "params":[
                {"name":"scope_type", "allow_empty":true},
                {"name":"scope_id", "requirement":"recommended", "severity":"warning",
                 "condition":{"kind":"all_values_in","params":["scope_type"],"values":["org"]}}
            ]
        }]
    })
}

fn request(artifact: String, artifact_kind: ArtifactKind) -> ValidationRequest {
    ValidationRequest {
        artifact,
        artifact_kind,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

fn report(query: &str) -> ValidationReport {
    ManifestRulePack::from_json(&manifest("xid").to_string())
        .unwrap()
        .validate(&request(
            format!("https://unit.example.test/collect?{query}"),
            ArtifactKind::Url,
        ))
}

#[test]
fn conditions_are_local_to_each_lexical_index_and_never_borrow_global_owners() {
    for (query, expected) in [
        ("xid[0][scope_type]=org", 1),
        ("xid[0][scope_type]=org&xid[0][scope_id]=opaque", 0),
        ("xid[0][scope_type]=game", 0),
        ("xid[0][scope_type]=future", 0),
        ("xid[0][scope_type]=org&scope_id=global", 1),
        ("xid[0][scope_type]=org&xid[1][scope_id]=other-row", 1),
        ("xid[0][scope_type]=org&xid[1][scope_type]=org", 2),
        ("xid[0][scope_type]=org&xid[00][scope_id]=other-spelling", 1),
        (
            "xid[0][scope_type]=org&other[0][scope_id]=other-collection",
            1,
        ),
    ] {
        let result = report(query);
        assert_eq!(result.violations.len(), expected, "{query}: {result:?}");
        assert!(
            result
                .violations
                .iter()
                .all(|v| v.severity == Severity::Warning)
        );
    }
}

#[test]
fn zero_gaps_leading_zeros_and_unbounded_index_spellings_are_preserved() {
    for index in [
        "0",
        "00",
        "7",
        "1000000",
        "99999999999999999999999999999999999999999999999",
    ] {
        let result = report(&format!("xid[{index}][scope_type]=org"));
        assert_eq!(result.violations.len(), 1);
        let finding = &result.violations[0];
        assert_eq!(
            finding.field.as_deref(),
            Some(format!("indexed_query.xid[{index}][scope_id]").as_str())
        );
        assert_eq!(
            finding.targets[0].name.as_deref(),
            Some(format!("xid[{index}][scope_type]").as_str())
        );
    }
}

#[test]
fn every_discriminator_occurrence_is_retained_including_bare_blank_and_macros() {
    for (tail, expected) in [
        ("&xid[0][scope_type]=org", 1),
        ("&xid[0][scope_type]=game", 0),
        ("&xid[0][scope_type]", 0),
        ("&xid[0][scope_type]=", 0),
        ("&xid[0][scope_type]=[SCOPE]", 0),
        ("&xid[0][scope_type]=%24%7BSCOPE%7D", 0),
    ] {
        let result = report(&format!("xid[0][scope_type]=org{tail}"));
        assert_eq!(result.violations.len(), expected, "{tail}: {result:?}");
    }
    let result =
        report("xid[0][scope_type]=org&xid[0][scope_id]&xid[0][scope_id]=&xid[0][scope_id]=opaque");
    assert_eq!(result.violations.len(), 2);
    for finding in result.violations {
        assert_eq!(
            finding.code,
            "custom.indexed-query.indexed_query.scope_id.empty"
        );
        assert_eq!(finding.targets[0].name.as_deref(), Some("xid[0][scope_id]"));
    }
    assert!(
        report("xid[0][scope_type]=org&xid[0][scope_id]=[OWNER]")
            .violations
            .is_empty()
    );
}

#[test]
fn names_and_values_decode_once_without_reading_fragments_nested_text_or_other_shapes() {
    for (query, expected) in [
        ("%78id%5B%30%5D%5Bscope_type%5D=%6Frg", 1),
        ("%2578id[0][scope_type]=org", 0),
        ("xid[0][scope_type]=%256Frg", 0),
        ("next=xid%5B0%5D%5Bscope_type%5D%3Dorg", 0),
        ("other=1#xid[0][scope_type]=org", 0),
        ("xid[][scope_type]=org", 0),
        ("xid[-1][scope_type]=org", 0),
        ("xid[+1][scope_type]=org", 0),
        ("xid[INDEX][scope_type]=org", 0),
        ("xid[0][scope_type][nested]=org", 0),
        ("xid[0][scope_type]suffix=org", 0),
        ("Xid[0][scope_type]=org", 0),
        ("xid[0][Scope_type]=org", 0),
        ("xid[0][scope_type]=org&xid[0][scope_Id]=opaque", 1),
    ] {
        assert_eq!(report(query).violations.len(), expected, "{query}");
    }
}

#[test]
fn interleaved_missing_rows_anchor_only_their_first_actual_pair() {
    let url = "https://unit.example.test/collect?note=é&xid[0][id]=α&xid[1][scope_id]=other&xid[0][scope_type]=org";
    let pack = ManifestRulePack::from_json(&manifest("xid").to_string()).unwrap();
    let result = pack.validate(&request(url.into(), ArtifactKind::Url));
    let target = &result.violations[0].targets[0];
    assert_eq!(target.name.as_deref(), Some("xid[0][id]"));
    assert_eq!(&url[target.start..target.end], "xid[0][id]=α");
    let url =
        "https://unit.example.test/collect?xid[00][scope_type]=org&xid%5B00%5D%5Bscope_id%5D=";
    let result = pack.validate(&request(url.into(), ArtifactKind::Url));
    let target = &result.violations[0].targets[0];
    assert_eq!(target.name.as_deref(), Some("xid[00][scope_id]"));
    assert_eq!(&url[target.start..target.end], "xid%5B00%5D%5Bscope_id%5D=");
}

#[test]
fn complete_http_captures_read_only_url_query_pairs() {
    let mut engine = Engine::default();
    engine
        .register_manifest_json(&manifest("xid").to_string())
        .unwrap();
    for (query, body, expected) in [
        ("xid[0][scope_type]=org", "xid[0][scope_id]=body-only", 1),
        ("xid[0][scope_id]=query-only", "xid[0][scope_type]=org", 0),
        ("", "xid[0][scope_type]=org", 0),
        (
            "xid[0][scope_type]=org&xid[0][scope_id]=query",
            "xid[0][scope_id]=",
            0,
        ),
    ] {
        let capture = json!({"url":format!("https://unit.example.test/collect?{query}"),"method":"POST","headers":{"Content-Type":"application/x-www-form-urlencoded"},"body":body});
        let summary = engine
            .validate(
                &request(capture.to_string(), ArtifactKind::NetworkRequest),
                &ValidationOptions::default(),
            )
            .unwrap();
        let result = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "custom/indexed-query")
            .unwrap();
        assert_eq!(result.violations.len(), expected, "{capture}: {result:?}");
        if expected > 0 {
            assert_eq!(
                result.violations[0].field.as_deref(),
                Some("indexed_query.xid[0][scope_id]")
            );
            assert!(result.violations[0].targets.is_empty());
        }
    }
}

#[test]
fn key_prefix_is_a_literal_collection_name() {
    let pack = ManifestRulePack::from_json(&manifest("a.b").to_string()).unwrap();
    for (key, count) in [("a.b", 1), ("aXb", 0), ("xa.b", 0), ("xid", 0)] {
        let result = pack.validate(&request(
            format!("https://unit.example.test/collect?{key}[0][scope_type]=org"),
            ArtifactKind::Url,
        ));
        assert_eq!(result.violations.len(), count);
    }
}

#[test]
fn loader_rejects_missing_ambiguous_and_legacy_only_options() {
    let original = manifest("xid");
    assert!(ManifestRulePack::from_json(&original.to_string()).is_ok());
    for invalid in [json!(null), json!(""), json!("x[id"), json!("x]id")] {
        let mut m = original.clone();
        m["query_scopes"][0]["key_prefix"] = invalid;
        assert!(ManifestRulePack::from_json(&m.to_string()).is_err());
    }
    let mut missing = original.clone();
    missing["query_scopes"][0]
        .as_object_mut()
        .unwrap()
        .remove("key_prefix");
    assert!(ManifestRulePack::from_json(&missing.to_string()).is_err());
    for (key, value) in [
        ("first_index", json!(1)),
        ("last_index", json!(0)),
        ("global_params", json!(["scope_type"])),
    ] {
        let mut m = original.clone();
        m["query_scopes"][0][key] = value;
        assert!(
            ManifestRulePack::from_json(&m.to_string()).is_err(),
            "{key}"
        );
    }
    for style in ["query_semicolon", "matrix", "colon_path"] {
        let mut m = original.clone();
        m["param_style"] = json!(style);
        assert!(
            ManifestRulePack::from_json(&m.to_string()).is_err(),
            "{style}"
        );
    }
    for (source, style) in [
        ("query_semicolon", "query_semicolon"),
        ("path_items", "colon_path"),
    ] {
        let mut m = original.clone();
        m["query_scopes"][0]["source"] = json!(source);
        m["param_style"] = json!(style);
        assert!(
            ManifestRulePack::from_json(&m.to_string()).is_err(),
            "{source}"
        );
    }
}
