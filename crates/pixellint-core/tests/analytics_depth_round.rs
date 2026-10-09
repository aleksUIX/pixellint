//! Independent source cases for Cloudflare's producer and Matomo's server parser.
use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, Severity, ValidationOptions,
    ValidationRequest,
};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    artifact: String,
    code: String,
    expect_violation: bool,
    #[serde(default)]
    expected_severity: Option<Severity>,
    expect_no_errors: bool,
    #[serde(default)]
    expect_no_findings: bool,
    source_url: String,
    source_statement: String,
    #[serde(default = "yes")]
    pack_selected: bool,
    #[serde(default)]
    expected_code_count: Option<usize>,
    #[serde(default)]
    reference_time: Option<i64>,
    #[serde(default)]
    expected_field: Option<String>,
    #[serde(default)]
    expect_no_targets: bool,
}
fn yes() -> bool {
    true
}

#[test]
fn collector_and_bulk_maps_match_primary_source_oracles() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    for name in ["cloudflare-rum", "matomo"] {
        engine
            .register_manifest_path(repo.join(format!(
                "crates/pixellint-core/rulepacks/vendor/{name}.json"
            )))
            .unwrap();
    }
    let mut failures = Vec::new();
    let mut checked = 0;
    for (name, filename) in [
        ("cloudflare-rum", "round-depth-cases.json"),
        ("matomo", "round-depth-cases.json"),
        ("cloudflare-rum", "sdk-shape-cases.json"),
    ] {
        let path = repo.join(format!("fixtures/vendor-{name}/{filename}"));
        if !path.exists() {
            continue;
        }
        let cases: Vec<SourceCase> =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        for case in cases {
            assert!(case.source_url.starts_with("https://"));
            assert!(!case.source_statement.is_empty());
            let request = ValidationRequest {
                artifact_kind: match case.kind.as_str() {
                    "url" => ArtifactKind::Url,
                    "request" => ArtifactKind::NetworkRequest,
                    _ => panic!("unsupported source case kind"),
                },
                artifact: case.artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            };
            let options = ValidationOptions::default();
            let summary = match case.reference_time {
                Some(t) => engine.validate_at(&request, &options, t),
                None => engine.validate(&request, &options),
            }
            .unwrap();
            let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
            let selected = summary
                .reports
                .iter()
                .any(|r| r.plugin_id == format!("vendor/{name}"));
            let matching: Vec<_> = findings.iter().filter(|v| v.code == case.code).collect();
            if selected != case.pack_selected
                || !matching.is_empty() != case.expect_violation
                || case
                    .expected_code_count
                    .is_some_and(|n| n != matching.len())
                || case
                    .expected_severity
                    .is_some_and(|s| matching.iter().any(|v| v.severity != s))
                || (case.expect_no_errors && !summary.is_ok())
                || (case.expect_no_findings && !findings.is_empty())
                || case
                    .expected_field
                    .as_deref()
                    .is_some_and(|field| matching.iter().any(|v| v.field.as_deref() != Some(field)))
                || (case.expect_no_targets && matching.iter().any(|v| !v.targets.is_empty()))
            {
                failures.push(format!("{name}/{}: selected {selected}, expected {}, target {} expected {}, findings {:?}", case.id,case.pack_selected,case.code,case.expect_violation,findings.iter().map(|v| (&v.code,v.severity)).collect::<Vec<_>>()));
            }
            checked += 1;
        }
    }
    assert!(checked >= 40);
    assert!(
        failures.is_empty(),
        "{} independent cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
