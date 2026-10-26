use std::{fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct SourceCases {
    id: String,
    engine_derived: bool,
    collector_calls: usize,
    cases: Vec<SourceCase>,
}

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    category: String,
    source_url: String,
    source_binding_url: String,
    source_requirement: String,
    expected_plugins: Vec<String>,
    expected_codes: Vec<String>,
    expected_warnings: usize,
    expected_infos: usize,
    #[serde(default)]
    expansion_state: Option<String>,
}

fn check_categories(categories: &[&str]) -> usize {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/vendor-gamesight-impression");
    let source: SourceCases = serde_json::from_str(
        &fs::read_to_string(directory.join("source-cases-depth4.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(source.id, "vendor/gamesight-impression");
    assert!(!source.engine_derived);
    assert_eq!(source.collector_calls, 0);
    let engine = Engine::default();
    let mut checked = 0;
    for case in source.cases {
        if !categories.contains(&case.category.as_str()) {
            continue;
        }
        assert!(case.source_url.starts_with("https://docs.gamesight.io/"));
        assert_eq!(
            case.source_binding_url,
            "https://docs.gamesight.io/docs/tracker-url-parameters"
        );
        assert!(!case.source_requirement.is_empty());
        let input = ValidationRequest {
            artifact: fs::read_to_string(directory.join(case.fixture)).unwrap(),
            artifact_kind: match case.kind.as_str() {
                "url" => ArtifactKind::Url,
                "vast" => ArtifactKind::VastTracker,
                "postback" => ArtifactKind::ServerPostback,
                "request" => ArtifactKind::NetworkRequest,
                other => panic!("unsupported kind {other}"),
            },
            claimed_vendor: None,
            expansion_state: if case.expansion_state.as_deref() == Some("template") {
                ExpansionState::Template
            } else {
                ExpansionState::Unknown
            },
        };
        let summary = engine
            .validate_at(&input, &ValidationOptions::default(), 1792886400)
            .unwrap();
        let plugins: Vec<_> = summary
            .reports
            .iter()
            .map(|r| r.plugin_id.clone())
            .collect();
        assert_eq!(plugins, case.expected_plugins, "{}", case.id);
        let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        let codes: Vec<_> = findings.iter().map(|v| v.code.clone()).collect();
        assert_eq!(codes, case.expected_codes, "{}", case.id);
        assert_eq!(
            findings
                .iter()
                .filter(|v| v.severity == Severity::Warning)
                .count(),
            case.expected_warnings,
            "{}",
            case.id
        );
        assert_eq!(
            findings
                .iter()
                .filter(|v| v.severity == Severity::Info)
                .count(),
            case.expected_infos,
            "{}",
            case.id
        );
        assert!(findings.iter().all(|v| v.severity != Severity::Error));
        for finding in findings {
            if finding
                .code
                .starts_with("vendor.gamesight-impression.indexed_query")
            {
                assert_eq!(finding.source.level, RuleSourceLevel::OfficialVendor);
                assert!(
                    finding
                        .source
                        .reference
                        .as_ref()
                        .unwrap()
                        .contains("measurement-api-console-identifiers")
                );
                assert!(finding.field.as_ref().unwrap().contains("xid["));
            }
        }
        checked += 1;
    }
    checked
}

#[test]
fn primary_url_examples_and_scoped_owner_advice_preserve_opaque_identifiers() {
    assert_eq!(check_categories(&["official", "dependency"]), 18);
}

#[test]
fn duplicate_macro_and_unindexed_ambiguity_do_not_infer_an_org_row() {
    assert_eq!(check_categories(&["duplicates", "macros", "unindexed"]), 13);
}

#[test]
fn lexical_indexes_once_only_decoding_and_interleaving_remain_explicit() {
    assert_eq!(check_categories(&["decoding", "indexes", "spans"]), 16);
}

#[test]
fn url_like_transports_keep_existing_attribution_and_ignore_form_fallback() {
    assert_eq!(check_categories(&["transport", "selection"]), 12);
}
