//! Route coverage must follow the producer template without borrowing another host's contract.

use pixellint_core::{
    ArtifactKind, Engine, EngineError, ExpansionState, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest,
};

const PACK: &str = "vendor/google-ima-interaction";
const SOURCE: &str =
    "https://developers.google.com/interactive-media-ads/docs/sdks/html5/client-side/tags";

fn request(artifact: &str, kind: ArtifactKind, state: ExpansionState) -> ValidationRequest {
    ValidationRequest {
        artifact: artifact.to_owned(),
        artifact_kind: kind,
        claimed_vendor: None,
        expansion_state: state,
    }
}

fn interaction_engine() -> Engine {
    let mut engine = Engine::new();
    engine
        .register_manifest_json(include_str!(
            "../rulepacks/vendor/google-ima-interaction.json"
        ))
        .unwrap();
    engine
}

#[test]
fn live_producer_callbacks_select_the_same_interaction_contract_in_each_input_form() {
    // Independent producer evidence and its hash are recorded in
    // docs/reviews/coverage-gaps-20261020-google-directory.json.
    let engine = Engine::default();
    for path in ["/pagead/live/interaction", "/pagead/live/interaction/"] {
        let url = format!(
            "https://pubads.g.doubleclick.net{path}?ai=opaque-ai&sigh=opaque-signature&label=videoplay"
        );
        let captured = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
        for state in [
            ExpansionState::Unknown,
            ExpansionState::Template,
            ExpansionState::Fired,
        ] {
            for (kind, artifact) in [
                (ArtifactKind::Url, url.as_str()),
                (ArtifactKind::VastTracker, url.as_str()),
                (ArtifactKind::NetworkRequest, captured.as_str()),
            ] {
                let result = engine
                    .validate(
                        &request(artifact, kind, state),
                        &ValidationOptions::default(),
                    )
                    .unwrap();
                let report = result.reports.iter().find(|r| r.plugin_id == PACK).unwrap();
                assert!(report.violations.is_empty(), "{path} {kind:?} {state:?}");
                assert!(result.is_ok());
                assert!(!result.reports.iter().any(|r| r.plugin_id == "directory"));
            }
        }
    }
}

#[test]
fn live_template_completeness_warns_without_inventing_signature_or_event_grammars() {
    let engine = interaction_engine();
    for (query, code) in [
        ("sigh=opaque&label=videoplay", "param.ai.missing"),
        ("ai=opaque&sigh=&label=videoplay", "param.sigh.empty"),
        ("ai=opaque&sigh=opaque", "param.label.missing"),
    ] {
        let url = format!("https://pubads.g.doubleclick.net/pagead/live/interaction/?{query}");
        let result = engine
            .validate(
                &request(&url, ArtifactKind::Url, ExpansionState::Unknown),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert_eq!(result.reports.len(), 1);
        let violations = &result.reports[0].violations;
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].code,
            format!("vendor.google-ima-interaction.{code}")
        );
        assert_eq!(violations[0].severity, Severity::Warning);
        assert_eq!(
            violations[0].source.level,
            RuleSourceLevel::OfficialTemplate
        );
        assert_eq!(violations[0].source.reference.as_deref(), Some(SOURCE));
    }
    let result = engine
        .validate(
            &request(
                "https://pubads.g.doubleclick.net/pagead/live/interaction/?ai=%E2%98%83&sigh=not-a-real-signature&label=future-event-label",
                ArtifactKind::Url,
                ExpansionState::Unknown,
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(result.reports[0].violations.is_empty());
}

#[test]
fn exact_live_route_does_not_claim_neighboring_paths_or_lookalike_hosts() {
    let engine = interaction_engine();
    for url in [
        "https://pubads.g.doubleclick.net/pagead/live/interaction-extra?ai=x&sigh=y&label=z",
        "https://pubads.g.doubleclick.net/pagead/live/interaction/extra?ai=x&sigh=y&label=z",
        "https://pubads.g.doubleclick.net/pagead/live/other?ai=x&sigh=y&label=z",
        "https://pubads.g.doubleclick.net/Pagead/live/interaction?ai=x&sigh=y&label=z",
        "https://pubads.g.doubleclick.net.evil.test/pagead/live/interaction?ai=x&sigh=y&label=z",
        "https://evil.test/pagead/live/interaction?ai=x&sigh=y&label=z",
        "https://securepubads.g.doubleclick.net/pagead/live/interaction?ai=x&sigh=y&label=z",
    ] {
        let result = engine.validate(
            &request(url, ArtifactKind::Url, ExpansionState::Unknown),
            &ValidationOptions::default(),
        );
        assert!(
            matches!(result, Err(EngineError::NoMatchingPlugin)),
            "{url}"
        );
    }
}

#[test]
fn pagead2_pcs_view_does_not_inherit_cm360_sai_advisories() {
    let engine = Engine::default();
    for query in ["xai=opaque-xai&sig=opaque-signature", "xai=&sig=", ""] {
        let url = format!("https://pagead2.googlesyndication.com/pcs/view?{query}");
        let result = engine
            .validate(
                &request(&url, ArtifactKind::Url, ExpansionState::Unknown),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(
            !result
                .reports
                .iter()
                .any(|r| r.plugin_id == "vendor/google-cm360-pcs-view")
        );
        assert!(
            !result
                .reports
                .iter()
                .any(|r| r.plugin_id == "vendor/google-ima-pcs")
        );
        assert!(result.reports.iter().any(|r| r.plugin_id == "directory"));
        assert!(
            !result
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code.ends_with("sai.missing"))
        );
    }
}

#[test]
fn vendor_selection_and_deferred_coverage_preserve_core_errors_and_broken_macros() {
    let engine = Engine::default();
    for url in [
        "https://pubads.g.doubleclick.net/pagead/live/interaction/?ai=opaque-ai&sigh=opaque-sigh&label=videoplay&gdpr=maybe&cb=!!CACHEBUSTER!",
        "https://pagead2.googlesyndication.com/pcs/view?xai=opaque-xai&sig=opaque-sig&gdpr=maybe&cb=!!CACHEBUSTER!",
    ] {
        let result = engine
            .validate(
                &request(url, ArtifactKind::VastTracker, ExpansionState::Template),
                &ValidationOptions::default(),
            )
            .unwrap();
        let core = result
            .reports
            .iter()
            .find(|r| r.plugin_id == "core")
            .unwrap();
        for (code, severity) in [
            ("core.privacy.gdpr_invalid", Severity::Error),
            ("core.macro.broken_delimiter", Severity::Warning),
        ] {
            assert!(
                core.violations
                    .iter()
                    .any(|v| v.code == code && v.severity == severity)
            );
        }
        assert!(!result.is_ok());
    }
}
