//! Missing URL presence can be owned by transport alternatives on matching
//! complete captures. Literal URL/form values retain their original contracts.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, Severity, ValidationOptions,
    ValidationRequest, ValidationSummary,
};
use serde_json::{Value, json};

fn manifest() -> Value {
    json!({
        "id": "custom/http-alternatives",
        "display_name": "Transport alternatives",
        "description": "Independent engine contract for URL presence deferral.",
        "source_level": "heuristic",
        "match": {"hosts": ["endpoint.example"], "paths": ["/events"]},
        "http_url_presence_overrides": ["token", "notice"],
        "params": [
            {"name": "token", "aliases": ["alias_token"], "requirement": "required",
             "format": {"kind": "regex", "pattern": "^VALID$"}},
            {"name": "notice", "requirement": "recommended"},
            {"name": "legacy", "requirement": "forbidden"},
            {"name": "old", "requirement": "deprecated"},
            {"name": "branch"}
        ],
        "http": {"params": [
            {"name": "query.token", "json_type": "string", "allow_empty": true},
            {"name": "body.token", "json_type": "string",
             "format": {"kind": "regex", "pattern": "^VALID$"}},
            {"name": "headers.authorization", "json_type": "string",
             "format": {"kind": "regex", "pattern": "^Bearer VALID$"}},
            {"name": "body.notice"}
        ], "rules": [{
            "code": "custom.http-alternatives.http.auth",
            "kind": "require_any_of",
            "groups": [["query.token"], ["body.token"], ["headers.authorization"]],
            "severity": "error", "message": "A populated authentication carrier is required."
        }]}
    })
}

fn test_engine(manifest: &Value) -> Engine {
    let mut engine = Engine::new();
    engine.register(ManifestRulePack::from_json(&manifest.to_string()).unwrap());
    engine
}

fn request(kind: ArtifactKind, artifact: String) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact,
        claimed_vendor: None,
        expansion_state: ExpansionState::Fired,
    }
}

fn capture(url: &str, header: Option<&str>, body: Option<(&str, &str)>) -> Value {
    let mut value = json!({"url": url, "method": "POST", "headers": {}});
    if let Some(header) = header {
        value["headers"]["Authorization"] = json!(header);
    }
    if let Some((content_type, body)) = body {
        value["headers"]["Content-Type"] = json!(content_type);
        value["body"] = json!(body);
    }
    value
}

fn validate(engine: &Engine, request: ValidationRequest, explicit: bool) -> ValidationSummary {
    engine
        .validate_at(
            &request,
            &ValidationOptions {
                only_rulepacks: if explicit {
                    vec!["custom/http-alternatives".into()]
                } else {
                    vec![]
                },
                except_rulepacks: vec![],
            },
            1_770_000_060,
        )
        .unwrap()
}

fn findings(summary: &ValidationSummary) -> Vec<(&str, Severity)> {
    summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|finding| (finding.code.as_str(), finding.severity))
        .collect()
}

#[test]
fn http_presence_override_loader_accepts_canonical_presence_contracts() {
    for name in ["token", "notice"] {
        let mut value = manifest();
        value["http_url_presence_overrides"] = json!([name]);
        assert!(ManifestRulePack::from_json(&value.to_string()).is_ok());
    }
    let mut suffix = manifest();
    suffix["match"] = json!({"host_suffixes": ["endpoint.example"], "paths": ["/events"]});
    assert!(ManifestRulePack::from_json(&suffix.to_string()).is_ok());
    let mut none = manifest();
    none.as_object_mut()
        .unwrap()
        .remove("http_url_presence_overrides");
    none.as_object_mut().unwrap().remove("http");
    assert!(ManifestRulePack::from_json(&none.to_string()).is_ok());
}

#[test]
fn http_presence_override_loader_rejects_unsafe_or_ineffective_names() {
    for names in [
        json!(["token", "token"]),
        json!(["unknown"]),
        json!(["alias_token"]),
        json!([""]),
        json!(["branch"]),
        json!(["legacy"]),
        json!(["old"]),
    ] {
        let mut value = manifest();
        value["http_url_presence_overrides"] = names.clone();
        let error = ManifestRulePack::from_json(&value.to_string())
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("http_url_presence_overrides"),
            "{names}: {error}"
        );
    }
    for http in [None, Some(json!([]))] {
        let mut value = manifest();
        if let Some(http) = http {
            value["http"] = http;
        } else {
            value.as_object_mut().unwrap().remove("http");
        }
        let error = ManifestRulePack::from_json(&value.to_string())
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("http_url_presence_overrides")
                || error.to_string().contains("contracts no body"),
            "{error}"
        );
    }
    let mut unbound = manifest();
    unbound["match"] = json!({"any_host": true, "paths": ["/events"]});
    let error = ManifestRulePack::from_json(&unbound.to_string())
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("http_url_presence_overrides"),
        "{error}"
    );
}

