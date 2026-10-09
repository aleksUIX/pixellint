//! Independent primary-source examples for previously directory-only families.

use std::{fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, Severity, ValidationOptions,
    ValidationRequest,
};
use serde::Deserialize;

const PACKS: [&str; 4] = [
    "adcanvas-csc",
    "xpln-video",
    "triplelift-tracking",
    "triplelift-sync",
];

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    artifact: String,
    code: String,
    expect_violation: bool,
    source_url: String,
    source_statement: String,
    #[serde(default)]
    clean: bool,
    #[serde(default)]
    force: bool,
    #[serde(default)]
    expected_code_count: Option<usize>,
}

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rulepacks/vendor");
    for name in PACKS {
        engine
            .register_manifest_path(root.join(format!("{name}.json")))
            .unwrap();
    }
    engine
}

fn request(case: &SourceCase) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: match case.kind.as_str() {
            "url" => ArtifactKind::Url,
            "json" => ArtifactKind::JsonPayload,
            "request" => ArtifactKind::NetworkRequest,
            "vast" => ArtifactKind::VastTracker,
            kind => panic!("unrecognized fixture kind {kind}"),
        },
        artifact: case.artifact.clone(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn primary_sdk_and_documentation_requirements_have_independent_cases() {
    let engine = engine();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut failures = Vec::new();
    let mut count = 0;
    for name in PACKS {
        let cases: Vec<SourceCase> = serde_json::from_str(
            &fs::read_to_string(root.join(format!("vendor-{name}/d1-source-cases.json"))).unwrap(),
        )
        .unwrap();
        for case in &cases {
            assert!(case.source_url.starts_with("https://"));
            assert!(!case.source_statement.is_empty());
            let options = ValidationOptions {
                only_rulepacks: if case.force {
                    vec![format!("vendor/{name}")]
                } else {
                    Vec::new()
                },
                except_rulepacks: Vec::new(),
            };
            let summary = engine.validate(&request(case), &options).unwrap();
            let violations: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .collect();
            let found = violations.iter().any(|finding| finding.code == case.code);
            let has_pack = summary
                .reports
                .iter()
                .any(|report| report.plugin_id == format!("vendor/{name}"));
            let code_count = violations
                .iter()
                .filter(|finding| finding.code == case.code)
                .count();
            if case
                .expected_code_count
                .is_some_and(|expected| expected != code_count)
                || found != case.expect_violation
                || !has_pack
                || (case.clean && !violations.is_empty())
            {
                failures.push(format!(
                    "{name}/{}: code {} expected {}, selected {}, clean {}, observed {:?}",
                    case.id,
                    case.code,
                    case.expect_violation,
                    has_pack,
                    case.clean,
                    violations
                        .iter()
                        .map(|finding| (&finding.code, finding.severity))
                        .collect::<Vec<_>>()
                ));
            }
            // Producer examples stay advisory. No rejection contract is inferred.
            if name != "triplelift-sync" {
                assert!(
                    violations
                        .iter()
                        .filter(|finding| finding.code.starts_with("vendor."))
                        .all(|finding| finding.severity != Severity::Error)
                );
            }
            count += 1;
        }
    }
    assert!(
        count >= 200,
        "independent source corpus unexpectedly shrank: {count}"
    );
    assert!(
        failures.is_empty(),
        "{} source cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn source_scopes_leave_unpublished_directory_endpoints_unclaimed() {
    let engine = engine();
    for artifact in [
        "https://analytics.adcanvas.com/not-the-source-endpoint?p=example",
        "https://log.xpln.tech/event?xid=example",
        "https://eb2.3lift.com/private-tracker?aid=1001",
        "https://s.update.3lift.com/sync?xuid=example",
        "https://tlx.3lift.com/sync?xuid=example",
        "https://track.celtra.com/json/opaque?md5=opaque",
        "https://s.innovid.com/1x1.gif?project_hash=example",
    ] {
        let summary = engine
            .validate(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::Url,
                    artifact: artifact.to_string(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(
            summary
                .reports
                .iter()
                .all(|report| !report.plugin_id.starts_with("vendor/")),
            "source scope widened for {artifact}"
        );
    }
}
