//! Exact producer templates establish advisory completeness, not server acceptance.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, ValidatorPlugin,
};

const MANIFEST: &str = include_str!("../rulepacks/vendor/adxspace-vast-tracking.json");
const PACK: &str = "vendor/adxspace-vast-tracking";
const DOC: &str = "https://wiki.00px.com.br/en/technologies/vast4";

fn request(url: &str, kind: ArtifactKind, state: ExpansionState) -> ValidationRequest {
    ValidationRequest {
        artifact: url.to_owned(),
        artifact_kind: kind,
        claimed_vendor: None,
        expansion_state: state,
    }
}

#[test]
fn generated_tracking_impression_and_error_templates_select_in_supported_input_forms() {
    let engine = Engine::default();
    // Independent producer URL and hash live in the review, not private telemetry.
    for url in [
        "https://00px.net/tracking/synthetic-tag/starts?s=synthetic-opaque&t=synthetic-cache&v1=synthetic-state",
        "https://00px.net/vast/pixel/synthetic-tag/e.gif?s=synthetic-opaque&t=synthetic-cache&f=synthetic-format&v1=synthetic-state",
        "https://00px.net/tracking/synthetic-tag/errors?error=901&s=synthetic-opaque",
    ] {
        let capture = serde_json::json!({"url":url,"method":"GET","headers":{}}).to_string();
        for state in [
            ExpansionState::Unknown,
            ExpansionState::Template,
            ExpansionState::Fired,
        ] {
            for (artifact, kind) in [
                (url, ArtifactKind::Url),
                (url, ArtifactKind::VastTracker),
                (capture.as_str(), ArtifactKind::NetworkRequest),
            ] {
                let summary = engine
                    .validate_at(
                        &request(artifact, kind, state),
                        &ValidationOptions::default(),
                        1792886400,
                    )
                    .unwrap();
                let report = summary
                    .reports
                    .iter()
                    .find(|r| r.plugin_id == PACK)
                    .unwrap();
                assert!(report.violations.is_empty(), "{url} {kind:?} {state:?}");
                assert!(
                    !summary
                        .reports
                        .iter()
                        .any(|r| r.plugin_id == "vendor/adxspace-pixel")
                );
                assert!(!summary.reports.iter().any(|r| r.plugin_id == "directory"));
                assert!(summary.is_ok());
            }
        }
    }
}

#[test]
fn missing_fields_and_empty_captures_warn_without_query_repair() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for (url, code) in [
        ("https://00px.net/tracking/x/starts", "param.s.missing"),
        ("https://00px.net/vast/pixel/x/e.gif?s=", "param.s.empty"),
        (
            "https://00px.net/tracking//starts?s=x",
            "param.tag_token.empty",
        ),
        (
            "https://00px.net/vast/pixel//e.gif?s=x",
            "param.tag_token.empty",
        ),
        ("https://00px.net/tracking/x/?s=x", "param.event_path.empty"),
        (
            "https://00px.net/tracking//starts?s=x&tag_token=spoof",
            "param.tag_token.empty",
        ),
        (
            "https://00px.net/tracking/x/?s=x&event_path=spoof",
            "param.event_path.empty",
        ),
    ] {
        let input = request(url, ArtifactKind::Url, ExpansionState::Unknown);
        assert!(pack.supports(&input));
        let report = pack.validate(&input);
        assert_eq!(report.violations.len(), 1, "{url}");
        let finding = &report.violations[0];
        assert_eq!(
            finding.code,
            format!("vendor.adxspace-vast-tracking.{code}")
        );
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
        assert_eq!(finding.source.reference.as_deref(), Some(DOC));
    }
}

#[test]
fn opaque_values_open_events_and_uncontracted_query_fields_remain_unconstrained() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for url in [
        "https://00px.net/tracking/x/future-event?s=not-a-signature",
        "https://00px.net/tracking/not-base64!/open-event?s=anything!&t=&v1=&f=&error=",
        "https://00px.net/tracking/123/%E2%98%83?s=%E2%98%83",
        "https://00px.net/tracking/x/errors?s=x",
    ] {
        assert!(
            pack.validate(&request(url, ArtifactKind::Url, ExpansionState::Unknown))
                .violations
                .is_empty(),
            "{url}"
        );
    }
}

#[test]
fn counting_pixels_assets_sibling_routes_and_lookalikes_are_not_selected() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for url in [
        "https://00px.net/pixel/x/e.gif?s=x",
        "https://00px.net/vast/4/x/vast.xml?s=x",
        "https://00px.net/click/x?s=x",
        "https://00px.net/tracking/x?s=x",
        "https://00px.net/tracking/x/starts/extra?s=x",
        "https://00px.net/vast/pixel/x/other.gif?s=x",
        "https://00px.net/vast/pixel/x/e.gif/extra?s=x",
        "https://cdn.00px.net/tracking/x/starts?s=x",
        "https://00px.net.example.test/tracking/x/starts?s=x",
        "https://other.example/tracking/x/starts?s=x",
    ] {
        assert!(
            !pack.supports(&request(url, ArtifactKind::Url, ExpansionState::Unknown)),
            "{url}"
        );
    }
}

#[test]
fn new_template_selection_keeps_core_privacy_errors_active() {
    let summary = Engine::default()
        .validate(
            &request(
                "https://00px.net/tracking/x/starts?s=x&gdpr=maybe",
                ArtifactKind::VastTracker,
                ExpansionState::Template,
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(summary.reports.iter().any(|r| r.plugin_id == PACK));
    assert!(summary.reports.iter().any(|r| {
        r.plugin_id == "core"
            && r.violations
                .iter()
                .any(|v| v.code == "core.privacy.gdpr_invalid" && v.severity == Severity::Error)
    }));
    assert!(!summary.is_ok());
}
