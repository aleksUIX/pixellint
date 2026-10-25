use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, ValidationOptions, ValidationRequest,
    ValidatorPlugin,
};
use serde_json::{Value, json};

fn manifest(markers: Value) -> Value {
    json!({
        "id": "custom/marker-profile",
        "display_name": "Marker profile",
        "description": "Select a profile by exact query marker values.",
        "source_level": "heuristic",
        "match": {"hosts": ["pixel.example.test"], "paths": ["/dispatch"], "query_params_values": markers},
        "params": [{"name": "version", "requirement": "required"}]
    })
}

fn request(url: &str, kind: ArtifactKind) -> ValidationRequest {
    ValidationRequest {
        artifact: url.into(),
        artifact_kind: kind,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

fn marker_pack() -> ManifestRulePack {
    ManifestRulePack::from_json(
        &manifest(json!([
            {"name":"a.A", "values":["im", "view"]},
            {"name":"a.hr", "values":["js"]}
        ]))
        .to_string(),
    )
    .unwrap()
}

#[test]
fn all_markers_are_required_and_every_duplicate_must_be_allowed() {
    let pack = marker_pack();
    for query in [
        "a.A=im&a.hr=js",
        "a.hr=js&a.A=view",
        "a.A=im&a.A=im&a.hr=js",
        "a.A=im&a.A=view&a.hr=js&a.hr=js",
        "%61.A=%69m&a.%68r=j%73",
    ] {
        let url = format!("https://pixel.example.test/dispatch?{query}");
        assert!(pack.supports(&request(&url, ArtifactKind::Url)), "{url}");
    }
    for query in [
        "a.A=im",
        "a.hr=js",
        "a.A=&a.hr=js",
        "a.A=im&a.A=cl&a.hr=js",
        "a.A=im&a.hr=js&a.hr=html",
        "A.A=im&a.hr=js",
        "a.A=IM&a.hr=js",
        "a.A=im&a.hr=JS",
        "a.A=im&a.A=&a.hr=js",
        "a.A=im&a.hr=js&a.hr",
    ] {
        let url = format!("https://pixel.example.test/dispatch?{query}");
        assert!(!pack.supports(&request(&url, ArtifactKind::Url)), "{url}");
    }
}

#[test]
fn markers_do_not_read_fragments_lookalikes_nested_text_or_second_decoding() {
    let pack = marker_pack();
    for url in [
        "https://pixel.example.test/dispatch#a.A=im&a.hr=js",
        "https://pixel.example.test/dispatch?a.A=im#a.hr=js",
        "https://pixel.example.test/dispatch?next=a.A%3Dim%26a.hr%3Djs",
        "https://pixel.example.test/dispatch?%2561.A=im&a.hr=js",
        "https://pixel.example.test/dispatch?a.A=%2569m&a.hr=js",
        "https://pixel.example.test/dispatch?a.A=im&a.hr=js%2520",
        "https://pixel.example.test.attacker.test/dispatch?a.A=im&a.hr=js",
        "https://other.example.test/dispatch?a.A=im&a.hr=js",
        "https://pixel.example.test/other?a.A=im&a.hr=js",
    ] {
        assert!(!pack.supports(&request(url, ArtifactKind::Url)), "{url}");
    }
}

#[test]
fn unresolved_macro_occurrences_never_select_a_literal_profile() {
    let pack = marker_pack();
    for query in [
        "a.A=[ACTION]&a.hr=js",
        "a.A=im&a.hr={{MODE}}",
        "a.A=im&a.A=[ACTION]&a.hr=js",
        "a.A=im&a.hr=js&a.hr=%24%7BMODE%7D",
    ] {
        let url = format!("https://pixel.example.test/dispatch?{query}");
        assert!(!pack.supports(&request(&url, ArtifactKind::Url)), "{url}");
    }
}

#[test]
fn allowed_empty_values_are_distinct_from_absence_and_use_form_decoding() {
    let pack = ManifestRulePack::from_json(&manifest(json!([
        {"name":"empty", "values":[""]}, {"name":"label", "values":["two words", "literal+plus"]}
    ])).to_string()).unwrap();
    for query in [
        "empty=&label=two+words",
        "empty&label=two%20words",
        "empty=&label=literal%2Bplus",
    ] {
        assert!(pack.supports(&request(
            &format!("https://pixel.example.test/dispatch?{query}"),
            ArtifactKind::Url
        )));
    }
    for query in [
        "label=two+words",
        "empty=x&label=two+words",
        "empty=&label=literal+plus",
    ] {
        assert!(!pack.supports(&request(
            &format!("https://pixel.example.test/dispatch?{query}"),
            ArtifactKind::Url
        )));
    }
}

#[test]
fn load_rejects_ambiguous_or_impossible_marker_declarations() {
    for markers in [
        json!([{"name":"", "values":["x"]}]),
        json!([{"name":"a", "values":[]}]),
        json!([{"name":"a", "values":["x", "x"]}]),
        json!([{"name":"a", "values":["", ""]}]),
        json!([{"name":"a", "values":["x"]}, {"name":"a", "values":["y"]}]),
    ] {
        assert!(ManifestRulePack::from_json(&manifest(markers).to_string()).is_err());
    }
    let mut value = manifest(json!([{"name":"a", "values":["x"]}]));
    value["match"]["query_params_none"] = json!(["a"]);
    assert!(ManifestRulePack::from_json(&value.to_string()).is_err());
    value["match"]["query_params_none"] = json!(["A"]);
    assert!(ManifestRulePack::from_json(&value.to_string()).is_ok());
}

#[test]
fn query_value_and_query_key_conditions_are_independent() {
    let mut value = manifest(json!([{"name":"a.A", "values":["im"]}]));
    value["match"]["query_params_any"] = json!(["enabled"]);
    value["match"]["query_params_none"] = json!(["disabled"]);
    let pack = ManifestRulePack::from_json(&value.to_string()).unwrap();
    for (query, expected) in [
        ("a.A=im&enabled=", true),
        ("a.A=im&enabled", true),
        ("a.A=im&en%61bled", true),
        ("a.A=im", false),
        ("a.A=im&enabled=&disabled=", false),
        ("a.A=im&enabled=&disabled", false),
        ("a.A=im&enabled&dis%61bled", false),
        ("a.A=im&enabled=&disabled=&disabled", false),
        ("a.A=cl&enabled=", false),
    ] {
        assert_eq!(
            pack.supports(&request(
                &format!("https://pixel.example.test/dispatch?{query}"),
                ArtifactKind::Url
            )),
            expected
        );
    }
}

#[test]
fn complete_captures_use_url_markers_not_body_keys_or_headers() {
    let mut engine = Engine::new();
    engine.register(marker_pack());
    let yes = json!({"url":"https://pixel.example.test/dispatch?a.A=im&a.hr=js", "method":"GET", "headers":{}});
    let result = engine
        .validate(
            &request(&yes.to_string(), ArtifactKind::NetworkRequest),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        result
            .reports
            .iter()
            .any(|report| report.plugin_id == "custom/marker-profile")
    );
    let no = json!({"url":"https://pixel.example.test/dispatch", "method":"POST", "headers":{"a.A":"im"}, "body":"{\"a.A\":\"im\",\"a.hr\":\"js\"}"});
    assert!(
        engine
            .validate(
                &request(&no.to_string(), ArtifactKind::NetworkRequest),
                &ValidationOptions::default()
            )
            .is_err()
    );
}

#[test]
fn explicit_rulepack_validation_retains_endpoint_mismatch_and_field_checks() {
    let mut engine = Engine::new();
    engine.register(marker_pack());
    let input = request(
        "https://pixel.example.test/dispatch?a.A=cl&a.hr=js",
        ArtifactKind::Url,
    );
    assert!(
        engine
            .validate(&input, &ValidationOptions::default())
            .is_err()
    );
    let options = ValidationOptions {
        only_rulepacks: vec!["custom/marker-profile".into()],
        ..ValidationOptions::default()
    };
    let report = engine.validate(&input, &options).unwrap();
    let codes: Vec<_> = report
        .reports
        .iter()
        .flat_map(|r| r.violations.iter().map(|v| v.code.as_str()))
        .collect();
    assert!(codes.contains(&"custom.marker-profile.endpoint_mismatch"));
    assert!(codes.contains(&"custom.marker-profile.param.version.missing"));
}
