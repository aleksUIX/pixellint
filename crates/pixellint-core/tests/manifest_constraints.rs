//! Structural and boundary checks through the public manifest validator.

use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, ManifestRulePack, ValidationOptions,
    ValidationRequest, ValidatorPlugin,
};
use serde_json::{Value, json};

fn pack(params: Value, scope: Option<&str>) -> ManifestRulePack {
    let mut body = json!({"params": params});
    if let Some(scope) = scope {
        body["scope"] = json!(scope);
    }
    ManifestRulePack::from_json(
        &json!({
            "id": "custom/constraints", "display_name": "Contract checks",
            "description": "Checks JSON structures and scalar bounds.",
            "source_level": "heuristic",
            "match": {"hosts": ["example.test"], "json_paths": ["data"]},
            "body": body,
        })
        .to_string(),
    )
    .unwrap()
}

fn request(artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::JsonPayload,
        artifact: artifact.to_string(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn gpp_applicable_ids_check_native_integer_notations_and_csv_subsets() {
    let pack = assertion_pack(
        json!([{"name":"consent"},{"name":"sections","json_type":"array"},{"name":"sections[]","json_type":"integer"}]),
        json!([{"code":"custom.constraints.sections","kind":"gpp_sections","param":"consent","sections":"sections","severity":"error","message":"Applicable IDs must occur in the header."}]),
        None,
    );
    // Official iabgpp-es GppModel test vector for section 7.
    for ids in ["[7]", "[7e0]", "[70e-1]", "[-1]", "[]"] {
        let artifact =
            format!(r#"{{"data":1,"consent":"DBABLA~CAAAVVVVVVRA.QA","sections":{ids}}}"#);
        assert!(
            pack.validate(&request(&artifact)).violations.is_empty(),
            "{ids}"
        );
    }
    for ids in ["[1e0]", "[4294967296]", "[1e1000000]", "[-2]", "[-1,7]"] {
        let artifact =
            format!(r#"{{"data":1,"consent":"DBABLA~CAAAVVVVVVRA.QA","sections":{ids}}}"#);
        let report = pack.validate(&request(&artifact));
        assert_eq!(report.violations.len(), 1, "{ids}: {:?}", report.violations);
        assert_eq!(report.violations[0].code, "custom.constraints.sections");
        assert_eq!(
            report.violations[0].targets[1].name.as_deref(),
            Some("sections")
        );
    }
    let csv_pack = assertion_pack(
        json!([{"name":"consent"},{"name":"sections"}]),
        json!([{"code":"custom.constraints.sections","kind":"gpp_sections","param":"consent","sections":"sections","severity":"error","message":"Header membership."}]),
        None,
    );
    let consent = "DBACNYA~CPSG_8APSG_8ANwAAAENAwCgAAAAAAAAAAAAAAAAAAAA.IAAA.YAAAAAAAAAAA~1YNN";
    for ids in ["2", "6", "2,6", "-1"] {
        assert!(
            csv_pack
                .validate(&request(
                    &json!({"data":1,"consent":consent,"sections":ids}).to_string()
                ))
                .violations
                .is_empty()
        );
    }
    assert_eq!(
        csv_pack
            .validate(&request(
                &json!({"data":1,"consent":consent,"sections":"7"}).to_string()
            ))
            .violations
            .len(),
        1
    );
}

#[test]
fn nested_body_json_representations_keep_original_spans_and_source_limits() {
    let manifest = json!({"id":"custom/embedded","display_name":"Nested strings","description":"Nested event JSON strings.","source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},"body":[
        {"source_field":"data[].value","source_max_length":80,"source_max_length_when":"orders","params":[{"name":"","json_type":"object"}]},
        {"source_field":"data[].value","decoded_source_field":"content","decoded_source_condition":{"path":"content","pattern":"^\\s*\\["},"scope":"[]","params":[{"name":"quantity","json_type":"number","minimum":1}]}
    ]});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let valid = json!({"content":"Title"}).to_string();
    let invalid = json!({"content":"[{\"quantity\":0}]"}).to_string();
    let artifact = json!({"data":[{"value":valid},{"value":invalid}]}).to_string();
    let report = pack.validate(&request(&artifact));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].field.as_deref(),
        Some("body.data[1].value.content[0].quantity")
    );
    let target = &report.violations[0].targets[0];
    assert_eq!(target.name.as_deref(), Some("data[1].value"));
    assert_eq!(
        serde_json::from_str::<String>(&artifact[target.start..target.end]).unwrap(),
        invalid
    );
    for length in [80, 81] {
        let overhead = json!({"orders":[],"note":""}).to_string().chars().count();
        let value = json!({"orders":[],"note":"😀".repeat(length-overhead)}).to_string();
        assert_eq!(value.chars().count(), length);
        let artifact = json!({"data":[{"value":value}]}).to_string();
        let findings = pack.validate(&request(&artifact)).violations;
        assert_eq!(findings.len(), usize::from(length > 80));
        if let Some(finding) = findings.first() {
            assert_eq!(
                finding.code,
                "custom.embedded.body.data[].value.source_length"
            );
            assert_eq!(finding.field.as_deref(), Some("body.data[0].value"));
        }
    }
    assert!(
        pack.validate(&request(
            &json!({"data":[{"value":json!({"note":"a".repeat(100)}).to_string()}]}).to_string()
        ))
        .violations
        .is_empty()
    );
    for body in [
        json!({"decoded_source_field":"content"}),
        json!({"source_field":"value","decoded_source_condition":"content"}),
        json!({"source_field":"value","source_max_length_when":"orders"}),
    ] {
        let mut invalid = manifest.clone();
        invalid["body"] = body;
        assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
    }
}

#[test]
fn unpadded_calendar_dates_are_an_explicit_vendor_option() {
    let pack = pack(
        json!([{"name":"data","format":{"kind":"datetime","allow_date_only":true,"allow_unpadded_date":true}}]),
        None,
    );
    for date in [
        "2026-8-6",
        "2024-2-29",
        "2026-08-06",
        "2026-08-06T12:00:00Z",
    ] {
        assert!(
            pack.validate(&request(&json!({"data":date}).to_string()))
                .violations
                .is_empty(),
            "{date}"
        );
    }
    for date in [
        "2026-2-29",
        "2026-0-6",
        "2026-8-0",
        "2026-888-6",
        "26-8-6",
        "2026-8-6T12:00:00Z",
        "2026-€-6",
    ] {
        assert_eq!(
            pack.validate(&request(&json!({"data":date}).to_string()))
                .violations
                .len(),
            1,
            "{date}"
        );
    }
}

#[test]
fn parallel_item_arrays_count_records_and_preserve_each_batch() {
    let pack = assertion_pack(
        json!([{"name":"ids","json_type":"array"},{"name":"prices","json_type":"array"}]),
        json!([{"code":"custom.constraints.items","kind":"equal_array_lengths","params":["ids","prices"],"severity":"error","message":"Arrays must describe the same items."}]),
        Some("data[]"),
    );
    for event in [
        json!({"ids":["a,b","c"],"prices":[1,2]}),
        json!({"ids":[],"prices":[]}),
        json!({"ids":["a,b"]}),
    ] {
        assert!(
            pack.validate(&request(&json!({"data":[event]}).to_string()))
                .violations
                .is_empty()
        );
    }
    let artifact = json!({"data":[{"ids":["a","b"],"prices":[1,2]},{"ids":["c","d"],"prices":[3]},{"ids":[],"prices":[4]}]}).to_string();
    let findings = pack.validate(&request(&artifact)).violations;
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].targets[0].name.as_deref(), Some("data[1].ids"));
    assert_eq!(
        findings[1].targets[1].name.as_deref(),
        Some("data[2].prices")
    );
    assert_eq!(
        pack.validate(&request(r#"{"data":[{"ids":["a"],"prices":"wrong"}]}"#))
            .violations
            .len(),
        1
    );
}

#[test]
fn destination_tcf_permissions_and_iab_policy_have_independent_findings() {
    let manifest = json!({"id":"custom/permissions","display_name":"Consent requirements","description":"Destination consent flags.","source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},"body":{"params":[{"name":"cv","format":{"kind":"tcf"}},{"name":"ct"}],"rules":[{"code":"custom.permissions.consent","kind":"tcf_consent","param":"cv","vendor_id":97,"purpose_ids":[1],"condition":{"kind":"value_in","param":"ct","values":["4"]},"severity":"error","message":"Destination needs vendor and purpose consent."}]}});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let generic = "CQSbk4AQSbk4ANwAAAENAwCgAAAAAAAAAAYgACPAAAAA.IDKQA4AAgAKAGQAygAAA.YAAAAAAAAAAA";
    let approved =
        "CPSzMzJPTeGtxADABCENB_CoAP_AAEJAAAAADGwBAAGABPADCAY0BjYAgADAAngBhAMaAAA.YAAAAAAAA4AA";
    assert!(
        pack.validate(&request(
            &json!({"data":1,"ct":"4","cv":approved}).to_string()
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&request(
        &json!({"data":1,"ct":"4","cv":generic}).to_string(),
    ));
    assert_eq!(report.violations.len(), 2);
    assert_eq!(
        report.violations[0].code,
        "custom.permissions.body.cv.policy_warning"
    );
    assert_eq!(
        report.violations[0].severity,
        pixellint_core::Severity::Warning
    );
    assert_eq!(report.violations[1].code, "custom.permissions.consent");
    assert_eq!(
        pack.validate(&request(
            &json!({"data":1,"ct":"3","cv":generic}).to_string()
        ))
        .violations
        .len(),
        1
    );
    assert_eq!(
        pack.validate(&request(r#"{"data":1,"ct":"4","cv":"CA"}"#))
            .violations
            .len(),
        1
    );
    assert!(
        pack.validate(&request(r#"{"data":1,"ct":"4","cv":"REDACTED"}"#))
            .violations
            .is_empty()
    );
    let mut invalid = manifest;
    invalid["body"]["rules"][0]["purpose_ids"] = json!([25]);
    assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
}

#[test]
fn destination_gpp_choices_follow_only_applicable_decoded_state_fields() {
    let pack = assertion_pack(
        json!([{"name":"consent"},{"name":"sid"}]),
        json!([{"code":"custom.constraints.choice","kind":"gpp_field_values","param":"consent","section_param":"sid","field":"SensitiveDataProcessing","values":[0],"severity":"error","message":"Sensitive data requires the published destination choice."}]),
        None,
    );
    let consent = "DBABLA~CAAAVVVVVVRA.QA";
    assert_eq!(
        pack.validate(&request(
            &json!({"data":1,"consent":consent,"sid":"7"}).to_string()
        ))
        .violations
        .len(),
        1
    );
    for (signal, sid) in [
        (consent, "8"),
        (consent, "-1"),
        (consent, "[GPP_SID]"),
        ("[GPP_STRING]", "7"),
        ("DBABLA~CVQqAAAAAAA", "7"),
    ] {
        assert!(
            pack.validate(&request(
                &json!({"data":1,"consent":signal,"sid":sid}).to_string()
            ))
            .violations
            .is_empty()
        );
    }
}

#[test]
fn query_key_selection_preserves_empty_markers_and_percent_encoded_names() {
    let manifest = json!({"id":"custom/identity","display_name":"Identity branch","description":"Query keys select the documented identity branch.","source_level":"heuristic","match":{"hosts":["example.test"],"paths":["/id"],"query_params_any":["org","mid"]},"params":[{"name":"org"},{"name":"mid"},{"name":"version","requirement":"required"}]});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    for url in [
        "https://example.test/id?org=",
        "https://example.test/id?%6frg=123",
        "https://example.test/id?mid=[MID]",
    ] {
        assert!(pack.supports(&url_request(url)), "{url}");
        assert!(
            pack.validate(&url_request(url))
                .violations
                .iter()
                .any(|finding| finding.code == "custom.identity.param.version.missing")
        );
    }
    for url in [
        "https://example.test/id?cid=123",
        "https://example.test/id?cid=org",
        "https://example.test/id#org=123",
        "https://example.test/event?org=123",
    ] {
        assert!(!pack.supports(&url_request(url)), "{url}");
    }
    let mut generic = manifest.clone();
    generic["match"]["query_params_any"] = json!([]);
    generic["match"]["query_params_none"] = json!(["org", "mid"]);
    let generic = ManifestRulePack::from_json(&generic.to_string()).unwrap();
    assert!(generic.supports(&url_request("https://example.test/id?cid=123")));
    assert!(!generic.supports(&url_request("https://example.test/id?org=")));
    let mut engine = Engine::new();
    engine.register(pack);
    let options = ValidationOptions {
        only_rulepacks: vec!["custom/identity".into()],
        ..ValidationOptions::default()
    };
    assert!(
        engine
            .validate(&url_request("https://example.test/id?cid=123"), &options)
            .unwrap()
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .any(|finding| finding.code == "custom.identity.param.version.missing")
    );
    for keys in [json!([""]), json!(["org"])] {
        let mut invalid = manifest.clone();
        invalid["match"]["query_params_none"] = keys;
        assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
    }
}

#[test]
fn destination_https_and_total_url_limits_cover_path_query_and_unicode() {
    let manifest = json!({"id":"custom/transport","display_name":"URL transport","description":"Destination HTTPS and complete link limits.","source_level":"heuristic","match":{"hosts":["example.test"]},"rules":[
        {"code":"custom.transport.https","kind":"require_https","severity":"error","message":"HTTPS is required."},
        {"code":"custom.transport.length","kind":"max_url_length","max_length":64,"severity":"error","message":"The complete link is too long."}
    ]});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    for prefix in [
        "https://example.test/",
        "https://example.test/long/path?q=",
        "https://example.test/#",
    ] {
        let at = format!("{prefix}{}", "😀".repeat(64 - prefix.chars().count()));
        assert!(
            pack.validate(&url_request(&format!("  {at}\n")))
                .violations
                .is_empty()
        );
        let report = pack.validate(&url_request(&format!("{at}a")));
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].code, "custom.transport.length");
        assert_eq!(report.violations[0].targets[0].end, format!("{at}a").len());
    }
    assert!(
        pack.validate(&url_request("HTTPS://example.test/path"))
            .violations
            .is_empty()
    );
    let report = pack.validate(&url_request("http://example.test/path"));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].code, "custom.transport.https");
}

#[test]
fn destination_specific_privacy_values_are_bound_to_matching_selected_endpoints() {
    let manifest = json!({"id":"custom/privacy","display_name":"Vendor privacy","description":"Documented nonstandard applicability flag.","source_level":"heuristic", "gdpr_non_applicable_values":["2"], "match":{"hosts":["example.test"],"paths":["/event"]},"params":[{"name":"gdpr","format":{"kind":"enum","values":["1","2"]}}]});
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine.register(ManifestRulePack::from_json(&manifest.to_string()).unwrap());
    assert!(
        engine
            .validate(
                &url_request("https://example.test/event?gdpr=2"),
                &ValidationOptions::default()
            )
            .unwrap()
            .is_ok()
    );
    for url in [
        "https://other.test/event?gdpr=2",
        "https://example.test/other?gdpr=2",
    ] {
        let report = engine
            .validate(&url_request(url), &ValidationOptions::default())
            .unwrap();
        assert!(
            report
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .any(|finding| finding.code == "core.privacy.gdpr_invalid"),
            "{url}"
        );
    }
    let excluded = ValidationOptions {
        except_rulepacks: vec!["custom/privacy".to_string()],
        ..ValidationOptions::default()
    };
    assert!(
        !engine
            .validate(&url_request("https://example.test/event?gdpr=2"), &excluded)
            .unwrap()
            .is_ok()
    );
    for value in ["1", "3", ""] {
        let mut invalid = manifest.clone();
        invalid["gdpr_non_applicable_values"] = json!([value]);
        assert!(
            ManifestRulePack::from_json(&invalid.to_string()).is_err(),
            "{value}"
        );
    }
}

#[test]
fn typed_vendor_shapes_do_not_claim_a_different_native_event_envelope() {
    let manifest = json!({"id":"custom/shape","display_name":"Typed event","description":"Recognizable native event type.","source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":[{"path":"event","json_type":"string"}]},"body":{"params":[{"name":"event","json_type":"string","min_length":1},{"name":"id","requirement":"required"}]}});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    assert!(pack.supports(&request(r#"{"event":""}"#)));
    assert!(pack.supports(&request(r#"{"event":"capture"}"#)));
    assert!(!pack.supports(&request(r#"{"event":{"data":{"field":1}}}"#)));
    let mut engine = Engine::new();
    engine.register(pack);
    let options = ValidationOptions {
        only_rulepacks: vec!["custom/shape".to_string()],
        ..ValidationOptions::default()
    };
    let report = engine
        .validate(&request(r#"{"event":{"data":{"field":1}}}"#), &options)
        .unwrap();
    assert!(
        report
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .any(|finding| finding.code.ends_with("body.event.invalid"))
    );
    let mut invalid = manifest.clone();
    invalid["match"]["json_paths"][0]["json_type"] = json!([]);
    assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
    let mut invalid = manifest;
    invalid["match"]["json_paths"][0]["typo"] = json!(true);
    assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
}

#[test]
fn actual_json_types_are_checked_before_scalar_formats() {
    for (kind, valid, invalid) in [
        ("string", json!("ok"), json!(42)),
        ("number", json!(1.5), json!("1.5")),
        ("integer", json!(-2), json!(1.5)),
        ("boolean", json!(false), json!("false")),
        ("object", json!({}), json!([])),
        ("array", json!([]), json!({})),
        ("null", Value::Null, json!("null")),
    ] {
        let pack = pack(
            json!([{"name":"data", "requirement":"required", "json_type":kind}]),
            None,
        );
        assert!(
            pack.validate(&request(&json!({"data": valid}).to_string()))
                .violations
                .is_empty(),
            "valid {kind}"
        );
        let report = pack.validate(&request(&json!({"data": invalid}).to_string()));
        assert_eq!(report.violations.len(), 1, "invalid {kind}");
        assert_eq!(
            report.violations[0].code,
            "custom.constraints.body.data.invalid"
        );
    }
}

#[test]
fn integer_types_accept_integral_numeric_notation() {
    let pack = pack(json!([{"name":"data", "json_type":"integer"}]), None);
    for value in ["1.0", "1e3", "-1e2", "18446744073709551615"] {
        assert!(
            pack.validate(&request(&format!("{{\"data\":{value}}}")))
                .violations
                .is_empty(),
            "{value}"
        );
    }
    for value in [
        "1.1",
        "1e-3",
        "1e-999",
        "9007199254740993.1",
        "\"1\"",
        "true",
        "{}",
    ] {
        assert_eq!(
            pack.validate(&request(&format!("{{\"data\":{value}}}")))
                .violations
                .len(),
            1,
            "{value}"
        );
    }
}

#[test]
fn union_constraints_only_limit_the_applicable_type() {
    let pack = pack(
        json!([{"name":"data", "json_type":["string","integer","null"],
        "max_length":2, "minimum":0, "maximum":1}]),
        None,
    );
    for value in [json!("éé"), json!(0), json!(1), Value::Null] {
        assert!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .is_empty()
        );
    }
    for value in [json!("ééé"), json!(-1), json!(2), json!(true)] {
        assert_eq!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .len(),
            1
        );
    }
}

#[test]
fn numeric_strings_and_utf8_byte_limits_are_distinct_from_union_number_limits() {
    let pack = pack(
        json!([
            {"name":"data.id","json_type":"string","minimum":0,"maximum":9007199254740991_u64},
            {"name":"data.name","json_type":"string","max_length":2,"max_byte_length":4},
        ]),
        None,
    );
    for value in ["0", "9007199254740991"] {
        assert!(
            pack.validate(&request(
                &json!({"data":{"id":value,"name":"éé"}}).to_string()
            ))
            .violations
            .is_empty()
        );
    }
    for value in ["9007199254740992", "-1", "hello"] {
        assert_eq!(
            pack.validate(&request(&json!({"data":{"id":value}}).to_string()))
                .violations
                .len(),
            1
        );
    }
    let report = pack.validate(&request(r#"{"data":{"name":"😀a"}}"#));
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].message.contains("UTF-8 bytes"));
}

#[test]
fn array_scope_alternatives_accept_scalar_hashes_and_empty_arrays() {
    // Use the real scalar/array alternative shape instead of two overlapping specs.
    let manifest = json!({
        "id":"custom/hash", "display_name":"Hash union", "description":"Scalar and array hashes.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"scope":["data[]","data"],"params":[{"name":"","json_type":"string","format":{"kind":"hex","length":4}}]},
    });
    let hashes = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    for artifact in [
        r#"{"data":"abcd"}"#,
        r#"{"data":[]}"#,
        r#"{"data":["abcd","0123"]}"#,
    ] {
        assert!(
            hashes.validate(&request(artifact)).violations.is_empty(),
            "{artifact}"
        );
    }
    for artifact in [
        r#"{"data":"bad"}"#,
        r#"{"data":["abcd","bad"]}"#,
        r#"{"data":42}"#,
    ] {
        assert_eq!(
            hashes.validate(&request(artifact)).violations.len(),
            1,
            "{artifact}"
        );
    }
}

#[test]
fn nested_json_string_decoding_can_anchor_inside_an_encoded_query_document() {
    let manifest = json!({
        "id":"custom/embedded","display_name":"Chained JSON","description":"Query JSON with embedded JSON strings.",
        "source_level":"heuristic","match":{"hosts":["example.test"]},
        "params":[{"name":"data"},{"name":"mode"}],
        "body":[
            {"source_param":"data","params":[{"name":"metadata","json_type":"object"},{"name":"metadata.pub_metadata","json_type":"string"}]},
            {"source_param":"data","source_field":"metadata.pub_metadata","condition":{"kind":"value_in","param":"mode","values":["strict"]},"params":[{"name":"","json_type":"object"},{"name":"amount","json_type":"number","minimum":0}]}
        ]
    });
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let outer = json!({"metadata":{"pub_metadata":"{\"amount\":-1}"}}).to_string();
    let url = format!(
        "https://example.test/?mode=strict&data={}",
        url::form_urlencoded::byte_serialize(outer.as_bytes()).collect::<String>()
    );
    let report = pack.validate(&url_request(&url));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].field.as_deref(),
        Some("param.data.metadata.pub_metadata.amount")
    );
    let target = &report.violations[0].targets[0];
    assert!(url[target.start..target.end].starts_with("data="));
    assert_eq!(target.name.as_deref(), Some("data"));
    assert!(
        pack.validate(&url_request(&url.replace("mode=strict", "mode=other")))
            .violations
            .is_empty()
    );
    for inner in ["broken", "[]", "null"] {
        let outer = json!({"metadata":{"pub_metadata":inner}}).to_string();
        let url = format!(
            "https://example.test/?mode=strict&data={}",
            url::form_urlencoded::byte_serialize(outer.as_bytes()).collect::<String>()
        );
        assert_eq!(
            pack.validate(&url_request(&url)).violations.len(),
            1,
            "{inner}"
        );
    }
}

#[test]
fn json_strings_are_decoded_per_batch_element_with_original_byte_targets() {
    let manifest = json!({
        "id":"custom/embedded", "display_name":"Embedded JSON", "description":"Nested JSON strings.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":[
            {"scope":"data[]","params":[{"name":"value","json_type":"string"}]},
            {"source_field":"data[].value","params":[{"name":"","json_type":"object"},{"name":"amount","json_type":"number","minimum":0}]},
        ],
    });
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let artifact =
        json!({"data":[{"value":"{\"amount\":1}"},{"value":"{\"amount\":-1}"}]}).to_string();
    let report = pack.validate(&request(&artifact));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].field.as_deref(),
        Some("body.data[1].value.amount")
    );
    let target = &report.violations[0].targets[0];
    assert_eq!(
        serde_json::from_str::<String>(&artifact[target.start..target.end]).unwrap(),
        "{\"amount\":-1}"
    );
    assert_eq!(target.name.as_deref(), Some("data[1].value"));
    for value in ["null", "[]", "broken"] {
        assert_eq!(
            pack.validate(&request(&json!({"data":[{"value":value}]}).to_string()))
                .violations
                .len(),
            1,
            "{value}"
        );
    }
}

#[test]
fn calendar_space_separator_is_opt_in() {
    let datetime_pack = pack(
        json!([{"name":"data","format":{"kind":"datetime","allow_space_separator":true}}]),
        None,
    );
    assert!(
        datetime_pack
            .validate(&request(r#"{"data":"2024-02-29 12:17:01.123"}"#))
            .violations
            .is_empty()
    );
    assert_eq!(
        datetime_pack
            .validate(&request(r#"{"data":"2023-02-29 12:17:01.123"}"#))
            .violations
            .len(),
        1
    );
    let strict = pack(json!([{"name":"data","format":{"kind":"datetime"}}]), None);
    assert_eq!(
        strict
            .validate(&request(r#"{"data":"2024-02-29 12:17:01"}"#))
            .violations
            .len(),
        1
    );
}

#[test]
fn reserved_properties_depth_and_group_totals_have_independent_limits() {
    let pack = pack(
        json!([
            {"name":"data.custom","json_type":"object","max_properties":2,"property_exclusions":["reserved"]},
            {"name":"data.nested","json_type":"object","max_depth":2},
            {"name":"data.groups","json_type":"object","max_properties":3,"max_member_values":3},
        ]),
        None,
    );
    assert!(pack.validate(&request(r#"{"data":{"custom":{"reserved":0,"a":1,"b":2},"nested":{"x":[]},"groups":{"a":"one","b":["two","three"]}}}"#)).violations.is_empty());
    let report = pack.validate(&request(r#"{"data":{"custom":{"reserved":0,"a":1,"b":2,"c":3},"nested":{"x":[{}]},"groups":{"a":"one","b":["two","three","four"]}}}"#));
    assert_eq!(report.violations.len(), 3);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.message.contains("nesting depth"))
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.message.contains("member values"))
    );
}

#[test]
fn request_byte_limits_include_whitespace_and_multibyte_characters() {
    let manifest = json!({
        "id":"custom/bytes", "display_name":"Request size", "description":"Request UTF-8 size.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"rules":[{"code":"custom.bytes.size","kind":"max_body_bytes","max_bytes":14,"severity":"error","message":"Body too large."}]},
    });
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    for (artifact, count) in [
        (r#"{"data":"é"}"#, 0),
        (" {\"data\":\"é\"}", 0),
        ("  {\"data\":\"é\"}", 1),
    ] {
        assert_eq!(
            pack.validate(&request(artifact)).violations.len(),
            count,
            "{artifact}"
        );
    }
}

#[test]
fn guarded_decoded_contracts_and_list_cardinality_follow_outer_url_values() {
    let manifest = json!({
        "id":"custom/tuples", "display_name":"Query records", "description":"Conditional decoded fields and list records.",
        "source_level":"heuristic", "match":{"hosts":["example.test"]},
        "params":[{"name":"event"},{"name":"data"},{"name":"sku"},{"name":"price"},{"name":"parent"}],
        "body":{"source_param":"data","condition":{"kind":"value_in","param":"event","values":["special"]},"params":[{"name":"id","requirement":"required"}]},
        "rules":[{"code":"custom.tuples.rows","kind":"equal_split_lengths","params":["sku","price","parent"],"separator":"|","severity":"error","message":"Lists must align."}],
    });
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    for artifact in [
        "https://example.test/?event=ordinary&data=broken&sku=a|b&price=1|2",
        "https://example.test/?event=[EVENT]&data=broken",
        "https://example.test/?event=special&data=%7B%22id%22%3A1%7D&sku=a|b&parent=c|d",
    ] {
        let mut req = request(artifact);
        req.artifact_kind = ArtifactKind::Url;
        assert!(pack.validate(&req).violations.is_empty(), "{artifact}");
    }
    for (artifact, code) in [
        (
            "https://example.test/?event=special&data=%7B%7D",
            "custom.tuples.body.id.missing",
        ),
        (
            "https://example.test/?sku=a|b&price=1",
            "custom.tuples.rows",
        ),
    ] {
        let mut req = request(artifact);
        req.artifact_kind = ArtifactKind::Url;
        assert_eq!(pack.validate(&req).violations[0].code, code);
    }
}

#[test]
fn path_capture_discriminators_cannot_be_overridden_by_query_keys() {
    let manifest = json!({
        "id":"custom/version", "display_name":"Versioned endpoint", "description":"Version-specific identifiers.",
        "source_level":"heuristic", "path_pattern":"^/(?P<version>v[12])/evt$", "match":{"hosts":["example.test"]},
        "params":[{"name":"version"},{"name":"id"}],
        "rules":[{"code":"custom.version.id","kind":"required_when_value","when":"version","equals":["v2"],"requires":["id"],"severity":"error","message":"V2 requires ID."}],
    });
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    for artifact in [
        "https://example.test/v2/evt",
        "https://example.test/v2/evt?version=v1",
    ] {
        let mut req = request(artifact);
        req.artifact_kind = ArtifactKind::Url;
        assert_eq!(pack.validate(&req).violations.len(), 1);
    }
    let mut req = request("https://example.test/v1/evt?version=v2");
    req.artifact_kind = ArtifactKind::Url;
    assert!(pack.validate(&req).violations.is_empty());
}

#[test]
fn dynamic_literal_keys_cannot_collide_with_nested_paths_or_hide_empty_names() {
    let pack = pack(
        json!([
            {"name":"family","name_pattern_parent":"data","name_pattern":".*","json_type":"string"},
        ]),
        None,
    );
    let artifact = r#"{"data":{"a.b":42,"a":{"b":"ok"},"":false,"[0]":null}}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 4);
    let names: Vec<_> = report
        .violations
        .iter()
        .flat_map(|v| &v.targets)
        .filter_map(|t| t.name.as_deref())
        .collect();
    for path in ["data[\"a.b\"]", "data.a", "data[\"\"]", "data[\"[0]\"]"] {
        assert!(names.contains(&path), "{path}: {names:?}");
    }
    let literal = report
        .violations
        .iter()
        .find(|v| v.field.as_deref() == Some("body.data[\"a.b\"]"))
        .unwrap();
    assert_eq!(
        &artifact[literal.targets[0].start..literal.targets[0].end],
        "42"
    );
}

#[test]
fn timestamp_windows_use_one_fixed_clock_and_keep_inclusive_boundaries() {
    for (unit, lower, older, upper, later) in [
        (
            "seconds",
            json!(90),
            json!(89.999),
            json!(102),
            json!(102.001),
        ),
        (
            "milliseconds",
            json!(90000),
            json!(89999),
            json!(102000),
            json!(102001),
        ),
        (
            "datetime",
            json!("1970-01-01T00:01:30Z"),
            json!("1970-01-01T00:01:29.999Z"),
            json!("1970-01-01T00:01:42Z"),
            json!("1970-01-01T00:01:42.001Z"),
        ),
    ] {
        let manifest = json!({
            "id":"custom/window", "display_name":"Timestamp window", "description":"Clock bounds.",
            "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
            "body":{"scope":"data[]","params":[{"name":"time"}],"rules":[{"code":"custom.window.age","kind":"time_window","param":"time","unit":unit,"max_age_seconds":10,"max_future_seconds":2,"severity":"error","message":"Timestamp outside accepted window."}]},
        });
        let mut engine = Engine::default();
        engine.register(ManifestRulePack::from_json(&manifest.to_string()).unwrap());
        let options = ValidationOptions {
            only_rulepacks: vec!["custom/window".to_string()],
            except_rulepacks: vec![],
        };
        let artifact = json!({"data":[{"time":lower},{"time":older},{"time":upper},{"time":later},{"time":"[TIME]"}]}).to_string();
        let report = engine
            .validate_at(&request(&artifact), &options, 100)
            .unwrap();
        let violations: Vec<_> = report.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(violations.len(), 2, "{unit}: {violations:?}");
        let targets: Vec<_> = violations
            .iter()
            .flat_map(|v| &v.targets)
            .filter_map(|t| t.name.as_deref())
            .collect();
        assert_eq!(targets, ["data[1].time", "data[3].time"]);
    }
}

#[test]
fn conditional_field_families_preserve_custom_namespaces() {
    let pack = pack(
        json!([
            {"name":"data.type","json_type":"string"},
            {"name":"unknown","name_pattern_parent":"data","name_pattern":".*","requirement":"forbidden",
             "condition":{"kind":"value_in","param":"data.type","values":["commerce"]}},
        ]),
        None,
    );
    for artifact in [
        r#"{"data":{"type":"custom","new_key":{"nested":true}}}"#,
        r#"{"data":{"type":"[TYPE]","new_key":1}}"#,
    ] {
        serde_json::from_str::<Value>(artifact).unwrap();
        assert!(
            pack.validate(&request(artifact)).violations.is_empty(),
            "{artifact}"
        );
    }
    let report = pack.validate(&request(r#"{"data":{"type":"commerce","new_key":1}}"#));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].code,
        "custom.constraints.body.data.new_key.forbidden"
    );
}

#[test]
fn documented_string_normalization_precedes_value_checks_and_preserves_evidence() {
    let pack = pack(
        json!([{"name":"data","json_type":"string","normalization":"trim_lowercase","max_length":3,
        "format":{"kind":"regex","pattern":"^[a-z]+$"}}]),
        None,
    );
    assert!(
        pack.validate(&request(r#"{"data":"  ABC  "}"#))
            .violations
            .is_empty()
    );
    let artifact = r#"{"data":"  ABCD  "}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 1);
    let target = &report.violations[0].targets[0];
    assert_eq!(&artifact[target.start..target.end], r#""  ABCD  ""#);
    assert_eq!(target.value.as_deref(), Some("  ABCD  "));
    assert_eq!(
        pack.validate(&request(r#"{"data":true}"#)).violations.len(),
        1
    );
    assert!(
        pack.validate(&request(r#"{"data":" [VALUE] "}"#))
            .violations
            .is_empty()
    );
}

#[test]
fn recursive_value_paths_check_arrays_objects_and_literal_keys() {
    let pack = pack(
        json!([{"name":"","json_type":["string","number","boolean","object","array","null"],"max_length":3}]),
        Some("data.**"),
    );
    let artifact =
        r#"{"data":{"x":["ok",{"a.b":"long"}],"":"four","nested":{"v":2}},"neighbor":"ignore me"}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 2);
    let targets: Vec<_> = report
        .violations
        .iter()
        .flat_map(|v| &v.targets)
        .filter_map(|t| t.name.as_deref())
        .collect();
    assert!(targets.contains(&"data.x[1][\"a.b\"]"));
    assert!(targets.contains(&"data[\"\"]"));
    assert!(
        pack.validate(&request(r#"{"data":{}}"#))
            .violations
            .is_empty()
    );
}

#[test]
fn large_moduli_do_not_overflow_and_unsupported_significands_fail_loading() {
    let maximum = u128::MAX.to_string();
    let manifest = format!(
        r#"{{"id":"custom/modulus","display_name":"Modulus","description":"Exact modulus.","source_level":"heuristic","match":{{"hosts":["example.test"],"json_paths":["data"]}},"body":{{"params":[{{"name":"data","json_type":"integer","multiple_of":{maximum}}}]}}}}"#
    );
    let pack = ManifestRulePack::from_json(&manifest).unwrap();
    for value in [maximum.clone(), format!("{maximum}0")] {
        assert!(
            pack.validate(&request(&format!("{{\"data\":{value}}}")))
                .violations
                .is_empty(),
            "{value}"
        );
    }
    assert_eq!(
        pack.validate(&request(
            "{\"data\":340282366920938463463374607431768211456}"
        ))
        .violations
        .len(),
        1
    );
    let unsupported = manifest.replace(&maximum, "1000000000000000000000000000000000000033");
    assert!(
        ManifestRulePack::from_json(&unsupported)
            .unwrap_err()
            .to_string()
            .contains("significand")
    );
}

#[test]
fn numeric_timestamp_conversion_limits_cannot_bypass_windows() {
    let manifest = json!({
        "id":"custom/window", "display_name":"Timestamp window", "description":"Clock bounds.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"params":[{"name":"data","json_type":"number"}],"rules":[{"code":"custom.window.time","kind":"time_window","param":"data","unit":"milliseconds","max_age_seconds":10,"max_future_seconds":0,"severity":"error","message":"Invalid window."}]},
    });
    let mut engine = Engine::new();
    engine.register(ManifestRulePack::from_json(&manifest.to_string()).unwrap());
    for artifact in ["{\"data\":9223372036854775808}", "{\"data\":1e-999}"] {
        let summary = engine
            .validate_at(&request(artifact), &ValidationOptions::default(), 100)
            .unwrap();
        assert!(!summary.is_ok(), "{artifact}");
    }
}

#[test]
fn batch_bounds_and_element_types_are_independent() {
    let pack = pack(
        json!([
            {"name":"data", "requirement":"required", "json_type":"array", "min_items":1, "max_items":2},
            {"name":"data[]", "json_type":"object"},
        ]),
        None,
    );
    for artifact in [
        r#"{"data":[]}"#,
        r#"{"data":{}}"#,
        r#"{"data":[{},{},{}]}"#,
        r#"{"data":[{},false]}"#,
    ] {
        assert_eq!(
            pack.validate(&request(artifact)).violations.len(),
            1,
            "{artifact}"
        );
    }
    assert!(
        pack.validate(&request(r#"{"data":[{},{}]}"#))
            .violations
            .is_empty()
    );
}

#[test]
fn root_and_scoped_object_limits_report_real_byte_spans() {
    let pack = pack(
        json!([{"name":"", "json_type":"object", "max_properties":1}]),
        Some("data[]"),
    );
    let artifact = r#"{"data":[{"x":1},{"x":1,"y":2},false]}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 2);
    for (violation, expected) in report.violations.iter().zip([r#"{"x":1,"y":2}"#, "false"]) {
        assert_eq!(violation.code, "custom.constraints.body.root.invalid");
        let target = &violation.targets[0];
        assert_eq!(&artifact[target.start..target.end], expected);
    }
    let root = pack_for_root_array();
    assert!(
        root.validate(&request("[{\"data\":1}]"))
            .violations
            .is_empty()
    );
    assert_eq!(
        root.validate(&request("[{\"data\":1},{\"data\":2}]"))
            .violations[0]
            .code,
        "custom.root.body.root.invalid"
    );
}

fn pack_for_root_array() -> ManifestRulePack {
    ManifestRulePack::from_json(&json!({
        "id":"custom/root", "display_name":"Root array", "description":"Array envelope contract.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["[].data"]},
        "body":{"params":[{"name":"","json_type":"array","max_items":1}]},
    }).to_string()).unwrap()
}

#[test]
fn numeric_bounds_are_inclusive_and_keep_large_integer_precision() {
    let pack = pack(
        json!([{"name":"data", "json_type":"integer", "minimum":9007199254740992_u64,
        "maximum":9007199254740993_u64}]),
        None,
    );
    for value in [9007199254740992_u64, 9007199254740993] {
        assert!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .is_empty()
        );
    }
    for value in [9007199254740991_u64, 9007199254740994] {
        assert_eq!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .len(),
            1
        );
    }
    for value in ["9007199254740991e0", "9007199254740994e0"] {
        assert_eq!(
            pack.validate(&request(&format!("{{\"data\":{value}}}")))
                .violations
                .len(),
            1
        );
    }
}

#[test]
fn manifest_decimal_bounds_preserve_their_original_precision() {
    for (bound, valid, invalid) in [
        (
            "\"maximum\":9007199254740993.0",
            "9007199254740993",
            "9007199254740994",
        ),
        ("\"minimum\":1e-999", "1e-999", "0"),
        (
            "\"minimum\":0.1000000000000000001",
            "0.1000000000000000001",
            "0.1",
        ),
    ] {
        let manifest = format!(
            r#"{{"id":"custom/decimal","display_name":"Exact decimal","description":"Exact bound.","source_level":"heuristic","match":{{"hosts":["example.test"],"json_paths":["data"]}},"body":{{"params":[{{"name":"data","json_type":"number",{bound}}}]}}}}"#
        );
        let pack = ManifestRulePack::from_json(&manifest).unwrap();
        assert!(
            pack.validate(&request(&format!("{{\"data\":{valid}}}")))
                .violations
                .is_empty(),
            "{bound}"
        );
        assert_eq!(
            pack.validate(&request(&format!("{{\"data\":{invalid}}}")))
                .violations
                .len(),
            1,
            "{bound}"
        );
    }
}

#[test]
fn decoded_url_lengths_and_numeric_bounds_preserve_template_behavior() {
    let pack = ManifestRulePack::from_json(&json!({
        "id":"custom/url", "display_name":"URL bounds", "description":"Decoded value contracts.",
        "source_level":"heuristic", "match":{"hosts":["example.test"]},
        "params":[{"name":"name","max_length":2},{"name":"value","minimum":-1,"maximum":1}],
    }).to_string()).unwrap();
    for artifact in [
        "https://example.test/?name=%C3%A9%C3%A9&value=-1",
        "https://example.test/?value=1e0",
        "https://example.test/?value=[VALUE]",
    ] {
        let mut req = request(artifact);
        req.artifact_kind = ArtifactKind::Url;
        assert!(pack.validate(&req).violations.is_empty(), "{artifact}");
    }
    for artifact in [
        "https://example.test/?name=%C3%A9%C3%A9%C3%A9",
        "https://example.test/?value=1.1",
        "https://example.test/?value=NaN",
    ] {
        let mut req = request(artifact);
        req.artifact_kind = ArtifactKind::Url;
        assert_eq!(pack.validate(&req).violations.len(), 1, "{artifact}");
    }
}

#[test]
fn invalid_constraints_are_rejected_at_load_time() {
    for contract in [
        json!({"name":"data", "json_type":[]}),
        json!({"name":"data", "json_type":"string", "max_items":2}),
        json!({"name":"data", "min_length":3, "max_length":2}),
        json!({"name":"data", "min_items":3, "max_items":2}),
        json!({"name":"data", "min_properties":3, "max_properties":2}),
        json!({"name":"data", "minimum":2, "maximum":1}),
    ] {
        let manifest = json!({"id":"custom/bad", "display_name":"Bad constraints", "description":"Reject invalid constraints.",
            "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
            "body":{"params":[contract]}});
        assert!(
            ManifestRulePack::from_json(&manifest.to_string()).is_err(),
            "{manifest}"
        );
    }
}

fn assertion_pack(params: Value, rules: Value, scope: Option<&str>) -> ManifestRulePack {
    let mut body = json!({"params":params,"rules":rules});
    if let Some(scope) = scope {
        body["scope"] = json!(scope);
    }
    ManifestRulePack::from_json(&json!({
        "id":"custom/constraints", "display_name":"Assertion checks", "description":"Scoped cross-field contracts.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":body,
    }).to_string()).unwrap()
}

#[test]
fn identifier_alternatives_accept_complete_groups_and_reject_empty_values() {
    let pack = assertion_pack(
        json!([{"name":"email"},{"name":"ip"},{"name":"ua"}]),
        json!([{"code":"custom.constraints.identifier","kind":"require_any_of",
            "groups":[["email"],["ip","ua"]],"severity":"error","message":"A match group is required."}]),
        None,
    );
    for artifact in [
        r#"{"data":1,"email":"hash"}"#,
        r#"{"data":1,"ip":"127.0.0.1","ua":"browser"}"#,
    ] {
        assert!(pack.validate(&request(artifact)).violations.is_empty());
    }
    for artifact in [
        r#"{"data":1,"email":""}"#,
        r#"{"data":1,"ip":"127.0.0.1"}"#,
        r#"{"data":1}"#,
    ] {
        assert!(
            pack.validate(&request(artifact))
                .violations
                .iter()
                .any(|violation| violation.code == "custom.constraints.identifier")
        );
    }
}

#[test]
fn discriminator_formats_run_per_batch_element() {
    let pack = assertion_pack(
        json!([{"name":"kind"},{"name":"value","json_type":"string"}]),
        json!([{"code":"custom.constraints.ip","kind":"format_when","when":"kind","equals":["ip"],
            "param":"value","format":{"kind":"ip"},"severity":"error","message":"Send a literal IP."}]),
        Some("data[]"),
    );
    let artifact = r#"{"data":[{"kind":"ip","value":"::1"},{"kind":"other","value":"text"},{"kind":"ip","value":"text"}]}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data[2].value")
    );
    let artifact = r#"{"data":[{"kind":"ip","value":"[IP_ADDRESS]"}]}"#;
    assert!(pack.validate(&request(artifact)).violations.is_empty());
}

#[test]
fn guarded_dependencies_preserve_unknown_macro_values() {
    let pack = assertion_pack(
        json!([{"name":"event"},{"name":"value"},{"name":"currency"}]),
        json!([{"code":"custom.constraints.currency","kind":"required_with","when":"value","requires":["currency"],
            "condition":{"kind":"not","condition":{"kind":"value_in","param":"event","values":["virtual"]}},
            "severity":"error","message":"Currency is required."}]),
        Some("data[]"),
    );
    for event in ["virtual", "[EVENT_NAME]"] {
        assert!(
            pack.validate(&request(
                &json!({"data":[{"event":event,"value":1}]}).to_string()
            ))
            .violations
            .is_empty()
        );
    }
    assert_eq!(
        pack.validate(&request(r#"{"data":[{"event":"purchase","value":1}]}"#))
            .violations
            .len(),
        1
    );
}

#[test]
fn dynamic_member_contracts_preserve_explicit_overrides() {
    let pack = pack(
        json!([
            {"name":"properties", "json_type":"object"},
            {"name":"properties.flag", "json_type":"boolean"},
            {"name":"property_values", "name_pattern":".*", "name_pattern_parent":"properties", "json_type":["string","number"],"max_length":3},
        ]),
        None,
    );
    let clean = r#"{"data":1,"properties":{"flag":false,"text":"yes","num":42}}"#;
    assert!(pack.validate(&request(clean)).violations.is_empty());
    let artifact = r#"{"data":1,"properties":{"flag":"false","nested":{},"text":"long"}}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 3);
    assert_eq!(
        report
            .violations
            .iter()
            .filter(|violation| violation.code == "custom.constraints.body.properties.flag.invalid")
            .count(),
        1
    );
    assert!(
        report
            .violations
            .iter()
            .any(|violation| violation.code == "custom.constraints.body.properties.nested.invalid")
    );
}

#[test]
fn wildcard_scopes_validate_every_dynamic_property_object() {
    let pack = pack(
        json!([{"name":"value", "json_type":"string", "max_length":2}]),
        Some("data.*"),
    );
    let artifact = r#"{"data":{"first":{"value":"ok"},"second":{"value":"long"}}}"#;
    let report = pack.validate(&request(artifact));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data.second.value")
    );
}

#[test]
fn explicit_selection_checks_unrecognized_envelopes_without_inferring_vendor() {
    let mut engine = Engine::new();
    engine.register(pack(
        json!([{"name":"data", "requirement":"required", "json_type":"array","min_items":1}]),
        None,
    ));
    let summary = engine
        .validate(
            &request(r#"{"data":[]}"#),
            &ValidationOptions {
                only_rulepacks: vec!["custom/constraints".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert!(!summary.is_ok());
    let summary = engine
        .validate(
            &request(r#"{}"#),
            &ValidationOptions {
                only_rulepacks: vec!["custom/constraints".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(summary.reports[0].detected_vendor, None);
    assert!(
        summary.reports[0]
            .violations
            .iter()
            .any(|violation| violation.code == "custom.constraints.body.data.missing")
    );
    assert!(
        summary.reports[0]
            .violations
            .iter()
            .any(|violation| violation.code == "custom.constraints.payload_mismatch")
    );
}

#[test]
fn ip_and_datetime_formats_check_address_family_and_calendar() {
    let pack = pack(
        json!([
            {"name":"data.ip", "format":{"kind":"ip","version":"v4"}},
            {"name":"data.ts", "format":{"kind":"datetime","require_timezone":true}},
        ]),
        None,
    );
    for (ip, ts) in [
        ("192.168.1.1", "2024-02-29T23:59:59Z"),
        ("127.0.0.1", "2026-10-07T00:00:00.123-07:00"),
    ] {
        assert!(
            pack.validate(&request(&json!({"data":{"ip":ip,"ts":ts}}).to_string()))
                .violations
                .is_empty()
        );
    }
    for (ip, ts) in [
        ("256.1.1.1", "2025-02-29T00:00:00Z"),
        ("::1", "2026-10-07T24:00:00Z"),
        ("host.test", "2026-10-07T00:00:00"),
    ] {
        assert_eq!(
            pack.validate(&request(&json!({"data":{"ip":ip,"ts":ts}}).to_string()))
                .violations
                .len(),
            2
        );
    }
}

#[test]
fn a_request_field_can_override_the_default_identifier_length() {
    let pack = assertion_pack(
        json!([{"name":"data[].id"},{"name":"options.min_id_length","json_type":"integer","minimum":1}]),
        json!([{"code":"custom.constraints.id_length","kind":"min_length_from","params":["data[].id"],
            "length_param":"options.min_id_length","default_length":5,"severity":"error","message":"IDs are too short."}]),
        None,
    );
    assert_eq!(
        pack.validate(&request(r#"{"data":[{"id":"abc"}]}"#))
            .violations
            .len(),
        1
    );
    assert!(
        pack.validate(&request(
            r#"{"data":[{"id":"abc"}],"options":{"min_id_length":3.0}}"#
        ))
        .violations
        .is_empty()
    );
    assert_eq!(
        pack.validate(&request(
            r#"{"data":[{"id":"abc"}],"options":{"min_id_length":4}}"#
        ))
        .violations
        .len(),
        1
    );
}

#[test]
fn query_order_and_total_length_are_checked_on_the_wire() {
    let pack = ManifestRulePack::from_json(&json!({
        "id":"custom/url", "display_name":"Wire contract", "description":"Raw query constraints.","source_level":"heuristic",
        "match":{"hosts":["example.test"]},"params":[{"name":"last"}],
        "rules":[
            {"code":"custom.url.order","kind":"last_param","param":"last","severity":"error","message":"Place last at the end."},
            {"code":"custom.url.length","kind":"max_query_length","max_length":10,"severity":"error","message":"Query is too long."},
        ],
    }).to_string()).unwrap();
    let mut req = request("https://example.test/?a=1&last=1");
    req.artifact_kind = ArtifactKind::Url;
    assert!(pack.validate(&req).violations.is_empty());
    req.artifact = "https://example.test/?last=1&a=1".to_string();
    assert_eq!(pack.validate(&req).violations[0].code, "custom.url.order");
    req.artifact = "https://example.test/?last=%C3%A9".to_string();
    assert_eq!(pack.validate(&req).violations[0].code, "custom.url.length");
}

fn encoded_pack(encoding: &str) -> ManifestRulePack {
    ManifestRulePack::from_json(&json!({
        "id":"custom/encoded", "display_name":"Encoded payload", "description":"JSON URL payload contracts.",
        "source_level":"heuristic", "match":{"hosts":["example.test"]},
        "params":[{"name":"payload","allow_empty":true}],
        "body":{"source_param":"payload","encoding":encoding,
            "params":[{"name":"data","requirement":"required","json_type":"string"}]},
    }).to_string()).unwrap()
}

fn encoded_request(payload: &str) -> ValidationRequest {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("payload", payload)
        .finish();
    let mut request = request(&format!("https://example.test/?{query}"));
    request.artifact_kind = ArtifactKind::Url;
    request
}

#[test]
fn encoded_json_checks_decoded_fields_and_points_to_original_url_bytes() {
    let pack = encoded_pack("json");
    assert!(
        pack.validate(&encoded_request(r#"{"data":"ok"}"#))
            .violations
            .is_empty()
    );
    let req = encoded_request(r#"{"data":42}"#);
    let report = pack.validate(&req);
    assert_eq!(report.violations.len(), 1);
    let violation = &report.violations[0];
    assert_eq!(violation.code, "custom.encoded.body.data.invalid");
    assert_eq!(violation.field.as_deref(), Some("param.payload.data"));
    let target = &violation.targets[0];
    assert!(req.artifact[target.start..target.end].starts_with("payload="));
    assert_eq!(target.name.as_deref(), Some("payload"));
    assert_eq!(
        pack.validate(&encoded_request("{broken")).violations[0].code,
        "custom.encoded.body.payload.invalid"
    );
    assert!(pack.validate(&encoded_request("")).violations.is_empty());
    assert!(
        pack.validate(&encoded_request("[PAYLOAD]"))
            .violations
            .is_empty()
    );
    assert!(!pack.supports(&request(r#"{"data":"ok"}"#)));
}

#[test]
fn base64_json_supports_both_alphabets_and_optional_padding() {
    let pack = encoded_pack("base64_json");
    for encoded in [
        "eyJkYXRhIjoib2sifQ==",
        "eyJkYXRhIjoib2sifQ",
        "eyJkYXRhIjoiw7/DvyJ9",
        "eyJkYXRhIjoiw7_DvyJ9",
    ] {
        assert!(
            pack.validate(&encoded_request(encoded))
                .violations
                .is_empty(),
            "{encoded}"
        );
    }
    for encoded in [
        "A",
        "***",
        "eyJkYXRhIjoib2sifQ=",
        "eyJkYXRhIjoib2sifR==",
        "////",
        "e2Jyb2tlbg==",
    ] {
        assert_eq!(
            pack.validate(&encoded_request(encoded)).violations.len(),
            1,
            "{encoded}"
        );
    }
    assert_eq!(
        pack.validate(&encoded_request("eyJkYXRhIjoxfQ=="))
            .violations[0]
            .code,
        "custom.encoded.body.data.invalid"
    );
}

#[test]
fn browser_btoa_json_preserves_latin1_text_without_changing_utf8_encoding() {
    let latin1 = encoded_pack("base64_latin1_json");
    let utf8 = encoded_pack("base64_json");
    let encoded = "eyJkYXRhIjoiY2Fm6SJ9";
    assert!(
        latin1
            .validate(&encoded_request(encoded))
            .violations
            .is_empty()
    );
    assert_eq!(utf8.validate(&encoded_request(encoded)).violations.len(), 1);

    let exact = ManifestRulePack::from_json(
        &json!({
        "id":"custom/latin1", "display_name":"Browser btoa payload",
        "source_level":"heuristic",
            "description":"Latin-1 JSON fields and original decoded wire bytes.",
            "match":{"hosts":["example.test"]}, "params":[{"name":"payload"}],
            "body":{"source_param":"payload","encoding":"base64_latin1_json",
                "params":[{"name":"data","format":{"kind":"regex","pattern":"^café$"}}],
                "rules":[{"code":"custom.latin1.bytes","kind":"max_body_bytes","max_bytes":15,
                    "severity":"error","message":"The decoded Latin-1 entity exceeds 15 bytes."}]}
        })
        .to_string(),
    )
    .unwrap();
    assert!(
        exact
            .validate(&encoded_request(encoded))
            .violations
            .is_empty()
    );
    // These are UTF-8 bytes. The Latin-1 codec must preserve both bytes separately.
    let report = exact.validate(&encoded_request("eyJkYXRhIjoiY2Fmw6kifQ=="));
    assert_eq!(report.violations.len(), 2);
    assert!(
        report
            .violations
            .iter()
            .any(|finding| finding.code == "custom.latin1.bytes")
    );
    assert!(
        report
            .violations
            .iter()
            .any(|finding| finding.code == "custom.latin1.body.data.invalid")
    );
    let report = exact.validate(&encoded_request("eyJkYXRhIjo0Mn0="));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("payload")
    );
}

#[test]
fn latin1_base64_embedded_fields_support_json_escapes_and_share_syntax_checks() {
    let pack = ManifestRulePack::from_json(
        &json!({
        "id":"custom/latin1", "display_name":"Browser btoa field",
        "source_level":"heuristic",
            "description":"An embedded Latin-1 JSON string.",
            "match":{"hosts":["example.test"],"json_paths":["payload"]},
            "body":{"source_field":"payload","encoding":"base64_latin1_json",
                "params":[{"name":"data","requirement":"required","json_type":"string"}]}
        })
        .to_string(),
    )
    .unwrap();
    for encoded in ["eyJkYXRhIjoiY2Fm6SJ9", "eyJkYXRhIjoiXHUwMTAwIn0="] {
        assert!(
            pack.validate(&request(&json!({"payload":encoded}).to_string()))
                .violations
                .is_empty()
        );
    }
    for encoded in [
        "A",
        "***",
        "eyJkYXRhIjoib2sifQ=",
        "eyJkYXRhIjoib2sifR==",
        "////",
        "e2Jyb2tlbg==",
    ] {
        assert_eq!(
            pack.validate(&request(&json!({"payload":encoded}).to_string()))
                .violations
                .len(),
            1,
            "{encoded}"
        );
    }
    assert!(
        pack.validate(&request(&json!({"payload":"[CUSTOM_DATA]"}).to_string()))
            .violations
            .is_empty()
    );
    let report = pack.validate(&request(&json!({"payload":"eyJkYXRhIjo0Mn0="}).to_string()));
    assert_eq!(
        report.violations[0].field.as_deref(),
        Some("body.payload.data")
    );
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("payload")
    );
}

#[test]
fn latin1_base64_can_be_nested_in_json_from_a_url_parameter() {
    let pack = ManifestRulePack::from_json(
        &json!({
        "id":"custom/latin1", "display_name":"Nested browser payload",
        "source_level":"heuristic",
            "description":"Outer JSON and inner browser btoa data use separate codecs.",
            "match":{"hosts":["example.test"]}, "params":[{"name":"payload"}],
            "body":{"source_param":"payload","encoding":"json","source_field":"inner",
                "field_encoding":"base64_latin1_json",
                "params":[{"name":"data","format":{"kind":"regex","pattern":"^café$"}}]}
        })
        .to_string(),
    )
    .unwrap();
    assert!(
        pack.validate(&encoded_request(r#"{"inner":"eyJkYXRhIjoiY2Fm6SJ9"}"#))
            .violations
            .is_empty()
    );
    let report = pack.validate(&encoded_request(r#"{"inner":"eyJkYXRhIjo0Mn0="}"#));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("payload")
    );
}

#[test]
fn root_array_scopes_check_element_fields_and_do_not_fall_back_on_empty_arrays() {
    let manifest = json!({
        "id":"custom/root", "display_name":"Root alternatives", "description":"Root object or array.","source_level":"heuristic",
        "match":{"hosts":["example.test"],"json_paths":[{"any_of":["[].data","data"]}]},
        "body":{"scope":["[]",""], "params":[{"name":"data","requirement":"required","json_type":"string"}]},
    });
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    assert!(
        pack.validate(&request(r#"[{"data":"ok"}]"#))
            .violations
            .is_empty()
    );
    assert_eq!(
        pack.validate(&request(r#"[{"data":"ok"},{"data":1}]"#))
            .violations[0]
            .targets[0]
            .name
            .as_deref(),
        Some("[1].data")
    );
    let mut engine = Engine::new();
    engine.register(pack);
    let report = engine
        .validate(
            &request("[]"),
            &ValidationOptions {
                only_rulepacks: vec!["custom/root".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(report.reports[0].violations.len(), 1);
    assert_eq!(
        report.reports[0].violations[0].code,
        "custom.root.payload_mismatch"
    );
}

#[test]
fn typed_non_empty_containers_retain_empty_checks_and_nullable_values_are_allowed() {
    let pack = pack(
        json!([{"name":"data","json_type":["object","null"],"format":{"kind":"non_empty"}}]),
        None,
    );
    assert_eq!(
        pack.validate(&request(r#"{"data":{}}"#)).violations[0].code,
        "custom.constraints.body.data.empty"
    );
    assert!(
        pack.validate(&request(r#"{"data":null}"#))
            .violations
            .is_empty()
    );
}

#[test]
fn strict_object_families_allow_declared_keys_and_report_unknown_keys_once() {
    let pack = pack(
        json!([
            {"name":"data"},
            {"name":"unknown","name_pattern_parent":"","name_pattern":".*","requirement":"forbidden"},
        ]),
        None,
    );
    assert!(
        pack.validate(&request(r#"{"data":1}"#))
            .violations
            .is_empty()
    );
    let report = pack.validate(&request(r#"{"data":1,"extra":true}"#));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].code,
        "custom.constraints.body.extra.forbidden"
    );
}

#[test]
fn dynamic_families_only_read_their_own_immediate_parent() {
    let pack = pack(
        json!([
            {"name":"data"},
            {"name":"$add","json_type":"object"},
            {"name":"$union","json_type":"object"},
            {"name":"add_values","name_pattern_parent":"$add","name_pattern":".*","json_type":"number"},
            {"name":"union_values","name_pattern_parent":"$union","name_pattern":".*","json_type":"array"},
            {"name":"unknown","name_pattern_parent":"","name_pattern":".*","requirement":"forbidden"},
        ]),
        None,
    );
    assert!(
        pack.validate(&request(
            r#"{"data":1,"$add":{"visits":3},"$union":{"tags":["paid"]}}"#
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&request(
        r#"{"data":1,"$add":{"visits":[]},"$union":{"tags":3}}"#,
    ));
    assert_eq!(report.violations.len(), 2);
}

#[test]
fn calendar_formats_allow_only_the_requested_variants() {
    let pack = pack(
        json!([
            {"name":"data.date","format":{"kind":"date"}},
            {"name":"data.timestamp","format":{"kind":"datetime","require_timezone":true,"allow_basic":true,"allow_date_only":true}},
        ]),
        None,
    );
    for ts in [
        "20240229T125234+0900",
        "2024-02-29T12:52:34+0900",
        "20240229T12:52:34+09:00",
        "2024-02-29",
        "20240229",
    ] {
        assert!(
            pack.validate(&request(
                &json!({"data":{"date":"2024-02-29","timestamp":ts}}).to_string()
            ))
            .violations
            .is_empty(),
            "{ts}"
        );
    }
    for ts in [
        "20250229T125234+0900",
        "20241031T245234+0900",
        "20240229T125234+2500",
    ] {
        assert_eq!(
            pack.validate(&request(&json!({"data":{"timestamp":ts}}).to_string()))
                .violations
                .len(),
            1,
            "{ts}"
        );
    }
    assert_eq!(
        pack.validate(&request(r#"{"data":{"date":"20240229"}}"#))
            .violations
            .len(),
        1
    );
}

#[test]
fn decimal_multiples_have_exact_boundary_semantics() {
    for (step, valid, invalid) in [
        (
            json!(300000),
            vec!["0", "300000", "600000", "-300000", "3e5"],
            vec!["300001", "299999", "300000.1"],
        ),
        (
            json!(0.1),
            vec!["0.1", "0.3", "-0.7", "3e-1", "1"],
            vec!["0.31", "0.07", "1e-999"],
        ),
        (
            json!(0.25),
            vec!["0.25", "0.75", "1", "0"],
            vec!["0.1", "0.26", "0.025"],
        ),
    ] {
        let pack = pack(
            json!([{"name":"data","json_type":"number","multiple_of":step}]),
            None,
        );
        for value in valid {
            assert!(
                pack.validate(&request(&format!("{{\"data\":{value}}}")))
                    .violations
                    .is_empty(),
                "step {step}, valid {value}"
            );
        }
        for value in invalid {
            assert_eq!(
                pack.validate(&request(&format!("{{\"data\":{value}}}")))
                    .violations
                    .len(),
                1,
                "step {step}, invalid {value}"
            );
        }
    }
}

#[test]
fn numeric_field_comparisons_keep_exponents_and_large_integers_exact() {
    let pack = assertion_pack(
        json!([{"name":"low","json_type":"number"},{"name":"high","json_type":"number"}]),
        json!([
            {"code":"custom.constraints.order","kind":"less_equal","left":"low","right":"high","severity":"error","message":"The minimum exceeds the maximum."},
        ]),
        None,
    );
    for artifact in [
        r#"{"data":1,"low":1e3,"high":1000}"#,
        r#"{"data":1,"low":-10,"high":-2}"#,
    ] {
        assert!(pack.validate(&request(artifact)).violations.is_empty());
    }
    for artifact in [
        r#"{"data":1,"low":1e3,"high":999}"#,
        r#"{"data":1,"low":9007199254740993e0,"high":9007199254740992}"#,
    ] {
        assert_eq!(
            pack.validate(&request(artifact)).violations[0].code,
            "custom.constraints.order"
        );
    }
}

#[test]
fn native_type_guards_and_key_only_groups_accept_empty_labels() {
    let pack = assertion_pack(
        json!([
            {"name":"metadata", "json_type":["object","string","number","null"], "allow_empty":true},
            {"name":"metadata.url","allow_empty":true},
            {"name":"metadata.value","allow_empty":true},
            {"name":"metadata.amount"}, {"name":"metadata.currency"}
        ]),
        json!([{"code":"custom.constraints.rich_metadata","kind":"require_any_of",
            "groups":[["metadata.url","metadata.value"],["metadata.amount","metadata.currency"]],
            "allow_empty":true,"condition":{"kind":"json_type","param":"metadata","json_type":"object"},
            "severity":"error","message":"The object needs a complete rich metadata pair."}]),
        None,
    );
    for artifact in [
        r#"{"data":1,"metadata":{"url":"https://example.test","value":""}}"#,
        r#"{"data":1,"metadata":{"amount":0,"currency":"USD"}}"#,
        r#"{"data":1,"metadata":"plain"}"#,
        r#"{"data":1,"metadata":0}"#,
        r#"{"data":1,"metadata":null}"#,
        r#"{"data":1,"metadata":"[METADATA]"}"#,
    ] {
        assert!(
            pack.validate(&request(artifact)).violations.is_empty(),
            "{artifact}"
        );
    }
    for metadata in [
        json!({}),
        json!({"url":"https://example.test"}),
        json!({"amount":0}),
    ] {
        let report = pack.validate(&request(&json!({"data":1,"metadata":metadata}).to_string()));
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations[0].code,
            "custom.constraints.rich_metadata"
        );
    }
}

#[test]
fn presence_guards_include_empty_containers() {
    let pack = pack(
        json!([
        {"name":"data.operator", "allow_empty":true},
            {"name":"data.direct","requirement":"forbidden", "condition":{"kind":"present","param":"data.operator"}}
        ]),
        None,
    );
    for operator in [json!({}), json!([]), json!({"name":"value"})] {
        let report = pack.validate(&request(
            &json!({"data":{"operator":operator,"direct":"value"}}).to_string(),
        ));
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations[0].code,
            "custom.constraints.body.data.direct.forbidden"
        );
    }
    assert!(
        pack.validate(&request(r#"{"data":{"direct":"value"}}"#))
            .violations
            .is_empty()
    );
}

fn query_manifest() -> Value {
    json!({
        "id":"custom/constraints", "display_name":"Grouped requests", "description":"Checks independent query groups.",
        "source_level":"heuristic","param_style":"query_semicolon","match":{"hosts":["example.test"]},
        "query_scopes":[
            {"first_index":0,"last_index":0,"params":[{"name":"nw","requirement":"required"}]},
            {"first_index":2,"condition":{"kind":"value_in","param":"ptgt","values":["a"]},
             "params":[{"name":"ptgt"},{"name":"slid"},{"name":"slau"},{"name":"tpos","minimum":0,"format":{"kind":"regex","pattern":"^[0-9]+(?:\\.[0-9]+)?$"}}],
             "unique_by":["slid"],"rules":[{"code":"custom.constraints.slot","kind":"required_when_value",
                "when":"ptgt","equals":["a"],"requires":["slid","slau","tpos"],
                "severity":"error","message":"Each temporal slot needs its own fields."}]}
        ]
    })
}

fn url_request(artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::Url,
        ..request(artifact)
    }
}

#[test]
fn query_groups_keep_required_fields_and_uniqueness_in_their_own_namespace() {
    let pack = ManifestRulePack::from_json(&query_manifest().to_string()).unwrap();
    let positive = "https://example.test/?nw=1;slid=targeting;ptgt=a&slid=pre&slau=preroll&tpos=0;ptgt=s&slid=pre;ptgt=a&slid=mid&slau=midroll&tpos=10;opaque=x";
    assert!(pack.validate(&url_request(positive)).violations.is_empty());
    let missing = "https://example.test/?nw=1;slau=targeting;ptgt=a&slid=pre&slau=preroll&tpos=0;ptgt=a&slid=mid;tpos=10&slau=tail";
    let report = pack.validate(&url_request(missing));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].code, "custom.constraints.slot");
    assert_eq!(
        &missing[report.violations[0].targets[0].start..report.violations[0].targets[0].end],
        "ptgt=a"
    );
    let spoof = "https://example.test/?prof=x;nw=1";
    let report = pack.validate(&url_request(spoof));
    assert_eq!(
        report.violations[0].code,
        "custom.constraints.query.nw.missing"
    );
    assert_eq!(report.violations[0].field.as_deref(), Some("query[0].nw"));
    assert_eq!(
        &spoof[report.violations[0].targets[0].start..report.violations[0].targets[0].end],
        "prof=x"
    );
    let duplicate = positive.replace("slid=mid", "slid=pre");
    let report = pack.validate(&url_request(&duplicate));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].code,
        "custom.constraints.query.slid.duplicate"
    );
    assert_eq!(report.violations[0].field.as_deref(), Some("query[4].slid"));
    assert_eq!(report.violations[0].targets.len(), 2);
    let macros = positive
        .replace("slid=mid", "slid=[SLOT_ID]")
        .replace("slid=pre", "slid=[SLOT_ID]");
    assert!(pack.validate(&url_request(&macros)).violations.is_empty());
}

#[test]
fn query_scope_loader_rejects_invalid_names_ranges_and_transports() {
    for invalid in 0..5 {
        let mut manifest = query_manifest();
        match invalid {
            0 => manifest["param_style"] = json!("query"),
            1 => manifest["query_scopes"][0]["last_index"] = json!(0),
            2 => manifest["query_scopes"][1]["unique_by"] = json!(["unknown"]),
            3 => manifest["query_scopes"][1]["unique_by"] = json!(["slid", "slid"]),
            _ => manifest["query_scopes"][1]["condition"]["param"] = json!("unknown"),
        }
        if invalid == 1 {
            manifest["query_scopes"][0]["first_index"] = json!(1);
        }
        assert!(
            ManifestRulePack::from_json(&manifest.to_string()).is_err(),
            "case {invalid}"
        );
    }
}

#[test]
fn currency_formats_enforce_assigned_codes_with_explicit_case_and_history_policy() {
    for (format, positives, negatives) in [
        (
            json!({"kind":"currency"}),
            vec!["USD", "EUR", "XAD", "ZWG"],
            vec!["usd", "ZZZ", "BGN", "$US"],
        ),
        (
            json!({"kind":"currency","case_insensitive":true}),
            vec!["usd", "EuR"],
            vec!["ZZZ", "bgn", "üSD"],
        ),
        (
            json!({"kind":"currency","allow_historical":true}),
            vec!["BGN", "DEM", "USD"],
            vec!["ZZZ", "dem"],
        ),
    ] {
        let pack = pack(
            json!([{"name":"data","json_type":"string","format":format}]),
            None,
        );
        for currency in positives {
            assert!(
                pack.validate(&request(&json!({"data":currency}).to_string()))
                    .violations
                    .is_empty(),
                "{currency}"
            );
        }
        for currency in negatives {
            assert_eq!(
                pack.validate(&request(&json!({"data":currency}).to_string()))
                    .violations
                    .len(),
                1,
                "{currency}"
            );
        }
    }
}

#[test]
fn conditional_ip_ranges_keep_family_boundaries_and_unknown_templates() {
    let ip_pack = pack(
        json!([
            {"name":"data.primary","format":{"kind":"ip"}},
            {"name":"data.alternate","format":{"kind":"ip","version":"v4","exclude_ranges":["10.0.0.0/8","100.64.0.0/10"]},
             "condition":{"kind":"value_pattern","param":"data.primary","pattern":":"}}
        ]),
        None,
    );
    for (primary, alternate) in [
        ("2001:db8::1", "9.255.255.255"),
        ("2001:db8::1", "11.0.0.0"),
        ("2001:db8::1", "100.63.255.255"),
        ("2001:db8::1", "100.128.0.0"),
        ("8.8.8.8", "ignored"),
        ("[PRIMARY_IP]", "ignored"),
    ] {
        assert!(
            ip_pack
                .validate(&request(
                    &json!({"data":{"primary":primary,"alternate":alternate}}).to_string()
                ))
                .violations
                .is_empty()
        );
    }
    for alternate in [
        "10.0.0.0",
        "10.255.255.255",
        "100.64.0.0",
        "100.127.255.255",
        "2001:db8::2",
    ] {
        let report = ip_pack.validate(&request(
            &json!({"data":{"primary":"2001:db8::1","alternate":alternate}}).to_string(),
        ));
        assert_eq!(report.violations.len(), 1, "{alternate}");
        assert_eq!(
            report.violations[0].code,
            "custom.constraints.body.data.alternate.invalid"
        );
    }
    for (range, value, invalid) in [
        ("0.0.0.0/0", "8.8.8.8", true),
        ("::/0", "2001:db8::1", true),
        ("::/0", "8.8.8.8", false),
        ("2001:db8::/128", "2001:db8::", true),
        ("2001:db8::/128", "2001:db8::1", false),
    ] {
        let pack = pack(
            json!([{"name":"data","format":{"kind":"ip","exclude_ranges":[range]}}]),
            None,
        );
        assert_eq!(
            !pack
                .validate(&request(&json!({"data":value}).to_string()))
                .violations
                .is_empty(),
            invalid
        );
    }
}

#[test]
fn malformed_condition_patterns_type_unions_and_cidr_ranges_fail_loading() {
    for condition in [
        json!({"kind":"value_pattern","param":"data","pattern":"("}),
        json!({"kind":"json_type","param":"data","json_type":[]}),
        json!({"kind":"any","conditions":[]}),
        json!({"kind":"not","condition":{"kind":"value_pattern","param":"data","pattern":"("}}),
    ] {
        let manifest = json!({"id":"custom/constraints","display_name":"Invalid conditions","description":"Invalid condition test.",
            "source_level":"heuristic","match":{"hosts":["example.test"]},
            "params":[{"name":"data","condition":condition}]});
        assert!(ManifestRulePack::from_json(&manifest.to_string()).is_err());
    }
    for range in ["10.0.0.0/33", "::/129", "invalid/8", "10.0.0.0"] {
        let manifest = json!({"id":"custom/constraints","display_name":"Invalid ranges","description":"Invalid range test.",
            "source_level":"heuristic","match":{"hosts":["example.test"]},
            "params":[{"name":"data","format":{"kind":"ip","exclude_ranges":[range]}}]});
        assert!(
            ManifestRulePack::from_json(&manifest.to_string()).is_err(),
            "{range}"
        );
    }
}

#[test]
fn effective_timestamp_windows_honor_per_item_overrides_and_exact_microseconds() {
    let params = json!([
        {"name":"time","json_type":"integer"},
        {"name":"request_time","root_path":"time","json_type":"integer"},
        {"name":"request_policy","root_path":"policy","json_type":"string"}
    ]);
    let rules = json!([{"code":"custom.constraints.age","kind":"time_window","param":"time",
        "fallback_param":"request_time","unit":"microseconds","max_age_seconds":10,"max_future_seconds":2,
        "condition":{"kind":"value_in","param":"request_policy","values":["strict"]},
        "severity":"error","message":"Effective timestamp is outside the accepted window."}]);
    let manifest = json!({"id":"custom/constraints","display_name":"Effective timestamps","description":"Per-item timestamp precedence.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":[{"scope":"data[]","params":params,"rules":rules},
                {"scope":"properties.*","params":params,"rules":rules}]});
    let mut engine = Engine::default();
    engine.register(ManifestRulePack::from_json(&manifest.to_string()).unwrap());
    let options = ValidationOptions {
        only_rulepacks: vec!["custom/constraints".to_string()],
        except_rulepacks: vec![],
    };
    for artifact in [
        r#"{"data":[{"time":100000000},{"time":90000000},{"time":102000000}],"time":1,"policy":"strict"}"#,
        r#"{"data":[{}],"time":90000000,"policy":"strict"}"#,
        r#"{"data":[{"time":1},{}],"time":1,"policy":"relaxed"}"#,
        r#"{"data":[{"time":"[TIME]"}],"time":1,"policy":"strict"}"#,
        r#"{"data":[{}],"policy":"strict"}"#,
    ] {
        let report = engine
            .validate_at(&request(artifact), &options, 100)
            .unwrap();
        assert!(
            report
                .reports
                .iter()
                .all(|report| report.violations.is_empty()),
            "{artifact}"
        );
    }
    let mixed = r#"{"data":[{"time":90000000},{},{"time":89999999},{"time":102000001}],"time":1,"policy":"strict","properties":{"current":{"time":100000000},"inherited":{}}}"#;
    let report = engine.validate_at(&request(mixed), &options, 100).unwrap();
    let violations: Vec<_> = report
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .collect();
    assert_eq!(violations.len(), 4);
    let targets: Vec<_> = violations
        .iter()
        .map(|violation| violation.targets[0].name.as_deref().unwrap())
        .collect();
    assert_eq!(targets, ["time", "data[2].time", "data[3].time", "time"]);
    let mut invalid = manifest.clone();
    invalid["body"][0]["params"][1]["root_path"] = json!("broken[");
    assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
}

#[test]
fn scalar_format_assertions_validate_calendars_without_duplicate_field_contracts() {
    let pack = assertion_pack(
        json!([{"name":"date","json_type":"string"}]),
        json!([
            {"code":"custom.constraints.calendar","kind":"format","param":"date",
             "format":{"kind":"datetime","require_timezone":true,"allow_space_separator":true},
             "severity":"error","message":"The timestamp must use a valid calendar date."}
        ]),
        None,
    );
    for date in [
        "2024-02-29 12:00:00+00:00",
        "2025-01-01 12:00:00-07:00",
        "[CONVERSION_TIME]",
    ] {
        assert!(
            pack.validate(&request(&json!({"data":1,"date":date}).to_string()))
                .violations
                .is_empty()
        );
    }
    let report = pack.validate(&request(r#"{"data":1,"date":"2025-02-29 12:00:00+00:00"}"#));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].code, "custom.constraints.calendar");
}

#[test]
fn strict_timestamp_order_compares_offsets_and_full_fractional_precision() {
    let pack = assertion_pack(
        json!([{"name":"start"},{"name":"end"}]),
        json!([
            {"code":"custom.constraints.order","kind":"less_equal","left":"start","right":"end",
             "strict":true,"unit":"datetime","severity":"error","message":"The conversion must follow the call."}
        ]),
        None,
    );
    for (start, end) in [
        ("2025-01-01 12:00:00+02:00", "2025-01-01 11:00:00+00:00"),
        ("2025-01-01T10:00:00.000001Z", "2025-01-01T10:00:00.000002Z"),
        ("1969-12-31T23:59:59.000001Z", "1969-12-31T23:59:59.000002Z"),
        ("[CALL_TIME]", "2025-01-01T11:00:00Z"),
    ] {
        assert!(
            pack.validate(&request(
                &json!({"data":1,"start":start,"end":end}).to_string()
            ))
            .violations
            .is_empty()
        );
    }
    for (start, end) in [
        ("2025-01-01 12:00:00+02:00", "2025-01-01 10:00:00+00:00"),
        ("2025-01-01T10:00:00.000002Z", "2025-01-01T10:00:00.000001Z"),
        ("1969-12-31T23:59:59.000002Z", "1969-12-31T23:59:59.000001Z"),
    ] {
        assert_eq!(
            pack.validate(&request(
                &json!({"data":1,"start":start,"end":end}).to_string()
            ))
            .violations
            .len(),
            1
        );
    }
}

#[test]
fn numeric_string_unions_apply_bounds_only_when_explicitly_declared() {
    let pack = pack(
        json!([{"name":"data","json_type":["integer","string"],"numeric_strings":true,
        "minimum":-2147483648_i64,"maximum":2147483647}]),
        None,
    );
    for value in [
        json!(2147483647),
        json!("2147483647"),
        json!(-2147483648_i64),
        json!("-2147483648"),
    ] {
        assert!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .is_empty()
        );
    }
    for value in [
        json!(2147483648_u64),
        json!("2147483648"),
        json!("-2147483649"),
        json!("label"),
    ] {
        assert_eq!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .len(),
            1
        );
    }
}

#[test]
fn monetary_path_items_read_currency_only_from_the_global_namespace() {
    let pack = ManifestRulePack::from_json(&json!({
        "id":"custom/constraints","display_name":"Monetary baskets","description":"Each monetary item reads the global currency.",
        "source_level":"heuristic","param_style":"colon_path","match":{"hosts":["example.test"]},
        "query_scopes":[{"source":"path_items","first_index":1,"global_params":["currency"],
            "params":[{"name":"value"},{"name":"currency","allow_empty":true}],
            "rules":[{"code":"custom.constraints.currency","kind":"required_with","when":"value","requires":["currency"],"severity":"error","message":"Monetary items require global currency."}]}]
    }).to_string()).unwrap();
    assert!(
        pack.validate(&url_request(
            "https://example.test/conversion/currency:USD/[]/[value:10]"
        ))
        .violations
        .is_empty()
    );
    for url in [
        "https://example.test/conversion/[currency:USD]/[value:10]",
        "https://example.test/conversion/[]/[value:10/currency:USD]",
        "https://example.test/conversion/[]/[value:10]/[currency:USD]",
    ] {
        let report = pack.validate(&url_request(url));
        assert!(
            report
                .violations
                .iter()
                .any(|finding| finding.code == "custom.constraints.currency"),
            "{url}"
        );
    }
}

#[test]
fn bracketed_path_items_cannot_satisfy_another_items_or_global_requirements() {
    let pack = ManifestRulePack::from_json(&json!({
        "id":"custom/constraints","display_name":"Path baskets","description":"Independent path item requirements.",
        "source_level":"heuristic","param_style":"colon_path","match":{"hosts":["example.test"]},
        "params":[{"name":"campaign","requirement":"required"}],
        "query_scopes":[{"source":"path_items","params":[{"name":"category","requirement":"required"},{"name":"quantity","format":{"kind":"integer"},"minimum":1}]}]
    }).to_string()).unwrap();
    let valid = "https://example.test/conversion/campaign:1/[category:A/quantity:1]/[category:B/quantity:2]";
    assert!(pack.validate(&url_request(valid)).violations.is_empty());
    let url = "https://example.test/conversion/[campaign:1/category:A/quantity:1]/[quantity:2]/[]";
    let report = pack.validate(&url_request(url));
    assert_eq!(report.violations.len(), 3);
    assert!(
        report
            .violations
            .iter()
            .any(|finding| finding.code.ends_with("param.campaign.missing"))
    );
    let missing_items: Vec<_> = report
        .violations
        .iter()
        .filter(|finding| finding.code.ends_with("path_items.category.missing"))
        .collect();
    assert_eq!(missing_items.len(), 2);
    assert_eq!(
        &url[missing_items[0].targets[0].start..missing_items[0].targets[0].end],
        "[quantity:2]"
    );
    assert_eq!(
        &url[missing_items[1].targets[0].start..missing_items[1].targets[0].end],
        "[]"
    );
    assert!(
        pack.validate(&url_request(
            "https://example.test/conversion/campaign:1/[BASKET]"
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&url_request(
        "https://example.test/conversion/campaign:1/[category:A",
    ));
    assert!(
        report
            .violations
            .iter()
            .any(|finding| finding.code.ends_with("path_items.malformed"))
    );
}

#[test]
fn self_hosted_endpoints_require_an_explicit_narrow_path() {
    let manifest = json!({"id":"custom/constraints","display_name":"Self hosted","description":"A documented endpoint on configurable hosts.",
        "source_level":"heuristic","match":{"any_host":true,"paths":["/tracker.php"]},"params":[{"name":"id","requirement":"required"}]});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let report = pack.validate(&url_request("https://customer.example/tracker.php"));
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].code.ends_with("param.id.missing"));
    let mut engine = Engine::new();
    engine.register(ManifestRulePack::from_json(&manifest.to_string()).unwrap());
    let automatic = engine
        .validate(
            &url_request("https://customer.example/tracker.php"),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(automatic.reports.len(), 1);
    assert!(
        automatic.reports[0].violations[0]
            .code
            .ends_with("param.id.missing")
    );
    assert!(
        pack.validate(&url_request("https://customer.example/other.php"))
            .violations[0]
            .code
            .ends_with("endpoint_mismatch")
    );
    for matcher in [
        json!({"any_host":true}),
        json!({"any_host":true,"paths":["/"]}),
        json!({"any_host":true,"path_contains":["tracker"]}),
        json!({"any_host":true,"paths":["/tracker.php"],"path_contains":["/"]}),
    ] {
        let mut invalid = manifest.clone();
        invalid["match"] = matcher;
        assert!(ManifestRulePack::from_json(&invalid.to_string()).is_err());
    }
}

#[test]
fn utc_local_timestamp_windows_use_the_vendor_declared_timezone() {
    let pack = assertion_pack(
        json!([{"name":"time","json_type":"string","format":{"kind":"datetime","allow_space_separator":true}}]),
        json!([{"code":"custom.constraints.age","kind":"time_window","param":"time","unit":"datetime_utc","max_age_seconds":86400,"max_future_seconds":0,"severity":"error","message":"UTC timestamp window."}]),
        None,
    );
    let mut engine = Engine::new();
    engine.register(pack);
    let options = ValidationOptions {
        only_rulepacks: vec!["custom/constraints".to_string()],
        ..ValidationOptions::default()
    };
    for time in [
        "1970-01-02 00:00:00",
        "1970-01-03 00:00:00",
        "1970-01-02T02:00:00+02:00",
    ] {
        assert!(
            engine
                .validate_at(
                    &request(&json!({"data":1,"time":time}).to_string()),
                    &options,
                    172800
                )
                .unwrap()
                .reports
                .iter()
                .all(|report| report.violations.is_empty()),
            "{time}"
        );
    }
    for time in ["1970-01-01 23:59:59", "1970-01-03 00:00:01"] {
        assert!(
            engine
                .validate_at(
                    &request(&json!({"data":1,"time":time}).to_string()),
                    &options,
                    172800
                )
                .unwrap()
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .any(|finding| finding.code == "custom.constraints.age")
        );
    }
}

#[test]
fn ancestor_fields_and_key_existence_keep_each_frame_branch_independent() {
    let pack = assertion_pack(
        json!([
            {"name":"stack_type","ancestor_path":{"levels":2,"path":"type"}},
            {"name":"context_line","allow_empty":true}, {"name":"platform","json_type":"string",
             "requirement":"required","condition":{"kind":"all","conditions":[
                 {"kind":"not","condition":{"kind":"value_in","param":"stack_type","values":["resolved"]}},
                 {"kind":"exists","param":"context_line"}]}}
        ]),
        json!([]),
        Some("data[].stack.frames[]"),
    );
    let payload = r#"{"data":[{"stack":{"type":"resolved","frames":[{"context_line":null}]}},{"stack":{"frames":[{"context_line":null},{"context_line":""},{"platform":"python","context_line":[]},{}]}}]}"#;
    let report = pack.validate(&request(payload));
    assert_eq!(report.violations.len(), 2);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data[1].stack.frames[0].platform")
    );
    assert_eq!(
        report.violations[1].targets[0].name.as_deref(),
        Some("data[1].stack.frames[1].platform")
    );
    let direct = pack.validate(&request(
        r#"{"data":[{"stack":{"type":"raw","frames":[{"context_line":[],"platform":3}]}}]}"#,
    ));
    assert_eq!(direct.violations.len(), 1);
    assert!(direct.violations[0].code.ends_with("platform.invalid"));
    for invalid in [
        json!({"levels":0,"path":"type"}),
        json!({"levels":2,"path":"broken["}),
    ] {
        let body = json!({"params":[{"name":"data","ancestor_path":invalid}]});
        let manifest = json!({"id":"custom/constraints","display_name":"Invalid","description":"Invalid ancestor.","source_level":"heuristic","match":{"hosts":["example.test"]},"body":body});
        assert!(ManifestRulePack::from_json(&manifest.to_string()).is_err());
    }
}

#[test]
fn wildcard_aliases_directly_under_an_array_scope_preserve_row_identity() {
    let pack = pack(
        json!([{"name":"[].field","aliases":["[].alt"],"requirement":"required","json_type":"string"}]),
        Some("data"),
    );
    let report = pack.validate(&request(r#"{"data":[{"alt":"ok"},{}]}"#));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data[1].field")
    );
}

#[test]
fn encoded_query_sources_share_the_declared_logical_alias() {
    let manifest = json!({"id":"custom/embedded","display_name":"Encoded alias","description":"Encoded query fields use aliases.","source_level":"heuristic","match":{"hosts":["example.test"]},
        "params":[{"name":"data","aliases":["payload"]}],"body":{"source_param":"payload","params":[{"name":"amount","json_type":"number","minimum":0}]}});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let report = pack.validate(&url_request(
        "https://example.test/?payload=%7B%22amount%22%3A-1%7D",
    ));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("payload")
    );
}

#[test]
fn json_string_format_preserves_all_json_value_types() {
    let pack = pack(
        json!([{"name":"data","json_type":"string","format":{"kind":"json"}}]),
        None,
    );
    for text in [
        "null",
        "[]",
        "{}",
        "true",
        "1",
        "\"plain\"",
        "{EVENT_VALUE}",
    ] {
        assert!(
            pack.validate(&request(&json!({"data":text}).to_string()))
                .violations
                .is_empty(),
            "{text}"
        );
    }
    for text in ["arbitrary string", r#"{"broken":}"#, "1,2"] {
        assert_eq!(
            pack.validate(&request(&json!({"data":text}).to_string()))
                .violations
                .len(),
            1,
            "{text}"
        );
    }
}

#[test]
fn wildcard_field_aliases_satisfy_only_their_own_nested_array_slot() {
    let pack = pack(
        json!([
            {"name":"pairs.rows[].field","aliases":["pairs.rows[].field_alt","pairs.row_alias[].field","pairs.row_alias[].field_alt"],"requirement":"required","json_type":"string"}
        ]),
        Some("data[]"),
    );
    let payload = r#"{"data":[{"pairs":{"rows":[{"field":"first"},{"field_alt":"second"},{}]}},{"pairs":{"row_alias":[{"field_alt":"third"},{}]}}]}"#;
    let report = pack.validate(&request(payload));
    // Each absent row reports once across alternate spellings and cannot use
    // an identifier supplied in a different array element.
    assert_eq!(report.violations.len(), 2);
    assert!(
        report
            .violations
            .iter()
            .all(|finding| finding.code.ends_with(".missing"))
    );
    assert!(report.violations.iter().all(|finding| {
        let path = finding.targets[0].name.as_deref().unwrap();
        path.contains("rows[2]") || path.contains("row_alias[1]")
    }));
    assert!(pack.validate(&request(r#"{"data":[{"pairs":{"rows":[{"field":"first"},{"field_alt":"second"}]}},{"pairs":{"row_alias":[{"field_alt":"third"}]}}]}"#)).violations.is_empty());
}

#[test]
fn alias_presence_conditions_and_assertions_use_the_same_logical_field() {
    let params = json!([{ "name":"api_key", "aliases":["token"], "requirement":"required" },
        {"name":"value","condition":{"kind":"present","param":"token"},"json_type":"integer"},
        {"name":"currency"}]);
    let rules = json!([{"code":"custom.constraints.currency","kind":"required_with","when":"token","requires":["currency"],
        "severity":"error","message":"The token requires a currency."}]);
    let pack = assertion_pack(params, rules, None);
    assert!(
        pack.validate(&request(
            r#"{"data":1,"token":"abc","currency":"USD","value":1}"#
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&request(r#"{"data":1,"token":"abc","value":"bad"}"#));
    assert_eq!(report.violations.len(), 2);
    assert_eq!(report.violations[1].code, "custom.constraints.currency");
    assert_eq!(
        report.violations[1].targets[0].name.as_deref(),
        Some("token")
    );
    let report = pack.validate(&request(r#"{"data":1}"#));
    assert_eq!(
        report
            .violations
            .iter()
            .filter(|violation| violation.code.ends_with("api_key.missing"))
            .count(),
        1
    );
    let url_pack = ManifestRulePack::from_json(&json!({"id":"custom/constraints","display_name":"Alias URL","description":"URL alias relationships.",
        "source_level":"heuristic","match":{"hosts":["example.test"]},
        "params":[{"name":"token","aliases":["key"],"requirement":"required"},{"name":"currency"}],
        "rules":[{"code":"custom.constraints.currency","kind":"required_with","when":"key","requires":["currency"],"severity":"error","message":"The key requires currency."}]}).to_string()).unwrap();
    let url = "https://example.test/?key=abc";
    let report = url_pack.validate(&url_request(url));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].targets[0].name.as_deref(), Some("key"));
    assert_eq!(
        &url[report.violations[0].targets[0].start..report.violations[0].targets[0].end],
        "key=abc"
    );
}

#[test]
fn null_as_unset_is_explicit_and_does_not_hide_null_array_members() {
    let pack = assertion_pack(
        json!([
            {"name":"optional","json_type":"string","null_as_missing":true},
            {"name":"required","json_type":"string","null_as_missing":true,"requirement":"required"},
            {"name":"currency"},{"name":"items[]","json_type":"object"}
        ]),
        json!([{"code":"custom.constraints.currency","kind":"required_with","when":"optional","requires":["currency"],"severity":"error","message":"A set optional field needs currency."}]),
        None,
    );
    assert!(
        pack.validate(&request(
            r#"{"data":1,"optional":null,"required":"value","items":[{}]}"#
        ))
        .violations
        .is_empty()
    );
    let report = pack.validate(&request(
        r#"{"data":1,"optional":null,"required":null,"items":[null]}"#,
    ));
    assert_eq!(report.violations.len(), 2);
    assert_eq!(
        report.violations[0].code,
        "custom.constraints.body.required.missing"
    );
    assert_eq!(
        report.violations[1].code,
        "custom.constraints.body.items[].invalid"
    );
}

#[test]
fn scope_exclusions_remove_reserved_subtrees_and_keep_custom_members() {
    let manifest = json!({"id":"custom/constraints","display_name":"Scoped mutations","description":"Custom property mutations.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"scope":"data.**","scope_exclusions":["data.reserved","data[\"a.b\"]"],"params":[{"name":"","json_type":["object","array","string"],"max_length":3}]}});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    let report = pack.validate(&request(
        r#"{"data":{"reserved":{"nested":["long"]},"a.b":"long","custom":{"name":"long"}}}"#,
    ));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data.custom.name")
    );
}

#[test]
fn consent_formats_share_structural_checks_and_keep_macros_unknown() {
    for (kind, valid, invalid) in [
        (
            "tcf",
            "CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA",
            "CPXxRf",
        ),
        ("gpp", "DBABLA~CAAAVVVVVVRA.QA", "DCABMA"),
        ("us_privacy", "1YNN", "2YNN"),
    ] {
        let pack = pack(json!([{"name":"data","format":{"kind":kind}}]), None);
        for value in [valid, "REDACTED", "[CONSENT]"] {
            assert!(
                pack.validate(&request(&json!({"data":value}).to_string()))
                    .violations
                    .is_empty(),
                "{kind}:{value}"
            );
        }
        for value in [invalid, "not a consent string", "D"] {
            assert_eq!(
                pack.validate(&request(&json!({"data":value}).to_string()))
                    .violations
                    .len(),
                1,
                "{kind}:{value}"
            );
        }
    }
}

#[test]
fn calendar_formats_report_malformed_unicode_without_panicking() {
    let pack = pack(
        json!([{"name":"data","format":{"kind":"datetime","allow_basic":true}}]),
        None,
    );
    for value in [
        "2025-01-01T12:00:€Z",
        "2025-01-01T12€00Z",
        "2025-01-01T😀00Z",
    ] {
        assert_eq!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .len(),
            1
        );
    }
}

#[test]
fn joined_string_array_budgets_include_separators_and_unicode_without_coercion() {
    let pack = pack(
        json!([{"name":"data","json_type":"array","allow_empty":true,
        "max_joined_length":4,"join_separator":"||"}]),
        None,
    );
    for value in [
        json!([]),
        json!(["é", "é"]),
        json!(["", "ab"]),
        json!(["[VALUE]", "long"]),
        json!([42, "long"]),
    ] {
        assert!(
            pack.validate(&request(&json!({"data":value}).to_string()))
                .violations
                .is_empty()
        );
    }
    let report = pack.validate(&request(r#"{"data":["éé","a"]}"#));
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].message.contains("5"));
}

#[test]
fn named_brace_templates_defer_literal_ip_checks_and_fired_urls_report_macros() {
    // Named brace placeholders appear in the vendor's attribution-link example:
    // https://support.appsflyer.com/hc/en-us/articles/360011316197-Advanced-Privacy-guide-postback-macros-for-ad-networks
    let engine = Engine::default();
    for token in ["{IP}", "{SA_IP_ADDRESS}", "%7BIP%7D"] {
        let artifact =
            format!("https://impressions.onelink.me/AbCd?pid=partner_int&c=test&af_ip={token}");
        let mut req = ValidationRequest {
            artifact_kind: ArtifactKind::VastTracker,
            artifact,
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let summary = engine
            .validate(&req, &ValidationOptions::default())
            .unwrap();
        assert!(summary.is_ok(), "{token}: {:?}", summary.reports);
        req.expansion_state = ExpansionState::Fired;
        let summary = engine
            .validate(&req, &ValidationOptions::default())
            .unwrap();
        if !token.starts_with('%') {
            assert!(
                summary
                    .reports
                    .iter()
                    .flat_map(|r| &r.violations)
                    .any(|v| v.code == "core.macro.unexpanded_in_fired_url")
            );
        }
        assert!(
            !summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "vendor.appsflyer-onelink-impression.param.af_ip.invalid")
        );
    }
    let literal = ValidationRequest {
        artifact_kind: ArtifactKind::Url,
        artifact: "https://impressions.onelink.me/AbCd?pid=partner_int&c=test&af_ip=invalid".into(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    };
    assert!(
        engine
            .validate(&literal, &ValidationOptions::default())
            .unwrap()
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code == "vendor.appsflyer-onelink-impression.param.af_ip.invalid")
    );
}

#[test]
fn macro_values_defer_unknown_budgets_but_keep_native_container_structure() {
    let object = pack(
        json!([{"name":"data","json_type":"object","min_properties":1,"max_compact_bytes":10}]),
        None,
    );
    for artifact in [
        r#"{"data":{"x":"[HASH]"}}"#,
        r#"{"data":{"x":"\u005bHASH\u005d"}}"#,
        r#"{"data":{"[KEY]":"literal"}}"#,
    ] {
        assert!(
            object.validate(&request(artifact)).violations.is_empty(),
            "{artifact}"
        );
    }
    for artifact in [r#"{"data":"[HASH]"}"#, r#"{"data":{}}"#] {
        assert_eq!(
            object.validate(&request(artifact)).violations.len(),
            1,
            "{artifact}"
        );
    }
    let keys = pack(
        json!([{"name":"data.*","max_key_length":3,"key_pattern":"^[a-z]+$"}]),
        None,
    );
    assert!(
        keys.validate(&request(r#"{"data":{"[LONG_KEY]":"literal"}}"#))
            .violations
            .is_empty()
    );
    assert_eq!(
        keys.validate(&request(r#"{"data":{"long":"[HASH]"}}"#))
            .violations
            .len(),
        1
    );
    let arrays = pack(
        json!([{"name":"data","json_type":"array","max_items":1,"max_compact_bytes":4}]),
        None,
    );
    assert!(
        arrays
            .validate(&request(r#"{"data":["[HASH]"]}"#))
            .violations
            .is_empty()
    );
    assert_eq!(
        arrays
            .validate(&request(r#"{"data":["[HASH]","[OTHER]"]}"#))
            .violations
            .len(),
        1
    );
    let scalar = pack(
        json!([{"name":"data","json_type":"integer","minimum":1}]),
        None,
    );
    assert!(
        scalar
            .validate(&request(r#"{"data":"[TIME]"}"#))
            .violations
            .is_empty()
    );
}

#[test]
fn compact_json_budget_counts_utf8_after_removing_whitespace_and_escape_spelling() {
    let pack = pack(
        json!([{"name":"data","json_type":"object","max_compact_bytes":10}]),
        None,
    );
    for artifact in [r#"{"data":{"x":"é"}}"#, r#"{"data": { "x" : "\u00e9" } }"#] {
        assert!(pack.validate(&request(artifact)).violations.is_empty());
    }
    let report = pack.validate(&request(r#"{"data":{"x":"éé"}}"#));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].targets[0].name.as_deref(),
        Some("data")
    );
}

#[test]
fn conditional_member_name_checks_apply_to_the_selected_object_key() {
    let manifest = json!({"id":"custom/constraints","display_name":"Attribute keys","description":"Selected object attribute keys.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"scope":"data.*","params":[{"name":"","json_type":["object","array","string","number"],"max_key_length":3,"key_pattern":"^[^ .$]+$",
            "condition":{"kind":"json_type","param":"","json_type":"object"}}]}});
    let pack = ManifestRulePack::from_json(&manifest.to_string()).unwrap();
    assert!(
        pack.validate(&request(r#"{"data":{"ééé":{},"a b":0,"name":["a"]}}"#))
            .violations
            .is_empty()
    );
    for key in ["a b", "a.b", "$a", "éééé"] {
        let report = pack.validate(&request(&json!({"data":{key:{}}}).to_string()));
        assert_eq!(report.violations.len(), 1, "{key}");
    }
}
