//! Published producer templates support completeness advisories only.
use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, ManifestRulePack,
    RuleSourceLevel, Severity, ValidationOptions, ValidationRequest, ValidatorPlugin,
};

const PACK: &str = "vendor/clipflip-vast-tracking";
const DOC: &str = "https://www.clipflip.video/vast-tester/";
const MANIFEST: &str = include_str!("../rulepacks/vendor/clipflip-vast-tracking.json");

fn input(artifact: &str, artifact_kind: ArtifactKind) -> ValidationRequest {
    ValidationRequest {
        artifact: artifact.into(),
        artifact_kind,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn source_impression_event_click_and_error_templates_select_in_all_url_input_forms() {
    let engine = Engine::default();
    for url in [
        "https://app.clipflip.video/api/vast/tracking/synthetic-creative/impression?v=3_0&ord=synthetic-cache",
        "https://app.clipflip.video/api/vast/tracking/synthetic-creative/firstQuartile?v=3_0",
        "https://app.clipflip.video/api/vast/tracking/synthetic-creative/click?v=3_0",
        "https://app.clipflip.video/api/vast/tracking/synthetic-creative/error?v=3_0",
    ] {
        let capture = serde_json::json!({"url":url,"method":"GET","headers":{}}).to_string();
        for (value, kind) in [
            (url, ArtifactKind::Url),
            (url, ArtifactKind::VastTracker),
            (capture.as_str(), ArtifactKind::NetworkRequest),
        ] {
            let summary = engine
                .validate(&input(value, kind), &ValidationOptions::default())
                .unwrap();
            assert!(summary.is_ok());
            assert_eq!(
                summary.coverage().status,
                EndpointCoverageStatus::VendorChecked
            );
            assert_eq!(summary.coverage().vendor_rulepacks, [PACK]);
            assert!(
                summary
                    .reports
                    .iter()
                    .find(|report| report.plugin_id == PACK)
                    .unwrap()
                    .violations
                    .is_empty()
            );
        }
    }
}

#[test]
fn malformed_template_components_warn_and_query_parameters_cannot_repair_paths() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for (url, suffix) in [
        (
            "https://app.clipflip.video/api/vast/tracking/x/start",
            "v.missing",
        ),
        (
            "https://app.clipflip.video/api/vast/tracking/x/start?v=",
            "v.empty",
        ),
        (
            "https://app.clipflip.video/api/vast/tracking//start?v=x",
            "creative_path.empty",
        ),
        (
            "https://app.clipflip.video/api/vast/tracking/x/?v=x",
            "event_path.empty",
        ),
        (
            "https://app.clipflip.video/api/vast/tracking//start?v=x&creative_path=spoof",
            "creative_path.empty",
        ),
        (
            "https://app.clipflip.video/api/vast/tracking/x/?v=x&event_path=spoof",
            "event_path.empty",
        ),
    ] {
        let request = input(url, ArtifactKind::Url);
        assert!(pack.supports(&request), "{url}");
        let report = pack.validate(&request);
        assert_eq!(report.violations.len(), 1, "{url}");
        let finding = &report.violations[0];
        assert_eq!(
            finding.code,
            format!("vendor.clipflip-vast-tracking.param.{suffix}")
        );
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
        assert_eq!(finding.source.reference.as_deref(), Some(DOC));
    }
}

#[test]
fn opaque_ids_open_events_versions_and_optional_ord_remain_unconstrained() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for url in [
        "https://app.clipflip.video/api/vast/tracking/opaque!/future-event?v=future-version",
        "https://app.clipflip.video/api/vast/tracking/x/impression?v=3_0",
        "https://app.clipflip.video/api/vast/tracking/x/impression?v=3_0&ord=",
        "https://app.clipflip.video/api/vast/tracking/x/impression?v=3_0&ord=-1",
        "https://app.clipflip.video/api/vast/tracking/x/error?v=x",
    ] {
        assert!(
            pack.validate(&input(url, ArtifactKind::Url))
                .violations
                .is_empty(),
            "{url}"
        );
    }
}

#[test]
fn producer_routes_extra_segments_other_hosts_and_lookalikes_are_not_selected() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for url in [
        "https://app.clipflip.video/api/vast/x?v=3_0",
        "https://app.clipflip.video/api/vast/tracking/x?v=x",
        "https://app.clipflip.video/api/vast/tracking/x/start/extra?v=x",
        "https://app.clipflip.video/api/VAST/tracking/x/start?v=x",
        "https://app.clipflip.video/api/events/x?v=x",
        "https://cdn.clipflip.video/api/vast/tracking/x/start?v=x",
        "https://child.app.clipflip.video/api/vast/tracking/x/start?v=x",
        "https://staging-app.clipflip.video/api/vast/tracking/x/start?v=x",
        "https://app.clipflip.video.evil.test/api/vast/tracking/x/start?v=x",
    ] {
        assert!(!pack.supports(&input(url, ArtifactKind::Url)), "{url}");
    }
    let engine = Engine::default();
    assert_eq!(
        engine
            .directory()
            .lookup_host("app.clipflip.video")
            .unwrap()
            .rulepack
            .as_deref(),
        Some(PACK)
    );
    let summary = engine
        .validate(
            &input(
                "https://app.clipflip.video/api/vast/x?v=3_0",
                ArtifactKind::Url,
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(
        summary.coverage().status,
        EndpointCoverageStatus::RecognizedUncontracted
    );
    assert!(summary.coverage().vendor_rulepacks.is_empty());
}

#[test]
fn callback_selection_preserves_core_privacy_errors() {
    let summary = Engine::default()
        .validate(
            &input(
                "https://app.clipflip.video/api/vast/tracking/x/start?v=x&gdpr=maybe",
                ArtifactKind::VastTracker,
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(
        summary.coverage().status,
        EndpointCoverageStatus::VendorChecked
    );
    assert!(summary.reports.iter().any(|report| {
        report.plugin_id == "core"
            && report.violations.iter().any(|finding| {
                finding.code == "core.privacy.gdpr_invalid" && finding.severity == Severity::Error
            })
    }));
    assert!(!summary.is_ok());
}
