//! Independent universal-condition oracles for observed and unavailable fields.
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug)]
enum Placement {
    Scope,
    Parameter,
    Assertion,
}

fn engine(placement: Placement, negated: bool) -> Engine {
    let guard = json!({"kind":"all_values_in",
        "params":["query.mode","query.mode[]","headers.x-mode"],"values":["3"]});
    let condition = if negated {
        json!({"kind":"not","condition":guard})
    } else {
        guard
    };
    let mut http = json!({"params":[
        {"name":"query.mode","allow_empty":true},
        {"name":"query.mode[]","allow_empty":true},
        {"name":"headers.x-mode"},
        {"name":"method","requirement":"required",
         "format":{"kind":"enum","values":["POST"]}}
    ]});
    match placement {
        Placement::Scope => http["condition"] = condition,
        Placement::Parameter => http["params"][3]["condition"] = condition,
        Placement::Assertion => {
            http["params"][3].as_object_mut().unwrap().remove("format");
            http["rules"] = json!([{"code":"custom.capture-values.method",
                "kind":"format","param":"method","format":{"kind":"enum","values":["POST"]},
                "condition":condition,"severity":"error","message":"An applicable request requires POST."}]);
        }
    }
    let mut engine = Engine::new();
    engine.register_manifest_json(&json!({
        "id":"custom/capture-values","display_name":"Capture values",
        "description":"Every mode occurrence must agree before applying a conditional method contract.",
        "source_level":"heuristic","match":{"hosts":["capture-values.example"]},"http":http
    }).to_string()).unwrap();
    engine
}

fn check(
    id: &str,
    query: &str,
    header: Option<&str>,
    capture: Value,
    proven: Option<bool>,
    incomplete: bool,
) {
    for placement in [Placement::Scope, Placement::Parameter, Placement::Assertion] {
        for negated in [false, true] {
            let headers = header.map_or_else(|| json!({}), |value| json!({"X-Mode":value}));
            let request = ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: json!({"url":format!("https://capture-values.example/events?{query}"),
                    "method":"GET","headers":headers,"capture":capture})
                .to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            };
            let summary = engine(placement, negated)
                .validate_at(&request, &ValidationOptions::default(), 1_800_000_000)
                .unwrap();
            let mut found: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .map(|finding| (finding.code.clone(), finding.severity))
                .collect();
            found.sort_by(|left, right| left.0.cmp(&right.0));
            let mut expected = Vec::new();
            if incomplete {
                expected.push((
                    "core.request.capture_incomplete".to_string(),
                    Severity::Info,
                ));
            }
            if proven.is_some_and(|value| value != negated) {
                let code = match placement {
                    Placement::Assertion => "custom.capture-values.method",
                    _ => "custom.capture-values.http.method.invalid",
                };
                expected.push((code.to_string(), Severity::Error));
            }
            expected.sort_by(|left, right| left.0.cmp(&right.0));
            assert_eq!(
                found, expected,
                "{id}/{placement:?}/negated={negated}: {summary:?}"
            );
        }
    }
}

#[test]
fn unavailable_members_prevent_universal_truth_but_known_conflicts_prove_false() {
    let unknown = json!({"unavailable_headers":["x-mode"]});
    check(
        "unknown-with-visible3",
        "mode=3",
        None,
        unknown.clone(),
        None,
        true,
    );
    check(
        "unknown-with-visible1",
        "mode=1",
        None,
        unknown.clone(),
        Some(false),
        true,
    );
    check(
        "unknown-with-no-visible-mode",
        "",
        None,
        unknown,
        None,
        true,
    );
    check(
        "redacted-observed3",
        "mode=3",
        Some("3"),
        json!({"redacted_headers":["x-mode"]}),
        None,
        true,
    );
    check(
        "redacted-with-visible-conflict",
        "mode=1",
        Some("3"),
        json!({"redacted_headers":["x-mode"]}),
        Some(false),
        true,
    );
    check(
        "populated-unavailable-is-observed",
        "mode=3",
        Some("3"),
        json!({"unavailable_headers":["x-mode"]}),
        Some(true),
        false,
    );
}

#[test]
fn complete_observations_keep_agreement_disagreement_and_nonvacuous_truth() {
    for (id, query, header, proven) in [
        ("all-observed3", "mode=3", Some("3"), true),
        ("agreeing-repeats", "mode=3&mode=3", Some("3"), true),
        ("conflicting-repeats", "mode=3&mode=1", Some("3"), false),
        ("conflicting-header", "mode=3", Some("1"), false),
        ("encoded-conflict", "mode=3&%6dode=1", Some("3"), false),
        ("empty-scalar-conflict", "mode=3&mode=", Some("3"), false),
        ("no-submitted-scalars", "", None, false),
    ] {
        check(id, query, header, json!({}), Some(proven), false);
    }
}

#[test]
fn macro_unknowns_defer_both_polarities_and_do_not_hide_known_conflicts() {
    for (id, query, header, proven) in [
        ("macro-query", "mode=3&mode=%5BMODE%5D", Some("3"), None),
        ("macro-header", "mode=3", Some("[MODE]"), None),
        (
            "macro-neighbor-conflict",
            "mode=1",
            Some("[MODE]"),
            Some(false),
        ),
    ] {
        check(id, query, header, json!({}), proven, false);
    }
}
