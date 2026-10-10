use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, ValidationOptions, ValidationRequest,
    ValidatorPlugin,
};
use serde_json::{Value, json};

fn manifest(matcher: Value) -> Value {
    json!({
        "id": "custom/path-selector-test",
        "display_name": "Path selector test",
        "description": "Tests endpoint selection independently of parameter extraction.",
        "source_level": "heuristic",
        "match": matcher,
        "params": []
    })
}

fn pack(matcher: Value) -> ManifestRulePack {
    ManifestRulePack::from_json(&manifest(matcher).to_string()).unwrap()
}

fn request(artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::VastTracker,
        artifact: artifact.into(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Template,
    }
}

#[test]
fn expressions_select_the_complete_path_with_host_boundaries() {
    let pack = pack(json!({
        "hosts": ["px.example.test"],
        "path_patterns": ["/pixel/[^/]*/e\\.gif"]
    }));
    for artifact in [
        "https://px.example.test/pixel/opaque-token/e.gif",
        "https://PX.EXAMPLE.TEST/pixel//e.gif?t=1",
        "https://px.example.test/pixel/[TOKEN]/e.gif",
    ] {
        assert!(pack.supports(&request(artifact)), "{artifact}");
    }
    for artifact in [
        "https://px.example.test/prefix/pixel/token/e.gif",
        "https://px.example.test/pixel/token/e.gif/suffix",
        "https://px.example.test/pixel/token/other.gif",
        "https://px.example.test/pixel/token/nested/e.gif",
        "https://px.example.test/Pixel/token/e.gif",
        "https://px.example.test/tracking/token/starts",
        "https://px.example.test/?next=/pixel/token/e.gif",
        "https://other.example.test/pixel/token/e.gif",
        "https://px.example.test.attacker.test/pixel/token/e.gif",
    ] {
        assert!(!pack.supports(&request(artifact)), "{artifact}");
    }
}

#[test]
fn expression_alternatives_and_existing_selectors_preserve_union_semantics() {
    let pack = pack(json!({
        "hosts": ["px.example.test"],
        "paths": ["/fixed"],
        "path_prefixes": ["/prefix/"],
        "path_contains": ["/legacy/"],
        "path_patterns": ["/one|/one/longer", "/dynamic/[0-9]+"]
    }));
    for path in [
        "/fixed",
        "/prefix/child",
        "/old/legacy/endpoint",
        "/one",
        "/one/longer",
        "/dynamic/12",
    ] {
        assert!(
            pack.supports(&request(&format!("https://px.example.test{path}"))),
            "{path}"
        );
    }
    for path in [
        "/unknown",
        "/one/longer/child",
        "/other/one",
        "/dynamic/text",
    ] {
        assert!(
            !pack.supports(&request(&format!("https://px.example.test{path}"))),
            "{path}"
        );
    }
}

#[test]
fn invalid_expressions_and_any_host_expressions_are_rejected() {
    for pattern in ["", "[", "pixel)|.*(?:escape"] {
        let source = manifest(json!({
            "hosts": ["px.example.test"], "path_patterns": [pattern]
        }));
        assert!(
            ManifestRulePack::from_json(&source.to_string()).is_err(),
            "{pattern}"
        );
    }
    for matcher in [
        json!({"any_host":true,"path_patterns":["/pixel/[^/]+/e\\.gif"]}),
        json!({"any_host":true,"paths":["/fixed"],"path_patterns":["/pixel/.*"]}),
        json!({"path_patterns":["/pixel/.*"]}),
    ] {
        assert!(ManifestRulePack::from_json(&manifest(matcher).to_string()).is_err());
    }
}

#[test]
fn expression_selection_keeps_path_capture_and_query_contracts_separate() {
    let mut source = manifest(json!({
        "hosts": ["px.example.test"],
        "path_patterns": ["/pixel/[^/]*/e\\.gif"],
        "query_params_any": ["enabled"]
    }));
    source["path_pattern"] = json!("^/pixel/(?<token>[^/]*)/e\\.gif$");
    source["params"] =
        json!([{"name":"token","requirement":"required","format":{"kind":"non_empty"}}]);
    let mut engine = Engine::default();
    engine.register_manifest_json(&source.to_string()).unwrap();
    for (artifact, selected, expected_code) in [
        (
            "https://px.example.test/pixel/value/e.gif?enabled=",
            true,
            None,
        ),
        (
            "https://px.example.test/pixel//e.gif?enabled=1",
            true,
            Some("custom.path-selector-test.param.token.empty"),
        ),
        ("https://px.example.test/pixel/value/e.gif", false, None),
        (
            "https://px.example.test/tracking/value/starts?enabled=1",
            false,
            None,
        ),
        (
            "https://px.example.test/pixel/again/e.gif?enabled=1",
            true,
            None,
        ),
    ] {
        let summary = engine
            .validate(&request(artifact), &ValidationOptions::default())
            .unwrap();
        let report = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "custom/path-selector-test");
        assert_eq!(report.is_some(), selected, "{artifact}");
        if let Some(report) = report {
            let codes: Vec<_> = report.violations.iter().map(|v| v.code.as_str()).collect();
            assert_eq!(
                codes,
                expected_code.into_iter().collect::<Vec<_>>(),
                "{artifact}"
            );
        }
    }
}

#[test]
fn host_suffix_routing_preserves_boundaries_for_expression_paths() {
    let pack = pack(json!({
        "host_suffixes": ["example.test"],
        "path_patterns": ["/pixel/[^/]+/e\\.gif"]
    }));
    for url in [
        "https://example.test/pixel/x/e.gif",
        "https://a.example.test/pixel/x/e.gif",
    ] {
        assert!(pack.supports(&request(url)));
    }
    for url in [
        "https://notexample.test/pixel/x/e.gif",
        "https://example.test.invalid/pixel/x/e.gif",
        "https://a.example.test/pixel/x/e.gif/more",
    ] {
        assert!(!pack.supports(&request(url)));
    }
}

#[test]
fn captured_http_requests_use_expression_scope_and_query_gates() {
    let mut engine = Engine::default();
    engine
        .register_manifest_json(
            &manifest(json!({
                "hosts": ["px.example.test"],
                "path_patterns": ["/pixel/[^/]+/e\\.gif"],
                "query_params_any": ["enabled"]
            }))
            .to_string(),
        )
        .unwrap();
    for (url, selected) in [
        ("https://px.example.test/pixel/x/e.gif?enabled=", true),
        ("https://px.example.test/pixel/x/e.gif", false),
        (
            "https://px.example.test/pixel/x/e.gif/extra?enabled=1",
            false,
        ),
        ("https://other.example.test/pixel/x/e.gif?enabled=1", false),
    ] {
        let mut submitted = request("");
        submitted.artifact_kind = ArtifactKind::NetworkRequest;
        submitted.artifact = json!({"url":url,"method":"POST","headers":{},"body":""}).to_string();
        let summary = engine
            .validate(&submitted, &ValidationOptions::default())
            .unwrap();
        assert_eq!(
            summary
                .reports
                .iter()
                .any(|r| r.plugin_id == "custom/path-selector-test"),
            selected,
            "{url}"
        );
    }
}
