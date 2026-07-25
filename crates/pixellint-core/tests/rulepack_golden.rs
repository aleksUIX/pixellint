//! Golden corpus for every shipped rulepack.
//!
//! Each directory under `fixtures/` holds artifacts plus a `manifest.json` that
//! states exactly which rulepacks should run and which findings they should
//! emit. Adding a rule without adding a fixture leaves the rule unproven; the
//! `every_builtin_rulepack_has_a_fixture_directory` test catches the pack-level
//! version of that mistake.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use pixellint_core::{
    ArtifactKind, BUILTIN_VENDOR_MANIFESTS, Engine, ExpansionState, ManifestRulePack, Severity,
    ValidationOptions, ValidationRequest, ValidationSummary, ValidatorPlugin,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureCase {
    id: String,
    kind: String,
    fixture: String,
    #[serde(default)]
    expansion_state: Option<String>,
    #[serde(default)]
    claimed_vendor: Option<String>,
    /// Rulepacks to force on for this case, as `--rulepack` does.
    #[serde(default)]
    rulepacks: Vec<String>,
    /// Rulepacks to skip for this case, as `--except` does.
    #[serde(default)]
    except_rulepacks: Vec<String>,
    /// Rulepacks expected to produce a report, in engine order. Omit to accept
    /// whatever the engine selects.
    #[serde(default)]
    expected_plugins: Option<Vec<String>>,
    expected_ok: bool,
    expected_errors: usize,
    expected_warnings: usize,
    expected_infos: usize,
    expected_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViolationCounts {
    errors: usize,
    warnings: usize,
    infos: usize,
}

#[test]
fn golden_corpus_matches_expected_findings() {
    let engine = Engine::default();
    let mut checked = 0;

    for directory in fixture_directories() {
        let manifest_path = directory.join("manifest.json");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", manifest_path.display()));
        let cases: Vec<FixtureCase> = serde_json::from_str(&manifest)
            .unwrap_or_else(|error| panic!("parse {}: {error}", manifest_path.display()));

        assert!(
            !cases.is_empty(),
            "fixture directory {} has no cases",
            directory.display()
        );

        for case in cases {
            let label = format!("{}/{}", directory_name(&directory), case.id);
            let artifact_path = directory.join(&case.fixture);
            let artifact = fs::read_to_string(&artifact_path).unwrap_or_else(|error| {
                panic!("read fixture {}: {error}", artifact_path.display())
            });

            let request = ValidationRequest {
                artifact_kind: parse_artifact_kind(&case.kind),
                artifact,
                claimed_vendor: case.claimed_vendor.clone(),
                expansion_state: parse_expansion_state(case.expansion_state.as_deref()),
            };

            let options = ValidationOptions {
                only_rulepacks: case.rulepacks.clone(),
                except_rulepacks: case.except_rulepacks.clone(),
            };

            let summary = engine
                .validate(&request, &options)
                .unwrap_or_else(|error| panic!("validate fixture {label}: {error}"));

            if let Some(expected_plugins) = &case.expected_plugins {
                let actual: Vec<&str> = summary
                    .reports
                    .iter()
                    .map(|report| report.plugin_id.as_str())
                    .collect();
                assert_eq!(&actual, expected_plugins, "fixture {label} rulepack set");
            }

            let counts = violation_counts(&summary);
            assert_eq!(summary.is_ok(), case.expected_ok, "fixture {label} ok");
            assert_eq!(
                counts.errors, case.expected_errors,
                "fixture {label} errors"
            );
            assert_eq!(
                counts.warnings, case.expected_warnings,
                "fixture {label} warnings"
            );
            assert_eq!(counts.infos, case.expected_infos, "fixture {label} infos");
            assert_eq!(
                violation_codes(&summary),
                case.expected_codes,
                "fixture {label} codes"
            );

            checked += 1;
        }
    }

    assert!(checked > 0, "no fixtures were checked");
}

#[test]
fn every_builtin_rulepack_compiles_with_a_matching_id() {
    for (id, json) in BUILTIN_VENDOR_MANIFESTS {
        let pack = ManifestRulePack::from_json(json)
            .unwrap_or_else(|error| panic!("built-in rulepack `{id}` failed to compile: {error}"));
        assert_eq!(
            &pack.metadata().id,
            id,
            "built-in rulepack id does not match its registration entry"
        );
        assert!(
            pack.vendor().is_some(),
            "built-in rulepack `{id}` should name its vendor"
        );
        assert!(
            pack.docs().is_some(),
            "built-in rulepack `{id}` should cite documentation"
        );
    }
}

#[test]
fn every_builtin_rulepack_has_a_fixture_directory() {
    let directories: BTreeSet<String> = fixture_directories()
        .iter()
        .map(|directory| directory_name(directory))
        .collect();

    for (id, _) in BUILTIN_VENDOR_MANIFESTS {
        let expected = id.replace('/', "-");
        assert!(
            directories.contains(&expected),
            "rulepack `{id}` has no fixtures at fixtures/{expected}"
        );
    }
}

#[test]
fn default_engine_registers_core_and_every_builtin_pack() {
    let ids: Vec<String> = Engine::default()
        .list_rulepacks()
        .into_iter()
        .map(|metadata| metadata.id)
        .collect();

    assert!(ids.contains(&"core".to_string()));
    for (id, _) in BUILTIN_VENDOR_MANIFESTS {
        assert!(ids.contains(&(*id).to_string()), "`{id}` is not registered");
    }
    assert_eq!(ids.len(), BUILTIN_VENDOR_MANIFESTS.len() + 1);
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn fixture_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = fs::read_dir(fixture_root())
        .expect("read fixtures directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("manifest.json").is_file())
        .collect();

    directories.sort();
    directories
}

fn directory_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .expect("fixture directory name")
        .to_string()
}

fn parse_expansion_state(value: Option<&str>) -> ExpansionState {
    match value.unwrap_or("unknown") {
        "unknown" => ExpansionState::Unknown,
        "template" => ExpansionState::Template,
        "fired" => ExpansionState::Fired,
        other => panic!("unknown expansion state in fixture manifest: {other}"),
    }
}

fn parse_artifact_kind(value: &str) -> ArtifactKind {
    match value {
        "url" => ArtifactKind::Url,
        "html" => ArtifactKind::HtmlSnippet,
        "js" => ArtifactKind::JavaScriptSnippet,
        "gtm" => ArtifactKind::GtmTemplate,
        "request" => ArtifactKind::NetworkRequest,
        "vast" => ArtifactKind::VastTracker,
        "postback" => ArtifactKind::ServerPostback,
        "unknown" => ArtifactKind::Unknown,
        other => panic!("unknown artifact kind in fixture manifest: {other}"),
    }
}

fn violation_codes(summary: &ValidationSummary) -> Vec<String> {
    summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
        .map(|violation| violation.code.clone())
        .collect()
}

fn violation_counts(summary: &ValidationSummary) -> ViolationCounts {
    let mut counts = ViolationCounts {
        errors: 0,
        warnings: 0,
        infos: 0,
    };

    for violation in summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
    {
        match violation.severity {
            Severity::Error => counts.errors += 1,
            Severity::Warning => counts.warnings += 1,
            Severity::Info => counts.infos += 1,
        }
    }

    counts
}
