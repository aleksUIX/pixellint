//! Independently authored conversion requirements keep fixed clocks and exact
//! error/warning oracles. Producer model cases remain explicitly labeled.

use std::{collections::HashSet, fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;

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
fn amplitude_multipart_matches_independent_source_oracles() {
    let mut engine = Engine::new();
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/amplitude-identify.json"))
        .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    let mut failures = Vec::new();
    for pack in ["amplitude-identify"] {
        let dir = root.join(format!("vendor-{pack}"));
        let cases: Vec<SourceCase> = serde_json::from_str(
            &fs::read_to_string(dir.join("round-034-multipart-source-cases.json")).unwrap(),
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
fn unavailable_multipart_captures_report_the_coverage_limit() {
    let mut engine = Engine::new();
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/amplitude-identify.json"))
        .unwrap();
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor-amplitude-identify");
    for case in [
        "file-unavailable",
        "charset-unavailable",
        "compressed-unavailable",
        "ambiguous-name-unavailable",
        "ambiguous-boundary-unavailable",
        "boundary-prefix-content",
        "default-ascii-unavailable",
        "repeated-default-charset-unavailable",
        "repeated-content-type-unavailable",
    ] {
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::NetworkRequest,
                    artifact: fs::read_to_string(
                        root.join(format!("depth-034-multipart-{case}.request.json")),
                    )
                    .unwrap(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        assert!(
            findings
                .iter()
                .any(|f| f.code == "core.request.unsupported_body_encoding"
                    && f.severity == Severity::Info),
            "{case}: {summary:?}"
        );
        assert!(
            !findings.iter().any(|f| f.code.ends_with(".missing")),
            "{case}: cannot invent unobserved values"
        );
    }
}

#[test]
fn query_and_multipart_credentials_never_enter_findings_or_offsets() {
    let mut engine = Engine::new();
    engine
        .register_manifest_json(
            &serde_json::json!({
                "id":"vendor/privacy", "display_name":"Credential capture fixture",
                "description":"Credential redaction boundary", "docs":"https://example.org/schema",
                "match":{"hosts":["privacy.example"]},
                "params":[{"name":"api_key","format":{"kind":"enum","values":["allowed"]}},
                          {"name":"access_token","format":{"kind":"enum","values":["allowed"]}}]
            })
            .to_string(),
        )
        .unwrap();
    let secret = "synthetic-CREDENTIAL+%abc";
    for multipart in [true, false] {
        let url = if multipart {
            "https://privacy.example/collect".to_string()
        } else {
            format!(
                "https://privacy.example/collect?access_token={}",
                url::form_urlencoded::byte_serialize(secret.as_bytes()).collect::<String>()
            )
        };
        let body = format!(
            "--test\r\nContent-Disposition: form-data; name=\"api_key\"\r\n\r\n{secret}\r\n--test--\r\n"
        );
        let summary = engine.validate_at(&ValidationRequest {
            artifact_kind: ArtifactKind::NetworkRequest,
            artifact: serde_json::json!({"url":url,"method":"POST", "headers":{"Content-Type":"multipart/form-data; boundary=test"},"body":body}).to_string(),
            claimed_vendor: None, expansion_state: ExpansionState::Unknown,
        }, &ValidationOptions::default(), 1_800_000_000).unwrap();
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(!serialized.contains(secret));
        assert!(serialized.contains("[redacted]"));
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .all(|f| f.targets.is_empty())
        );
    }
}

#[test]
fn unavailable_mime_defers_aliases_and_dependent_rules_but_keeps_observable_checks() {
    let mut engine = Engine::new();
    engine.register_manifest_json(&serde_json::json!({
        "id":"custom/mime", "display_name":"Observed MIME fixture", "description":"Root aliases preserve unknown capture values", "docs":"https://example.org/mime-schema",
        "match":{"hosts":["mime.example"]},
        "http":{"params":[
            {"name":"mime", "root_path":"content_type", "requirement":"required", "format":{"kind":"enum", "values":["multipart/form-data"]}},
            {"name":"method", "format":{"kind":"enum", "values":["POST"]}}
        ], "rules":[{"code":"custom.mime.required", "kind":"require_one_of", "params":["mime"], "severity":"error", "message":"The MIME essence must be observable."}]}
    }).to_string()).unwrap();
    let request = |headers, method| {
        ValidationRequest {
        artifact_kind: ArtifactKind::NetworkRequest,
        artifact: serde_json::json!({"url":"https://mime.example/collect", "method":method, "headers":headers, "body":"unavailable"}).to_string(),
        claimed_vendor: None, expansion_state: ExpansionState::Unknown,
    }
    };
    let headers = serde_json::json!([{"name":"Content-Type","value":"multipart/form-data; boundary=a"},{"name":"Content-Type","value":"text/plain"}]);
    let summary = engine
        .validate_at(
            &request(headers.clone(), "POST"),
            &ValidationOptions::default(),
            1_800_000_000,
        )
        .unwrap();
    let codes: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|r| &r.violations)
        .map(|f| f.code.as_str())
        .collect();
    assert_eq!(codes, ["core.request.unsupported_body_encoding"]);
    let summary = engine
        .validate_at(
            &request(headers, "DELETE"),
            &ValidationOptions::default(),
            1_800_000_000,
        )
        .unwrap();
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|f| f.code == "custom.mime.http.method.invalid")
    );
    for headers in [
        serde_json::json!({}),
        serde_json::json!({"Content-Type":"text/plain"}),
    ] {
        let summary = engine
            .validate_at(
                &request(headers, "POST"),
                &ValidationOptions::default(),
                1_800_000_000,
            )
            .unwrap();
        assert!(
            !summary.is_ok(),
            "Known missing or wrong MIME must retain its error."
        );
    }
}
