//! Published Trade Desk examples support attribution without inventing contracts.

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
fn published_trade_desk_hosts_receive_attribution_without_protocol_checks() {
    let engine = Engine::default();
    for url in [
        "https://enduser.adsrvr.org/enduser/video/?opaque=synthetic",
        "https://enduser.adsrvr.org/enduser/pie/?opaque=synthetic",
        "https://vam-bid.adsrvr.org/bid/feedback/opaque",
    ] {
        let capture = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
        for (kind, artifact) in [
            (ArtifactKind::Url, url),
            (ArtifactKind::VastTracker, url),
            (ArtifactKind::NetworkRequest, capture.as_str()),
        ] {
            let summary = validate(&engine, kind, artifact);
            assert_eq!(
                summary.coverage.status,
                EndpointCoverageStatus::RecognizedUncontracted,
                "{summary:?}"
            );
            assert!(summary.coverage.core_checks_ran);
            assert!(summary.coverage.vendor_rulepacks.is_empty());
            let directory = summary
                .reports
                .iter()
                .find(|report| report.plugin_id == "directory")
                .unwrap();
            assert_eq!(directory.detected_vendor.as_deref(), Some("thetradedesk"));
            assert!(directory.violations.iter().any(|violation| {
                violation.code == "directory.no_rulepack_coverage"
                    && violation.severity == Severity::Info
            }));
            assert!(summary.is_ok(), "{summary:?}");
        }
    }
}

#[test]
fn new_attribution_keeps_lookalikes_and_undocumented_siblings_unrecognized() {
    let engine = Engine::default();
    for host in [
        "enduser.adsrvr.org.evil.test",
        "notenduser.adsrvr.org",
        "vam-bid.adsrvr.org.evil.test",
        "notvam-bid.adsrvr.org",
        "vat-bid.adsrvr.org",
        "sg2-bid.adsrvr.org",
        "wa2-bid.adsrvr.org",
        "adsrvr.org",
    ] {
        let summary = validate(
            &engine,
            ArtifactKind::Url,
            &format!("https://{host}/enduser/video/"),
        );
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::Unrecognized
        );
        assert!(summary.coverage.core_checks_ran);
        assert!(summary.coverage.vendor_rulepacks.is_empty());
    }
}

#[test]
fn core_privacy_findings_and_existing_trade_desk_checks_are_preserved() {
    let engine = Engine::default();
    for host in ["enduser.adsrvr.org", "vam-bid.adsrvr.org"] {
        let summary = validate(
            &engine,
            ArtifactKind::VastTracker,
            &format!("https://{host}/uncontracted?gdpr=NaN"),
        );
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::RecognizedUncontracted
        );
        assert!(summary.reports.iter().any(|report| {
            report.plugin_id == "core"
                && report.violations.iter().any(|violation| {
                    violation.code == "core.privacy.gdpr_invalid"
                        && violation.severity == Severity::Error
                })
        }));
    }
    let covered = validate(
        &engine,
        ArtifactKind::Url,
        "https://insight.adsrvr.org/track/up?adv=synthetic&upid=synthetic&ref=https%3A%2F%2Fexample.org%2F&upv=3.3.0",
    );
    assert_eq!(
        covered.coverage.status,
        EndpointCoverageStatus::VendorChecked
    );
    assert!(
        covered
            .coverage
            .vendor_rulepacks
            .iter()
            .any(|pack| pack == "vendor/the-trade-desk")
    );
}
