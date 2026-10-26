//! Minimum timestamp ages use inclusive bounds on the supplied clock.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, ValidationOptions, ValidationRequest,
};
use serde_json::{Value, json};

fn manifest(unit: &str, bounds: Value) -> Value {
    let mut assertion = json!({
        "code":"custom/age.too_recent".replace('/', "."), "kind":"time_window",
        "param":"timestamp", "unit":unit, "severity":"error", "message":"Outside age bounds."
    });
    assertion
        .as_object_mut()
        .unwrap()
        .extend(bounds.as_object().unwrap().clone());
    json!({"id":"custom/age", "display_name":"Minimum age", "description":"Inclusive age contracts.",
        "source_level":"heuristic", "match":{"hosts":["example.test"],"json_paths":["events"]},
        "body":{"scope":"events[]","params":[{"name":"timestamp","allow_empty":true},{"name":"fallback","allow_empty":true}],"rules":[assertion]}})
}

fn findings(unit: &str, bounds: Value, timestamps: Value) -> Vec<String> {
    let mut engine = Engine::new();
    engine.register(ManifestRulePack::from_json(&manifest(unit, bounds).to_string()).unwrap());
    let result = engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::JsonPayload,
                artifact: json!({"events":timestamps}).to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
            1_000_000,
        )
        .unwrap();
    result
        .reports
        .into_iter()
        .flat_map(|report| report.violations.into_iter().map(|v| v.code))
        .collect()
}

#[test]
fn minimum_age_is_inclusive_in_seconds_milliseconds_and_microseconds() {
    for (unit, scale) in [
        ("seconds", 1_i64),
        ("milliseconds", 1000),
        ("microseconds", 1_000_000),
    ] {
        let boundary = 740_800 * scale;
        let codes = findings(
            unit,
            json!({"min_age_seconds":259200}),
            json!([
                {"timestamp":boundary-1},{"timestamp":boundary},{"timestamp":boundary+1}
            ]),
        );
        assert_eq!(codes, ["custom.age.too_recent"], "{unit}");
    }
}

#[test]
fn minimum_and_maximum_age_apply_to_every_record() {
    assert_eq!(findings("seconds",json!({"min_age_seconds":10,"max_age_seconds":20}),json!([
        {"timestamp":999979},{"timestamp":999980},{"timestamp":999990},{"timestamp":999991},{"timestamp":1000001}
    ])).len(),3);
}

#[test]
fn fractional_microseconds_do_not_round_across_the_minimum_age() {
    assert!(
        findings(
            "microseconds",
            json!({"min_age_seconds":1}),
            json!([
                {"timestamp":999999000000.0}
            ])
        )
        .is_empty()
    );
    assert_eq!(
        findings(
            "microseconds",
            json!({"min_age_seconds":1}),
            json!([
                {"timestamp":999999000000.1}
            ])
        )
        .len(),
        1
    );
}

#[test]
fn fallback_applies_only_when_the_primary_timestamp_is_absent() {
    assert_eq!(
        findings(
            "seconds",
            json!({"min_age_seconds":10,"fallback_param":"fallback"}),
            json!([
                {"fallback":1000000},{"timestamp":999980,"fallback":1000000}
            ])
        )
        .len(),
        1
    );
}

#[test]
fn missing_macro_and_nonnumeric_timestamps_do_not_invent_age_failures() {
    assert!(
        findings(
            "milliseconds",
            json!({"min_age_seconds":259200}),
            json!([
                {},{"timestamp":"[TIMESTAMP]"},{"timestamp":"unknown"},{"timestamp":null}
            ])
        )
        .is_empty()
    );
}

#[test]
fn reversed_age_bounds_are_rejected() {
    assert!(
        ManifestRulePack::from_json(
            &manifest(
                "seconds",
                json!({"min_age_seconds":20,"max_age_seconds":10})
            )
            .to_string()
        )
        .is_err()
    );
}
