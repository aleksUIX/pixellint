//! Source targets have their own oracle and clock, independent of golden counts.
//! Native type mutations and primary-schema mutations remain labeled separately
//! from vendor examples and manually reviewed requirement regressions.

use std::{collections::BTreeMap, collections::HashSet, fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;

const PACKS: &[&str] = &[
    "adjust",
    "appsflyer-onelink-impression",
    "appsflyer",
    "awin-basket",
    "awin-mastertag",
    "awin",
    "branch",
    "cj",
    "criteo-retail-media",
    "criteo",
    "impact-conversions",
    "impact",
    "kochava",
    "linkedin-conversions-api",
    "linkedin",
    "meta-conversions-api",
    "meta",
    "microsoft-clarity",
    "microsoft-conversions-api",
    "microsoft-uet",
    "nextdoor-conversions-api",
    "openai-conversions-api",
    "openai",
    "partnerize",
    "pinterest-conversions-api",
    "pinterest",
    "quora-conversions-api",
    "rakuten",
    "reddit-conversions-api",
    "reddit",
    "singular",
    "snapchat",
    "tiktok-events-2",
    "tiktok-events-api",
    "tiktok",
    "x-conversions-api",
    "x",
    "yahoo-conversions-api",
    "yahoo-dot",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceTarget {
    id: String,
    kind: String,
    fixture: String,
    reference_time: i64,
    rulepacks: Vec<String>,
    except_rulepacks: Vec<String>,
    claimed_vendor: Option<String>,
    expansion_state: ExpansionState,
    required_code: Option<String>,
    forbidden_code: Option<String>,
    expect_no_errors_or_warnings: bool,
    #[serde(default)]
    expect_ok: Option<bool>,
    case_origin: String,
    source_url: String,
    source_access: String,
    source_requirement: String,
    supporting_source_urls: Vec<String>,
    provenance_basis: String,
    encoding_source: Option<String>,
}

#[test]
fn conversion_primary_source_targets_hold_at_declared_clocks() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    let mut checked = 0;
    let mut populated_packs = 0;
    let mut origins = BTreeMap::new();
    let mut failures = Vec::new();

    for name in PACKS {
        let dir = root.join(format!("vendor-{name}"));
        let path = dir.join("source-targets.json");
        let cases: Vec<SourceTarget> = serde_json::from_str(
            &fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        populated_packs += usize::from(!cases.is_empty());
        let mut ids = HashSet::new();

        for case in cases {
            let label = format!("{name}/{}", case.id);
            assert!(
                ids.insert(case.id.clone()),
                "duplicate source target {label}"
            );
            assert!(case.source_url.starts_with("https://"), "{label} source");
            assert!(
                !case.source_access.trim().is_empty(),
                "{label} source access"
            );
            assert!(
                !case.source_requirement.trim().is_empty(),
                "{label} source requirement"
            );
            assert!(
                [
                    "original_case_record",
                    "reviewed_primary_contract_inventory"
                ]
                .contains(&case.provenance_basis.as_str()),
                "{label} provenance basis"
            );
            for source in case
                .supporting_source_urls
                .iter()
                .chain(case.encoding_source.iter())
            {
                assert!(source.starts_with("https://"), "{label} supporting source");
            }
            assert!(
                [
                    "vendor_source_example_or_requirement",
                    "reviewed_requirement_regression",
                    "contract_derived_mutation",
                    "primary_schema_derived_mutation",
                ]
                .contains(&case.case_origin.as_str()),
                "{label} case origin"
            );
            *origins.entry(case.case_origin).or_insert(0usize) += 1;
            assert!(
                case.required_code.is_some()
                    || case.forbidden_code.is_some()
                    || case.expect_no_errors_or_warnings,
                "{label} must retain an independent assertion"
            );
            assert!(
                !case.fixture.contains('/') && !case.fixture.contains('\\'),
                "{label} fixture stays inside its pack directory"
            );
            let request = ValidationRequest {
                artifact_kind: match case.kind.as_str() {
                    "url" => ArtifactKind::Url,
                    "json" => ArtifactKind::JsonPayload,
                    "request" => ArtifactKind::NetworkRequest,
                    other => panic!("{label}: unsupported artifact kind {other}"),
                },
                artifact: fs::read_to_string(dir.join(&case.fixture))
                    .unwrap_or_else(|error| panic!("{label}: {error}")),
                claimed_vendor: case.claimed_vendor,
                expansion_state: case.expansion_state,
            };
            let options = ValidationOptions {
                only_rulepacks: case.rulepacks,
                except_rulepacks: case.except_rulepacks,
            };
            let summary = engine
                .validate_at(&request, &options, case.reference_time)
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
            let codes: Vec<_> = findings.iter().map(|v| v.code.as_str()).collect();
            if let Some(code) = case.required_code
                && !codes.contains(&code.as_str())
            {
                failures.push(format!(
                    "{label}: missing required {code}; observed {codes:?}"
                ));
            }
            if let Some(code) = case.forbidden_code
                && codes.contains(&code.as_str())
            {
                failures.push(format!("{label}: forbidden {code}; observed {codes:?}"));
            }
            if let Some(expected) = case.expect_ok
                && summary.is_ok() != expected
            {
                failures.push(format!(
                    "{label}: expected ok={expected}; observed {codes:?}"
                ));
            }
            if case.expect_no_errors_or_warnings
                && findings
                    .iter()
                    .any(|v| matches!(v.severity, Severity::Error | Severity::Warning))
            {
                failures.push(format!("{label}: clean source case reported {codes:?}"));
            }
            checked += 1;
        }
    }

    assert!(
        checked >= 1740,
        "source targets must not silently disappear"
    );
    assert!(
        populated_packs >= 24,
        "covered pack corpora must not disappear"
    );
    assert_eq!(origins.len(), 4, "keep case origins distinct");
    assert!(
        failures.is_empty(),
        "{} source-target failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
