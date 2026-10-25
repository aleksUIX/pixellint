//! Approved measurement roots are attributed without inventing pixel contracts.

use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, ValidationOptions,
    ValidationRequest,
};

fn request(kind: ArtifactKind, artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact: artifact.to_owned(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn approved_roku_measurement_roots_are_attributed_across_url_like_inputs() {
    let engine = Engine::default();
    for (host, vendor) in [
        ("p.placed.com", "placed"),
        ("pixel.mtrcs.samba.tv", "samba-tv"),
        ("lighthouse.edoinc.com", "edo"),
        ("p.tvpixel.com", "liveramp"),
    ] {
        let url = format!("https://{host}/synthetic-pixel");
        let capture = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
        for (kind, artifact) in [
            (ArtifactKind::Url, url.as_str()),
            (ArtifactKind::VastTracker, url.as_str()),
            (ArtifactKind::NetworkRequest, capture.as_str()),
        ] {
            let summary = engine
                .validate(&request(kind, artifact), &ValidationOptions::default())
                .unwrap();
            assert_eq!(
                summary.coverage.status,
                EndpointCoverageStatus::RecognizedUncontracted
            );
            assert!(summary.coverage.vendor_rulepacks.is_empty());
            assert!(summary.coverage.core_checks_ran);
            assert!(summary.reports.iter().any(
                |r| r.plugin_id == "directory" && r.detected_vendor.as_deref() == Some(vendor)
            ));
        }
    }
}

#[test]
fn lookalikes_and_unconfigured_measurement_siblings_remain_unattributed() {
    let engine = Engine::default();
    for host in [
        "notp.placed.com",
        "p.placed.com.example",
        "placed.com",
        "other.samba.tv",
        "pixel.mtrcs.samba.tv.example",
        "edoinc.com",
        "lighthouse.edoinc.com.example",
        "tvpixel.com",
        "p.tvpixel.com.example",
    ] {
        let summary = engine
            .validate(
                &request(
                    ArtifactKind::Url,
                    &format!("https://{host}/synthetic-pixel"),
                ),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::Unrecognized,
            "{host}: {summary:?}"
        );
    }
}

#[test]
fn a_measurement_alias_does_not_select_the_liveramp_identity_contract() {
    let summary = Engine::default()
        .validate(
            &request(
                ArtifactKind::Url,
                "https://p.tvpixel.com/api/envelope?gdpr=NaN",
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(summary.coverage.vendor_rulepacks.is_empty());
    assert_eq!(
        summary.coverage.status,
        EndpointCoverageStatus::RecognizedUncontracted
    );
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code == "core.privacy.gdpr_invalid")
    );
}
