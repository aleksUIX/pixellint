//! Independently authored vendor requirements, fixed clocks and boundaries.

use std::{fs, path::PathBuf};

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct SourceCase {
    id: String,
    kind: String,
    artifact: String,
    expected_codes: Vec<String>,
    severity: String,
    source_url: String,
    reference_time: i64,
    origin: String,
}

fn request(kind: ArtifactKind, artifact: String) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn primary_requirements_and_boundaries_have_exact_findings() {
    let engine = Engine::default();
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    for pack in [
        "hubspot-pixel",
        "liveramp-envelope",
        "liveramp-envelope-refresh",
        "outbrain",
    ] {
        let cases: Vec<SourceCase> = serde_json::from_str(
            &fs::read_to_string(fixture_root.join(format!("vendor-{pack}/source-cases-035.json")))
                .unwrap(),
        )
        .unwrap();
        for case in cases {
            assert!(case.source_url.starts_with("https://"));
            assert_eq!(case.origin, "independently_authored_primary_requirement");
            assert_eq!(case.reference_time, 1_800_000_000);
            let kind = match case.kind.as_str() {
                "url" => ArtifactKind::Url,
                "request" => ArtifactKind::NetworkRequest,
                other => panic!("unknown source case kind {other}"),
            };
            let summary = engine
                .validate_at(
                    &request(kind, case.artifact),
                    &ValidationOptions {
                        only_rulepacks: vec![format!("vendor/{pack}")],
                        except_rulepacks: vec![],
                    },
                    case.reference_time,
                )
                .unwrap();
            let findings: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .collect();
            let mut codes: Vec<_> = findings
                .iter()
                .map(|finding| finding.code.clone())
                .collect();
            codes.sort();
            let mut expected_codes = case.expected_codes;
            expected_codes.sort();
            assert_eq!(codes, expected_codes, "{pack}/{}: {summary:?}", case.id);
            for finding in findings {
                assert_eq!(
                    format!("{:?}", finding.severity).to_ascii_lowercase(),
                    case.severity,
                    "{pack}/{}",
                    case.id
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 217);
}

#[test]
fn all_values_in_proves_every_visible_mode_and_preserves_macro_unknowns() {
    for negated in [false, true] {
        let guard = json!({"kind":"all_values_in", "params":["query.mode", "query.mode[]"], "values":["3"]});
        let condition = if negated {
            json!({"kind":"not", "condition":guard})
        } else {
            guard
        };
        let mut engine = Engine::new();
        engine.register_manifest_json(&json!({
            "id":"custom/mode", "display_name":"Mode", "description":"Unambiguous observed mode.",
            "source_level":"heuristic", "match":{"hosts":["example.test"]},
            "http":{"params":[{"name":"query.mode","aliases":["query.type"],"allow_empty":true},
                {"name":"query.mode[]","aliases":["query.type[]"],"allow_empty":true},
                {"name":"headers.x-marker","requirement":"required","condition":condition}]}
        }).to_string()).unwrap();
        for (query, proven) in [
            ("", Some(false)),
            ("mode=3", Some(true)),
            ("mode=3&mode=3", Some(true)),
            ("mode=3&%6dode=3", Some(true)),
            ("type=3&type=3", Some(true)),
            ("mode=1&mode=3", Some(false)),
            ("mode=3&mode=1", Some(false)),
            ("mode=3&mode=", Some(false)),
            ("mode&mode=3", Some(false)),
            ("mode=3&mode=%5BTYPE%5D", None),
            ("mode=%5BTYPE%5D&mode=3", None),
            ("mode=%5BTYPE%5D", None),
            ("mode=1&mode=%5BTYPE%5D", Some(false)),
        ] {
            let summary = engine.validate_at(
                &request(ArtifactKind::NetworkRequest, json!({
                    "url":format!("https://example.test/?{query}"),"method":"GET","headers":{}
                }).to_string()),
                &ValidationOptions::default(), 1_800_000_000
            ).unwrap();
            let expected = proven.is_some_and(|value| value != negated);
            assert_eq!(
                summary.reports[0].violations.len(),
                usize::from(expected),
                "{negated}/{query}: {summary:?}"
            );
        }
    }
    for condition in [
        json!({"kind":"all_values_in","params":[],"values":["3"]}),
        json!({"kind":"all_values_in","params":["mode"],"values":[]}),
        json!({"kind":"all_values_in","params":["uncontracted"],"values":["3"]}),
    ] {
        assert!(Engine::new().register_manifest_json(&json!({
            "id":"custom/mode", "display_name":"Mode", "description":"Guard declarations.",
            "source_level":"heuristic", "match":{"hosts":["example.test"]},
            "params":[{"name":"mode"},{"name":"marker","condition":condition}]
        }).to_string()).is_err());
    }
}

#[test]
fn max_occurrences_respects_decoded_names_aliases_empty_values_and_source_spans() {
    let mut engine = Engine::new();
    engine.register_manifest_json(&json!({
        "id":"custom/cardinality", "display_name":"Cardinality", "description":"Submitted query occurrences.",
        "source_level":"heuristic", "match":{"hosts":["example.test"]},
        "params":[{"name":"tc","aliases":["target"],"allow_empty":true}],
        "rules":[{"code":"custom.cardinality.maximum", "kind":"max_occurrences", "param":"tc",
            "max_occurrences":5, "severity":"warning", "message":"Too many submitted targeted fields."}]
    }).to_string()).unwrap();
    let url = "https://example.test/collect?tc=a&%74c=b&target=c&tc=&target&%74c";
    let summary = engine
        .validate_at(
            &request(ArtifactKind::Url, url.into()),
            &ValidationOptions::default(),
            1_800_000_000,
        )
        .unwrap();
    let findings = &summary.reports[0].violations;
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].targets.len(), 6);
    for target in &findings[0].targets {
        let segment = &url[target.start..target.end];
        assert!(!segment.is_empty() && !segment.contains('&'));
    }
}

#[test]
fn ip_chain_macros_do_not_hide_invalid_literal_neighbors_for_scalar_and_assertion_formats() {
    for assertion in [false, true] {
        let mut manifest = json!({
            "id":"custom/chain", "display_name":"IP chain", "description":"Literal chain syntax.",
            "source_level":"heuristic", "match":{"hosts":["example.test"]},
            "params":[{"name":"ip"}]
        });
        if assertion {
            manifest["rules"] = json!([{"code":"custom.chain.invalid", "kind":"format", "param":"ip",
                "format":{"kind":"ip_chain"},"severity":"warning", "message":"Invalid chain."}]);
        } else {
            manifest["params"][0]["format"] = json!({"kind":"ip_chain"});
            manifest["params"][0]["format_severity"] = json!("warning");
        }
        let mut engine = Engine::new();
        engine
            .register_manifest_json(&manifest.to_string())
            .unwrap();
        for (chain, expected) in [
            ("%5BIPADDRESS%5D%2C203.0.113.1", 0),
            ("%5BIPADDRESS%5D%2C203.0.113.999", 1),
            ("203.0.113.999%2C%5BIPADDRESS%5D", 1),
            ("%5BIPADDRESS%5D%3A443", 1),
            ("203.0.113.1%0A", 1),
        ] {
            let summary = engine
                .validate_at(
                    &request(
                        ArtifactKind::Url,
                        format!("https://example.test/?ip={chain}"),
                    ),
                    &ValidationOptions::default(),
                    1_800_000_000,
                )
                .unwrap();
            assert_eq!(
                summary.reports[0].violations.len(),
                expected,
                "{assertion}/{chain}: {summary:?}"
            );
        }
    }
}

#[test]
fn datetime_representation_lists_are_nonempty_and_reject_unknown_modes() {
    for formats in [json!([]), json!(["guess_timezone"])] {
        let mut engine = Engine::new();
        assert!(engine.register_manifest_json(&json!({
            "id":"custom/dates", "display_name":"Dates", "description":"Selected source date formats.",
            "source_level":"heuristic", "match":{"hosts":["example.test"]},
            "params":[{"name":"time","format":{"kind":"datetime_formats","formats":formats}}]
        }).to_string()).is_err());
    }
}
