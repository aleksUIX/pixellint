//! Index namespace attribution does not assert an event collector contract.

use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, Severity, ValidationOptions,
    ValidationRequest, ValidationSummary,
};

fn validate(engine: &Engine, kind: ArtifactKind, artifact: &str) -> ValidationSummary {
    engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: kind,
                artifact: artifact.to_owned(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
            1_792_526_400,
        )
        .unwrap()
}

#[test]
fn observed_index_roots_attribute_url_tracker_and_request_inputs() {
    let engine = Engine::default();
    for host in [
        "a6111.casalemedia.com",
        "a6124.casalemedia.com",
        "k8s1-event-tracker-va.lb.indexww.com",
    ] {
        for path in ["/events/v1/synthetic", "/event/synthetic", "/"] {
            let url = format!("https://{host}{path}");
            let capture =
                serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
            for (kind, artifact) in [
                (ArtifactKind::Url, url.as_str()),
                (ArtifactKind::VastTracker, url.as_str()),
                (ArtifactKind::NetworkRequest, capture.as_str()),
            ] {
                let summary = validate(&engine, kind, artifact);
                assert_eq!(
                    summary.coverage.status,
                    EndpointCoverageStatus::RecognizedUncontracted,
                    "{kind:?} {artifact}: {summary:?}"
                );
                assert!(summary.coverage.core_checks_ran);
                assert!(summary.coverage.vendor_rulepacks.is_empty());
                let directory = summary
                    .reports
                    .iter()
                    .find(|r| r.plugin_id == "directory")
                    .unwrap();
                assert_eq!(directory.detected_vendor.as_deref(), Some("indexexchange"));
                assert!(
                    directory
                        .violations
                        .iter()
                        .any(|v| v.code == "directory.no_rulepack_coverage")
                );
            }
        }
    }
}

#[test]
fn lookalikes_unconfigured_siblings_and_rtactivate_remain_unattributed() {
    let engine = Engine::default();
    for url in [
        "https://a6111.casalemedia.com.example/event",
        "https://nota6111.casalemedia.com/event",
        "https://unconfigured.casalemedia.com/event",
        "https://unconfigured.indexww.com/event",
        "https://k8s1-event-tracker-va.lb.indexww.com.example/event",
        "https://idxgm.rtactivate.com/tagid/synthetic/",
    ] {
        let summary = validate(&engine, ArtifactKind::Url, url);
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::Unrecognized,
            "{url}: {summary:?}"
        );
    }
}

#[test]
fn existing_index_catalog_roots_and_core_privacy_findings_are_preserved() {
    let engine = Engine::default();
    for host in [
        "htlb.casalemedia.com",
        "js-sec.indexww.com",
        "a6111.casalemedia.com",
        "a6124.casalemedia.com",
        "k8s1-event-tracker-va.lb.indexww.com",
    ] {
        let summary = validate(
            &engine,
            ArtifactKind::Url,
            &format!("https://{host}/event/synthetic?gdpr=NaN"),
        );
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::RecognizedUncontracted
        );
        assert!(
            summary
                .reports
                .iter()
                .filter(|r| r.plugin_id == "core")
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "core.privacy.gdpr_invalid" && v.severity == Severity::Error)
        );
    }
}
