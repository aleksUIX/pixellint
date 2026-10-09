//! Independent primary-source oracles for endpoint families discovered in D1.
//! Public artifacts are synthetic. D1 frequency supplies routing priorities,
//! never parameter requiredness or a replacement oracle.

use std::{collections::HashSet, fs, path::PathBuf};

use pixellint_core::{ArtifactKind, Engine, Severity, ValidationOptions, ValidationRequest};
use serde::Deserialize;

const PACKS: &[&str] = &[
    "ias-video-pixel",
    "ias-display-pixel",
    "appsflyer-impression",
    "google-ima-telemetry",
    "google-ima-interaction",
    "google-ima-pcs",
    "google-activeview",
    "flashtalking-impression",
    "flashtalking-state",
    "liveramp-ctvid",
    "nielsen-dar-pixel",
    "innovid-legacy-impression",
    "innovid-legacy-state",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    reference_time: i64,
    expected_plugins: Vec<String>,
    expected_ok: bool,
    expected_errors: usize,
    expected_warnings: usize,
    expected_infos: usize,
    expected_codes: Vec<String>,
    source_urls: Vec<String>,
    source_access: String,
    source_requirement: String,
    case_origin: String,
    #[serde(default)]
    source_artifacts: Vec<SourceArtifact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceArtifact {
    url: String,
    sha256: String,
    member: Option<String>,
    member_sha256: Option<String>,
}

#[test]
fn existing_vendor_endpoint_source_contracts_and_routing_hold() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut engine = Engine::default();
    for pack in PACKS {
        engine
            .register_manifest_path(repository.join(format!(
                "crates/pixellint-core/rulepacks/vendor/{pack}.json"
            )))
            .unwrap();
    }
    let mut checked = 0;
    let mut failures = Vec::new();
    for pack in PACKS {
        let directory = repository.join(format!("fixtures/vendor-{pack}"));
        let cases: Vec<SourceCase> = serde_json::from_str(
            &fs::read_to_string(directory.join("d1-source-cases.json")).unwrap(),
        )
        .unwrap();
        let mut ids = HashSet::new();
        assert!(cases.len() >= 3, "{pack} has source boundaries");
        for case in cases {
            let label = format!("{pack}/{}", case.id);
            assert!(ids.insert(case.id.clone()), "duplicate {label}");
            assert!(!case.source_urls.is_empty(), "{label} source");
            assert!(
                case.source_urls
                    .iter()
                    .all(|url| url.starts_with("https://"))
            );
            assert_eq!(
                case.case_origin,
                "independent_primary_requirement_regression"
            );
            assert!(!case.source_access.trim().is_empty());
            assert!(!case.source_requirement.trim().is_empty());
            for artifact in &case.source_artifacts {
                assert!(artifact.url.starts_with("https://"), "{label} producer URL");
                let is_digest = |value: &str| {
                    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                };
                assert!(is_digest(&artifact.sha256), "{label} producer digest");
                if let Some(member) = &artifact.member {
                    assert!(!member.trim().is_empty());
                    assert!(artifact.member_sha256.as_deref().is_some_and(is_digest));
                }
            }
            assert!(!case.fixture.contains('/') && !case.fixture.contains('\\'));
            let request = ValidationRequest {
                artifact_kind: match case.kind.as_str() {
                    "url" => ArtifactKind::Url,
                    "request" => ArtifactKind::NetworkRequest,
                    other => panic!("{label}: unsupported {other}"),
                },
                artifact: fs::read_to_string(directory.join(case.fixture)).unwrap(),
                claimed_vendor: None,
                expansion_state: Default::default(),
            };
            let result = engine
                .validate_at(&request, &ValidationOptions::default(), case.reference_time)
                .unwrap();
            let plugins: Vec<_> = result
                .reports
                .iter()
                .map(|report| report.plugin_id.clone())
                .collect();
            let violations: Vec<_> = result
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .collect();
            let codes: Vec<_> = violations
                .iter()
                .map(|violation| violation.code.clone())
                .collect();
            let count = |severity| {
                violations
                    .iter()
                    .filter(|violation| violation.severity == severity)
                    .count()
            };
            if plugins != case.expected_plugins
                || codes != case.expected_codes
                || result.is_ok() != case.expected_ok
                || count(Severity::Error) != case.expected_errors
                || count(Severity::Warning) != case.expected_warnings
                || count(Severity::Info) != case.expected_infos
            {
                failures.push(format!(
                    "{label}: plugins {plugins:?}, codes {codes:?}, ok {}",
                    result.is_ok()
                ));
            }
            checked += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} source cases checked:\n{}",
        checked,
        failures.join("\n")
    );
}