#[test]
fn complete_capture_alternatives_keep_missing_and_literal_failures_distinct() {
    let engine = test_engine(&manifest());
    for value in [
        capture(
            "https://endpoint.example/events",
            Some("Bearer VALID"),
            None,
        ),
        capture("https://endpoint.example/events?token=VALID", None, None),
        capture(
            "https://endpoint.example/events",
            None,
            Some(("application/json", r#"{"token":"VALID"}"#)),
        ),
        capture(
            "https://endpoint.example/events",
            None,
            Some(("application/x-www-form-urlencoded", "token=VALID")),
        ),
        capture(
            "https://endpoint.example/events?alias_token=VALID",
            Some("Bearer VALID"),
            None,
        ),
    ] {
        let summary = validate(
            &engine,
            request(ArtifactKind::NetworkRequest, value.to_string()),
            false,
        );
        assert!(findings(&summary).is_empty(), "{value}: {summary:?}");
    }
    for (value, expected) in [
        (
            capture("https://endpoint.example/events", None, None),
            "custom.http-alternatives.http.auth",
        ),
        (
            capture(
                "https://endpoint.example/events?token=",
                Some("Bearer VALID"),
                None,
            ),
            "custom.http-alternatives.param.token.empty",
        ),
        (
            capture(
                "https://endpoint.example/events?token=bad",
                Some("Bearer VALID"),
                None,
            ),
            "custom.http-alternatives.param.token.invalid",
        ),
        (
            capture(
                "https://endpoint.example/events?alias_token=bad",
                Some("Bearer VALID"),
                None,
            ),
            "custom.http-alternatives.param.token.invalid",
        ),
        (
            capture("https://endpoint.example/events", Some("Basic VALID"), None),
            "custom.http-alternatives.http.headers.authorization.invalid",
        ),
        (
            capture(
                "https://endpoint.example/events?legacy=x",
                Some("Bearer VALID"),
                None,
            ),
            "custom.http-alternatives.param.legacy.forbidden",
        ),
    ] {
        let summary = validate(
            &engine,
            request(ArtifactKind::NetworkRequest, value.to_string()),
            false,
        );
        assert_eq!(findings(&summary), [(expected, Severity::Error)], "{value}");
    }
    let value = capture(
        "https://endpoint.example/events?old=x",
        Some("Bearer VALID"),
        None,
    );
    let summary = validate(
        &engine,
        request(ArtifactKind::NetworkRequest, value.to_string()),
        false,
    );
    assert_eq!(
        findings(&summary),
        [(
            "custom.http-alternatives.param.old.deprecated",
            Severity::Warning
        )]
    );
    let value = capture(
        "https://endpoint.example/events",
        Some("Bearer VALID"),
        Some(("application/x-www-form-urlencoded", "token=bad")),
    );
    let summary = validate(
        &engine,
        request(ArtifactKind::NetworkRequest, value.to_string()),
        false,
    );
    assert_eq!(
        findings(&summary),
        [
            (
                "custom.http-alternatives.param.token.invalid",
                Severity::Error
            ),
            (
                "custom.http-alternatives.http.body.token.invalid",
                Severity::Error
            ),
        ]
    );
}

#[test]
fn bare_urls_and_unmatched_capture_branches_keep_original_presence() {
    let engine = test_engine(&manifest());
    for kind in [ArtifactKind::Url, ArtifactKind::NetworkRequest] {
        let summary = validate(
            &engine,
            request(kind, "https://endpoint.example/events".into()),
            false,
        );
        assert_eq!(
            findings(&summary),
            [
                (
                    "custom.http-alternatives.param.token.missing",
                    Severity::Error
                ),
                (
                    "custom.http-alternatives.param.notice.missing",
                    Severity::Warning
                ),
            ]
        );
    }
    for url in [
        "https://other.example/events",
        "https://endpoint.example/unrelated",
    ] {
        let value = capture(url, Some("Bearer VALID"), None);
        let summary = validate(
            &engine,
            request(ArtifactKind::NetworkRequest, value.to_string()),
            true,
        );
        assert_eq!(
            findings(&summary),
            [("custom.http-alternatives.endpoint_mismatch", Severity::Info)]
        );
        assert_eq!(summary.reports[0].detected_vendor, None);
    }
    let mut branch = manifest();
    branch["match"]["query_params_any"] = json!(["branch"]);
    let engine = test_engine(&branch);
    let value = capture(
        "https://endpoint.example/events",
        Some("Bearer VALID"),
        None,
    );
    let summary = validate(
        &engine,
        request(ArtifactKind::NetworkRequest, value.to_string()),
        true,
    );
    assert_eq!(
        findings(&summary),
        [
            ("custom.http-alternatives.endpoint_mismatch", Severity::Info),
            (
                "custom.http-alternatives.param.token.missing",
                Severity::Error
            ),
            (
                "custom.http-alternatives.param.notice.missing",
                Severity::Warning
            ),
        ]
    );
}
