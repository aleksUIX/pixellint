//! Independently authored conversion requirements keep fixed clocks and exact
//! error/warning oracles. Producer model cases remain explicitly labeled.

use std::{collections::HashSet, fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct ExpectedFinding {
    code: String,
    severity: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    reference_time: i64,
    expected_findings: Vec<ExpectedFinding>,
    expected_ok: bool,
    source_url: String,
    supporting_source_urls: Vec<String>,
    source_requirement: String,
    case_origin: String,
}

#[test]
fn conversion_contracts_match_independent_fixed_clock_oracles() {
    let engine = Engine::default();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    let mut failures = Vec::new();
    for pack in [
        "meta-conversions-api",
        "pinterest-conversions-api",
        "tiktok-events-2",
    ] {
        let dir = root.join(format!("vendor-{pack}"));
        let cases: Vec<SourceCase> = serde_json::from_str(
            &fs::read_to_string(dir.join("round-034-source-cases.json")).unwrap(),
        )
        .unwrap();
        assert!(!cases.is_empty(), "{pack} must have source cases");
        let mut ids = HashSet::new();
        for mut case in cases {
            let label = format!("{pack}/{}", case.id);
            assert!(ids.insert(case.id.clone()), "duplicate {label}");
            assert!(case.source_url.starts_with("https://"), "{label}");
            assert!(!case.source_requirement.trim().is_empty(), "{label}");
            assert!(
                [
                    "independently_authored_requirement",
                    "primary_model_type_mutation"
                ]
                .contains(&case.case_origin.as_str()),
                "{label} provenance"
            );
            assert!(
                case.supporting_source_urls
                    .iter()
                    .all(|url| url.starts_with("https://")),
                "{label} supporting source"
            );
            assert!(!case.fixture.contains('/') && !case.fixture.contains('\\'));
            let request = ValidationRequest {
                artifact_kind: match case.kind.as_str() {
                    "json" => ArtifactKind::JsonPayload,
                    "request" => ArtifactKind::NetworkRequest,
                    "url" => ArtifactKind::Url,
                    other => panic!("{label}: unsupported {other}"),
                },
                artifact: fs::read_to_string(dir.join(&case.fixture)).unwrap(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            };
            let summary = engine
                .validate_at(
                    &request,
                    &ValidationOptions {
                        only_rulepacks: vec![format!("vendor/{pack}")],
                        except_rulepacks: vec![],
                    },
                    case.reference_time,
                )
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            let mut observed: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .filter(|finding| finding.severity != Severity::Info)
                .map(|finding| ExpectedFinding {
                    code: finding.code.clone(),
                    severity: match finding.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Info => unreachable!(),
                    }
                    .into(),
                })
                .collect();
            observed.sort();
            case.expected_findings.sort();
            if observed != case.expected_findings || summary.is_ok() != case.expected_ok {
                failures.push(format!(
                    "{label}: expected ok={} {:?}; observed ok={} {:?}",
                    case.expected_ok,
                    case.expected_findings,
                    summary.is_ok(),
                    observed
                ));
            }
            checked += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("Checked {checked} conversion source cases against fixed clocks.");
}

#[test]
fn conversion_pack_goldens_remain_exact() {
    let engine = Engine::default();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut failures = Vec::new();
    let mut checked = 0;
    for pack in [
        "meta-conversions-api",
        "pinterest-conversions-api",
        "tiktok-events-2",
    ] {
        let dir = root.join(format!("vendor-{pack}"));
        let cases: Vec<Value> =
            serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
        for case in cases {
            let label = format!("{pack}/{}", case["id"].as_str().unwrap());
            let request = ValidationRequest {
                artifact_kind: match case["kind"].as_str().unwrap() {
                    "url" => ArtifactKind::Url,
                    "json" => ArtifactKind::JsonPayload,
                    "request" => ArtifactKind::NetworkRequest,
                    other => panic!("{label}: {other}"),
                },
                artifact: fs::read_to_string(dir.join(case["fixture"].as_str().unwrap())).unwrap(),
                claimed_vendor: case
                    .get("claimed_vendor")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                expansion_state: case
                    .get("expansion_state")
                    .map_or(ExpansionState::Unknown, |value| {
                        serde_json::from_value(value.clone()).unwrap()
                    }),
            };
            let options = ValidationOptions {
                only_rulepacks: case.get("rulepacks").map_or_else(Vec::new, |value| {
                    serde_json::from_value(value.clone()).unwrap()
                }),
                except_rulepacks: case.get("except_rulepacks").map_or_else(Vec::new, |value| {
                    serde_json::from_value(value.clone()).unwrap()
                }),
            };
            let summary = case
                .get("reference_time")
                .and_then(Value::as_i64)
                .map_or_else(
                    || engine.validate(&request, &options),
                    |time| engine.validate_at(&request, &options, time),
                )
                .unwrap();
            let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
            let codes: Vec<_> = findings.iter().map(|f| f.code.as_str()).collect();
            let expected: Vec<String> =
                serde_json::from_value(case["expected_codes"].clone()).unwrap();
            let plugins: Vec<_> = summary
                .reports
                .iter()
                .map(|r| r.plugin_id.as_str())
                .collect();
            let wrong_plugins = case.get("expected_plugins").is_some_and(|value| {
                let expected: Vec<String> = serde_json::from_value(value.clone()).unwrap();
                plugins != expected
            });
            let wrong_counts = [
                ("expected_errors", Severity::Error),
                ("expected_warnings", Severity::Warning),
                ("expected_infos", Severity::Info),
            ]
            .iter()
            .any(|(field, severity)| {
                findings.iter().filter(|f| f.severity == *severity).count()
                    != case[*field].as_u64().unwrap() as usize
            });
            if codes != expected
                || summary.is_ok() != case["expected_ok"].as_bool().unwrap()
                || wrong_plugins
                || wrong_counts
            {
                failures.push(format!("{label}: expected {expected:?}, observed {codes:?}, plugins {plugins:?}, ok {}", summary.is_ok()));
            }
            checked += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} golden mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("Checked {checked} owned conversion goldens.");
}
