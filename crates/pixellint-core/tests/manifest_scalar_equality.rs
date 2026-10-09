//! Native numeric equality without float rounding or lost duplicate evidence.

use pixellint_core::{
    ArtifactKind, ExpansionState, ManifestRulePack, ValidationRequest, ValidatorPlugin,
};
use serde_json::{Value, json};

fn pack(params: Value, rules: Value) -> ManifestRulePack {
    ManifestRulePack::from_json(
        &json!({"id":"custom/equality","display_name":"Scalar equality", "description":"Exact scalar comparisons.",
        "source_level":"heuristic","match":{"hosts":["example.test"],"json_paths":["data"]},
        "body":{"params":params,"rules":rules}})
        .to_string(),
    )
    .unwrap()
}

fn request(value: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::JsonPayload,
        artifact: value.into(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn native_number_conditions_match_exact_values_while_strings_remain_literal() {
    let pack = pack(
        json!([{"name":"data"}]),
        json!([{"code":"custom.equality.sentinel",
        "kind":"format","param":"data","format":{"kind":"enum","values":["never"]},
        "condition":{"kind":"value_in","param":"data","values":["-1","9007199254740993","0"]},
        "message":"Selected numeric value.","severity":"warning"}]),
    );
    for (value, selected) in [
        ("-1", true),
        ("-1.0", true),
        ("-1e0", true),
        ("-0.0", true),
        ("9007199254740993", true),
        ("90071992547409930e-1", true),
        ("9007199254740992", false),
        (r#""-1""#, true),
        (r#""-1.0""#, false),
        (r#""-1e0""#, false),
        ("true", false),
    ] {
        let report = pack.validate(&request(&format!("{{\"data\":{value}}}")));
        assert_eq!(
            report.violations.len(),
            usize::from(selected),
            "{value}: {:?}",
            report.violations
        );
    }
}

#[test]
fn extreme_numeric_exponents_preserve_identical_duplicate_evidence() {
    let pack = pack(
        json!([{"name":"data","json_type":"array"}]),
        json!([
        {"code":"custom.equality.duplicate","kind":"unique_array_by","param":"data","field":"type",
            "message":"Repeated native scalar value.","severity":"warning"}]),
    );
    for (left, right, duplicate) in [
        ("1e9223372036854775808", "1e9223372036854775808", true),
        ("1e-9223372036854775809", "1e-9223372036854775809", true),
        ("1e9223372036854775808", "2e9223372036854775808", false),
        ("1", "1.00e0", true),
        ("-0.0", "0", true),
        ("1", r#""1""#, false),
    ] {
        let artifact = format!("{{\"data\":[{{\"type\":{left}}},{{\"type\":{right}}}]}}");
        let report = pack.validate(&request(&artifact));
        assert_eq!(
            report.violations.len(),
            usize::from(duplicate),
            "{left}/{right}: {:?}",
            report.violations
        );
        if duplicate {
            let finding = &report.violations[0];
            assert_eq!(finding.code, "custom.equality.duplicate");
            assert_eq!(finding.targets.len(), 2);
            assert_eq!(
                &artifact[finding.targets[0].start..finding.targets[0].end],
                left
            );
            assert_eq!(
                &artifact[finding.targets[1].start..finding.targets[1].end],
                right
            );
        }
    }
}
