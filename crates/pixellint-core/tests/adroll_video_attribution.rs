//! AdRoll video host attribution does not claim the conversion pixel contract.

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
fn exact_adroll_video_host_is_attributed_without_selecting_conversion_packs() {
    let engine = Engine::default();
    for host in ["x.adroll.com", "X.ADROLL.COM:443"] {
        for path in [
            "/video/synthetic/start",
            "/pixel/SYNTHETICADV/SYNTHETICPIX",
            "/",
        ] {
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
                assert_eq!(directory.detected_vendor.as_deref(), Some("adroll"));
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
fn adroll_directory_entry_does_not_claim_lookalikes_or_outside_siblings() {
    let engine = Engine::default();
    for host in ["x.adroll.com.example", "notx.adroll.com", "xadroll.com"] {
        let summary = validate(
            &engine,
            ArtifactKind::Url,
            &format!("https://{host}/video/synthetic/start"),
        );
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::Unrecognized
        );
        assert!(
            summary
                .reports
                .iter()
                .all(|r| r.detected_vendor.as_deref() != Some("adroll"))
        );
    }
}

#[test]
fn existing_adroll_conversion_endpoints_keep_their_selected_packs() {
    let engine = Engine::default();
    for (url, pack) in [
        (
            include_str!("../../../fixtures/vendor-adroll-pixel/depth048-basic.txt"),
            "vendor/adroll-pixel",
        ),
        (
            include_str!("../../../fixtures/vendor-adroll-image/depth048-segment.txt"),
            "vendor/adroll-image",
        ),
    ] {
        let summary = validate(&engine, ArtifactKind::Url, url.trim());
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::VendorChecked
        );
        assert_eq!(summary.coverage.vendor_rulepacks, [pack]);
        assert!(summary.is_ok());
    }
}

#[test]
fn core_privacy_validation_stays_active_on_uncontracted_adroll_routes() {
    let summary = validate(
        &Engine::default(),
        ArtifactKind::Url,
        "https://x.adroll.com/video/synthetic/start?gdpr=NaN",
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
