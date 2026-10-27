use pixellint_core::{
    ArtifactKind, ExpansionState, ManifestRulePack, Severity, ValidationRequest, ValidatorPlugin,
};
use serde_json::{Value, json};

fn manifest(max_items: usize, scope: Option<&str>) -> Value {
    let mut body = json!({
        "params": [
            {"name":"a", "aliases":["alternate_a"], "json_type":"array"},
            {"name":"b", "json_type":"array"}
        ],
        "rules":[{"code":"custom.budget.max", "kind":"max_total_array_items",
            "params":["a","b"], "max_items":max_items,
            "severity":"warning", "message":"Combined array budget."}]
    });
    if let Some(scope) = scope {
        body["scope"] = scope.into();
    }
    json!({"id":"custom/budget", "display_name":"Array budget", "description":"Combined native arrays.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["_profile"]}, "body":body})
}

fn validate(pack: &ManifestRulePack, mut value: Value) -> pixellint_core::ValidationReport {
    value["_profile"] = true.into();
    pack.validate(&ValidationRequest {
        artifact: value.to_string(),
        artifact_kind: ArtifactKind::JsonPayload,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    })
}

#[test]
fn combined_budget_counts_native_items_at_inclusive_boundaries() {
    let pack = ManifestRulePack::from_json(&manifest(2, None).to_string()).unwrap();
    for value in [
        json!({}),
        json!({"a":[],"b":[]}),
        json!({"a":[{},{}]}),
        json!({"a":[[1,2,3]],"b":[{"one":1,"two":2}]}),
    ] {
        assert!(
            validate(&pack, value.clone()).violations.is_empty(),
            "{value}"
        );
    }
    let report = validate(&pack, json!({"a":[1,2],"b":[null]}));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].severity, Severity::Warning);
    assert_eq!(report.violations[0].targets.len(), 2);
    let zero = ManifestRulePack::from_json(&manifest(0, None).to_string()).unwrap();
    assert!(validate(&zero, json!({"a":[]})).violations.is_empty());
    assert_eq!(validate(&zero, json!({"a":[false]})).violations.len(), 1);
}

#[test]
fn array_budget_uses_known_arrays_without_reinterpreting_scalars() {
    let pack = ManifestRulePack::from_json(&manifest(2, None).to_string()).unwrap();
    assert_eq!(
        validate(&pack, json!({"a":{"x":1,"y":2},"b":[1,2]}))
            .violations
            .len(),
        1
    );
    let report = validate(&pack, json!({"a":"[1,2,3]","b":[1,2]}));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].code, "custom.budget.body.a.invalid");
    assert!(
        !validate(&pack, json!({"a":"[ITEMS]","b":[1,2]}))
            .violations
            .iter()
            .any(|v| v.code == "custom.budget.max")
    );
    assert!(
        validate(&pack, json!({"a":["[A]","[B]"]}))
            .violations
            .is_empty()
    );
    let report = validate(&pack, json!({"a":null,"b":[1,2,3]}));
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.code == "custom.budget.max")
    );
}

#[test]
fn combined_budget_is_scoped_per_record_and_keeps_concrete_alias_targets() {
    let pack = ManifestRulePack::from_json(&manifest(2, Some("batches[]")).to_string()).unwrap();
    let report = validate(
        &pack,
        json!({"batches":[{"a":[1,2]}, {"alternate_a":[3,4],"b":[5]}]}),
    );
    assert_eq!(report.violations.len(), 1);
    let names: Vec<_> = report.violations[0]
        .targets
        .iter()
        .filter_map(|t| t.name.as_deref())
        .collect();
    assert_eq!(names, ["batches[1].alternate_a", "batches[1].b"]);
}

#[test]
fn combined_budget_rejects_ambiguous_or_empty_parameter_sets() {
    for names in [
        json!([]),
        json!(["a", "a"]),
        json!(["a", "alternate_a"]),
        json!(["missing"]),
    ] {
        let mut m = manifest(2, None);
        m["body"]["rules"][0]["params"] = names.clone();
        assert!(
            ManifestRulePack::from_json(&m.to_string()).is_err(),
            "{names}"
        );
    }
}

#[test]
fn native_array_budget_does_not_parse_url_values_as_arrays() {
    let pack = ManifestRulePack::from_json(&manifest(2, None).to_string()).unwrap();
    let report = pack.validate(&ValidationRequest {
        artifact: "https://example.test?a=%5B1,2,3%5D&b=%5B4,5,6%5D".into(),
        artifact_kind: ArtifactKind::Url,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    });
    assert!(report.violations.is_empty());
}
