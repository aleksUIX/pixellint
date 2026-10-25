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
    network_calls_to_collectors: usize,
    cases: Vec<SourceCase>,
}

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    category: String,
    carrier: String,
    case_origin: String,
    source_url: String,
    source_requirement: String,
    expected_plugins: Vec<String>,
    expected_codes: Vec<String>,
    expected_errors: usize,
    expected_infos: usize,
}

fn check_categories(categories: &[&str]) -> usize {
    let engine = Engine::default();
    let mut checked = 0;
    let mut carriers = BTreeSet::new();
    for slug in ["mixpanel-engage", "mixpanel-groups"] {
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/vendor-{slug}"));
        let source: SourceCases = serde_json::from_str(
            &fs::read_to_string(directory.join("source-cases-depth3.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(source.id, format!("vendor/{slug}"));
        assert!(!source.engine_derived);
        assert_eq!(source.network_calls_to_collectors, 0);
        let mut ids = BTreeSet::new();
        for case in source.cases {
            assert!(ids.insert(case.id.clone()));
            if !categories.contains(&case.category.as_str()) {
                continue;
            }
            let label = format!("{slug}/{}", case.id);
            assert!(case.source_url.starts_with("https://docs.mixpanel.com/"));
            assert!(!case.source_requirement.is_empty(), "{label}");
            assert!(
                [
                    "independently_authored_contract_boundary",
                    "official_request_example"
                ]
                .contains(&case.case_origin.as_str()),
                "{label}"
            );
            carriers.insert(case.carrier);
            let summary = engine
                .validate_at(
                    &ValidationRequest {
                        artifact: fs::read_to_string(directory.join(case.fixture)).unwrap(),
                        artifact_kind: match case.kind.as_str() {
                            "url" => ArtifactKind::Url,
                            "json" => ArtifactKind::JsonPayload,
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
            let plugins: Vec<_> = summary
                .reports
                .iter()
                .map(|report| report.plugin_id.clone())
                .collect();
            assert_eq!(plugins, case.expected_plugins, "{label}");
            let findings: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .collect();
            let codes: Vec<_> = findings
                .iter()
                .map(|finding| finding.code.clone())
                .collect();
            assert_eq!(codes, case.expected_codes, "{label}");
            assert_eq!(
                findings
                    .iter()
                    .filter(|finding| finding.severity == Severity::Error)
                    .count(),
                case.expected_errors,
                "{label}"
            );
            assert_eq!(
                findings
                    .iter()
                    .filter(|finding| finding.severity == Severity::Info)
                    .count(),
                case.expected_infos,
                "{label}"
            );
            for finding in findings {
                if finding.code.starts_with("vendor.mixpanel-") {
                    assert_eq!(finding.source.level, RuleSourceLevel::OfficialVendor);
                    assert!(finding.field.is_some(), "{label}");
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 0);
    if categories.contains(&"keys") {
        assert_eq!(carriers.len(), 3);
    }
    checked
}

#[test]
fn published_examples_keep_native_json_and_empty_operation_maps() {
    assert_eq!(check_categories(&["official"]), 3);
}

#[test]
fn operation_member_types_include_empty_and_multiline_names_in_all_three_decoders() {
    assert_eq!(check_categories(&["keys", "types"]), 84);
}

#[test]
fn every_batch_element_is_checked_without_cross_record_satisfaction() {
    assert_eq!(check_categories(&["batch"]), 9);
}

#[test]
fn duplicate_json_paths_and_selected_carrier_pairs_keep_existing_observation_limits() {
    assert_eq!(check_categories(&["duplicates"]), 22);
}

#[test]
fn xhr_form_get_beacon_and_query_form_precedence_preserve_the_same_constraints() {
    assert_eq!(check_categories(&["transport"]), 22);
}

#[test]
fn exact_rest_and_default_javascript_hosts_exclude_ui_query_and_adjacent_paths() {
    assert_eq!(check_categories(&["hosts", "selection"]), 34);
}

#[test]
fn unknown_operations_empty_union_arrays_and_unproven_group_add_scope_stay_open() {
    assert_eq!(check_categories(&["open", "group-add-gap"]), 11);
}

#[test]
fn pinned_sdk_constructors_preserve_empty_and_multiline_names_without_network_calls() {
    use base64::Engine as _;
    let engine = Engine::default();
    let mut checked = 0;
    for slug in ["mixpanel-engage", "mixpanel-groups"] {
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/vendor-{slug}"));
        let proof: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(directory.join("source-producer-depth3.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(proof["engine_derived"], false);
        assert_eq!(proof["network_calls"], 0);
        assert_eq!(
            proof["git_commit"],
            "d6bdc7f80230a54f8ec470d344058d0a36a81aa2"
        );
        for case in proof["cases"].as_array().unwrap() {
            let mut row = if slug == "mixpanel-engage" {
                serde_json::json!({"$token":"synthetic-token","$distinct_id":"synthetic-user"})
            } else {
                serde_json::json!({"$token":"synthetic-token","$group_key":"synthetic-key","$group_id":"synthetic-group"})
            };
            for (key, value) in case["operation"].as_object().unwrap() {
                row[key] = value.clone();
            }
            let raw = serde_json::json!([row]).to_string();
            let path = if slug == "mixpanel-engage" {
                "engage"
            } else {
                "groups"
            };
            for carrier in ["native", "json-data", "base64-data"] {
                let data = if carrier == "base64-data" {
                    base64::engine::general_purpose::STANDARD.encode(&raw)
                } else {
                    raw.clone()
                };
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("verbose", "1")
                    .append_pair("data", &data)
                    .finish();
                let summary = engine
                    .validate_at(
                        &ValidationRequest {
                            artifact: if carrier == "native" {
                                raw.clone()
                            } else {
                                format!("https://api-js.mixpanel.com/{path}/?{query}")
                            },
                            artifact_kind: if carrier == "native" {
                                ArtifactKind::JsonPayload
                            } else {
                                ArtifactKind::Url
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
                    .find(|report| report.plugin_id == format!("vendor/{slug}"))
                    .unwrap();
                assert!(
                    report.violations.is_empty(),
                    "{} {carrier}: {report:?}",
                    case["id"]
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 9);
}
