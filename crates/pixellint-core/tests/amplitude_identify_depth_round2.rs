use std::{collections::BTreeSet, fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct SourceCases {
    id: String,
    engine_derived: bool,
    network_calls_to_ingestion: usize,
    cases: Vec<SourceCase>,
}

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    category: String,
    fixture: String,
    kind: String,
    source_url: String,
    case_origin: String,
    source_requirement: String,
    expected_vendor_codes: Vec<String>,
}

fn check_category(category: &str) -> usize {
    let engine = Engine::default();
    let mut checked = 0;
    for slug in ["amplitude-identify", "amplitude-group-identify"] {
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/vendor-{slug}"));
        let source: SourceCases = serde_json::from_str(
            &fs::read_to_string(directory.join("source-cases-depth2.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(source.id, format!("vendor/{slug}"));
        assert!(!source.engine_derived);
        assert_eq!(source.network_calls_to_ingestion, 0);
        let mut ids = BTreeSet::new();
        for case in source.cases {
            assert!(ids.insert(case.id.clone()));
            if case.category != category {
                continue;
            }
            let label = format!("{slug}/{}", case.id);
            assert!(case.source_url.starts_with("https://amplitude.com/docs/"));
            assert!(!case.source_requirement.is_empty(), "{label}");
            assert!(
                [
                    "independently_authored_contract_boundary",
                    "official_request_example",
                    "official_operation_example_in_documented_carrier",
                ]
                .contains(&case.case_origin.as_str()),
                "{label}"
            );
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact: fs::read_to_string(directory.join(case.fixture)).unwrap(),
                        artifact_kind: match case.kind.as_str() {
                            "url" => ArtifactKind::Url,
                            "request" => ArtifactKind::NetworkRequest,
                            other => panic!("{label}: {other}"),
                        },
                        claimed_vendor: None,
                        expansion_state: ExpansionState::Unknown,
                    },
                    &ValidationOptions::default(),
                    1792886400,
                )
                .unwrap();
            let report = summary
                .reports
                .iter()
                .find(|report| report.plugin_id == source.id)
                .unwrap_or_else(|| panic!("{label}: intended pack was not selected"));
            let mut actual: Vec<_> = report
                .violations
                .iter()
                .map(|finding| finding.code.clone())
                .collect();
            actual.sort();
            assert_eq!(actual, case.expected_vendor_codes, "{label}");
            for finding in &report.violations {
                assert_eq!(finding.severity, Severity::Error, "{label}");
                assert_eq!(finding.source.level, RuleSourceLevel::OfficialVendor);
                assert!(
                    finding
                        .source
                        .reference
                        .as_deref()
                        .unwrap()
                        .starts_with(&case.source_url),
                    "{label}"
                );
                assert!(finding.field.is_some(), "{label}");
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no {category} cases checked");
    checked
}

#[test]
fn official_examples_preserve_identify_get_and_both_documented_form_endpoints() {
    assert_eq!(check_category("official"), 4);
}

#[test]
fn identify_unset_requires_the_literal_string_without_coercing_native_values() {
    assert_eq!(check_category("unset"), 22);
}

#[test]
fn both_numeric_operations_check_empty_multiline_and_path_like_custom_names() {
    assert_eq!(check_category("numeric"), 28);
}

#[test]
fn existing_identify_mixing_covers_every_known_operation_and_preserves_open_members() {
    assert_eq!(check_category("mixing"), 18);
}

#[test]
fn batch_elements_duplicate_carriers_and_query_form_precedence_remain_observable() {
    assert_eq!(check_category("transport"), 19);
}

#[test]
fn unresolved_operation_values_remain_deferred_to_core_macro_findings() {
    let engine = Engine::default();
    for (slug, value) in [
        (
            "amplitude-identify",
            serde_json::json!({"user_id":"synthetic-user","user_properties":{"$add":{"":"[VALUE]"},"$unset":{"old":"[VALUE]"}}}),
        ),
        (
            "amplitude-group-identify",
            serde_json::json!({"group_type":"synthetic-type","group_value":"synthetic-group","group_properties":{"$add":{"\n":"[VALUE]"}}}),
        ),
    ] {
        let path = if slug == "amplitude-identify" {
            "identify"
        } else {
            "groupidentify"
        };
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("api_key", "synthetic-project-key")
            .append_pair("identification", &value.to_string())
            .finish();
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact: format!("https://api2.amplitude.com/{path}?{query}"),
                    artifact_kind: ArtifactKind::Url,
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                1792886400,
            )
            .unwrap();
        let vendor = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == format!("vendor/{slug}"))
            .unwrap();
        assert!(vendor.violations.is_empty(), "{vendor:?}");
    }
}

#[test]
fn exact_identify_hosts_preserve_existing_directory_fallback_without_broad_attribution() {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor-amplitude-identify");
    let source: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(directory.join("source-cases-depth2.json")).unwrap(),
    )
    .unwrap();
    let engine = Engine::default();
    let mut checked = 0;
    for case in source["cases"].as_array().unwrap() {
        if case["category"] != "selection" {
            continue;
        }
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact: fs::read_to_string(directory.join(case["fixture"].as_str().unwrap()))
                        .unwrap(),
                    artifact_kind: ArtifactKind::Url,
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                1792886400,
            )
            .unwrap();
        let plugins: Vec<_> = summary
            .reports
            .iter()
            .map(|report| &report.plugin_id)
            .collect();
        let codes: Vec<_> = summary
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .map(|finding| &finding.code)
            .collect();
        assert_eq!(
            serde_json::json!(plugins),
            case["expected_plugins"],
            "{}",
            case["id"]
        );
        assert_eq!(
            serde_json::json!(codes),
            case["expected_codes"],
            "{}",
            case["id"]
        );
        checked += 1;
    }
    assert_eq!(checked, 8);
}
