//! Primary-source transport examples and native contract corrections.

use std::{fs, path::PathBuf};

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde::Deserialize;

#[derive(Deserialize)]
struct SessionCase {
    id: String,
    kind: String,
    artifact: String,
    code: String,
    expect_violation: bool,
    expect_no_errors: bool,
    source_url: String,
    #[serde(default, alias = "reference_time")]
    reference_time_unix_seconds: Option<i64>,
    #[serde(default)]
    expected_code_count: Option<usize>,
}

#[test]
fn analytics_complete_requests_and_source_corrections() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    let mut checked = 0;
    let mut failures = Vec::new();
    for dir in fs::read_dir(fixture_root).unwrap() {
        let dir = dir.unwrap().path();
        let path = dir.join("session-depth-cases.json");
        if !path.exists() {
            continue;
        }
        let name = dir.file_name().unwrap().to_str().unwrap();
        let name = name.strip_prefix("vendor-").unwrap();
        let cases: Vec<SessionCase> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        for case in cases {
            assert!(case.source_url.starts_with("https://"));
            let request = ValidationRequest {
                artifact_kind: match case.kind.as_str() {
                    "request" => ArtifactKind::NetworkRequest,
                    "url" => ArtifactKind::Url,
                    "json" => ArtifactKind::JsonPayload,
                    other => panic!("unsupported source fixture kind {other}"),
                },
                artifact: case.artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            };
            let options = ValidationOptions {
                only_rulepacks: vec![format!("vendor/{name}")],
                except_rulepacks: Vec::new(),
            };
            let result = match case.reference_time_unix_seconds {
                Some(clock) => engine.validate_at(&request, &options, clock),
                None => engine.validate(&request, &options),
            };
            match result {
                Err(error) => failures.push(format!("{name}/{}: {error}", case.id)),
                Ok(summary) => {
                    let violations: Vec<_> =
                        summary.reports.iter().flat_map(|r| &r.violations).collect();
                    let count = violations.iter().filter(|v| v.code == case.code).count();
                    let found = count > 0;
                    if found != case.expect_violation
                        || (case.expect_no_errors && !summary.is_ok())
                        || case
                            .expected_code_count
                            .is_some_and(|expected| expected != count)
                    {
                        // Print only finding identifiers, never captured credentials.
                        failures.push(format!(
                            "{name}/{}: {} expected {}, clean {}, observed {:?}",
                            case.id,
                            case.code,
                            case.expect_violation,
                            case.expect_no_errors,
                            violations
                                .iter()
                                .map(|v| (&v.code, v.severity))
                                .collect::<Vec<_>>()
                        ));
                    }
                }
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 100,
        "source corpus unexpectedly empty: {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} source cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn analytics_complete_request_examples_select_the_destination_from_the_url() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    let mut checked = 0;
    let mut failures = Vec::new();
    for dir in fs::read_dir(fixture_root).unwrap() {
        let dir = dir.unwrap().path();
        let path = dir.join("session-depth-cases.json");
        if !path.exists() {
            continue;
        }
        let name = dir.file_name().unwrap().to_str().unwrap();
        let name = name.strip_prefix("vendor-").unwrap();
        let cases: Vec<SessionCase> =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        for case in cases
            .into_iter()
            .filter(|case| case.kind == "request" && case.expect_no_errors)
        {
            let request = ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: case.artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            };
            match engine.validate(&request, &ValidationOptions::default()) {
                Ok(summary)
                    if summary.is_ok()
                        && summary
                            .reports
                            .iter()
                            .any(|report| report.plugin_id == format!("vendor/{name}")) => {}
                Ok(summary) => failures.push(format!(
                    "{name}/{}: observed plugins {:?}, finding codes {:?}",
                    case.id,
                    summary
                        .reports
                        .iter()
                        .map(|report| &report.plugin_id)
                        .collect::<Vec<_>>(),
                    summary
                        .reports
                        .iter()
                        .flat_map(|report| &report.violations)
                        .map(|violation| &violation.code)
                        .collect::<Vec<_>>()
                )),
                Err(error) => failures.push(format!("{name}/{}: {error}", case.id)),
            }
            checked += 1;
        }
    }
    assert!(checked >= 40, "too few valid transport branches: {checked}");
    assert!(
        failures.is_empty(),
        "{} automatic routing cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
