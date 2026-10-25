//! Published Gamesight impression options stay scoped to the image tracker.

use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, ValidationOptions,
    ValidationRequest,
};

fn request(kind: ArtifactKind, artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact: artifact.to_owned(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Template,
    }
}

#[test]
fn gamesight_impression_contract_is_consistent_across_url_like_inputs() {
    let engine = Engine::default();
    let url = "https://gsght.com/imgp/SYNTHETIC?cost_model=CPA&xid[0][sha256]=server&xid[1][sha256]=md5&xid[1][scope_type]=team";
    let capture = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
    for (kind, artifact) in [
        (ArtifactKind::Url, url),
        (ArtifactKind::VastTracker, url),
        (ArtifactKind::NetworkRequest, capture.as_str()),
    ] {
        let summary = engine
            .validate_at(
                &request(kind, artifact),
                &ValidationOptions::default(),
                1_792_526_400,
            )
            .unwrap();
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::VendorChecked
        );
        assert_eq!(
            summary.coverage.vendor_rulepacks,
            ["vendor/gamesight-impression"]
        );
        let report = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "vendor/gamesight-impression")
            .unwrap();
        for code in [
            "vendor.gamesight-impression.param.cost_model.invalid",
            "vendor.gamesight-impression.param.xid[1][sha256].invalid",
            "vendor.gamesight-impression.param.xid[1][scope_type].invalid",
        ] {
            assert!(
                report.violations.iter().any(|v| v.code == code),
                "{kind:?}: {report:?}"
            );
        }
    }
}

#[test]
fn click_custom_domain_and_nearby_routes_do_not_select_the_impression_pack() {
    let engine = Engine::default();
    for url in [
        "https://gsght.com/c/SYNTHETIC?destination_url=https%3A%2F%2Fdestination.example",
        "https://gsght.com/imgp/SYNTHETIC/extra",
        "https://gsght.com/imgp/SYNTHETIC/",
        "https://custom.game.example/imgp/SYNTHETIC",
        "https://sub.gsght.com/imgp/SYNTHETIC",
        "https://gsght.com.example/imgp/SYNTHETIC",
        "https://notgsght.com/imgp/SYNTHETIC",
    ] {
        let summary = engine
            .validate(
                &request(ArtifactKind::Url, url),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(
            !summary
                .coverage
                .vendor_rulepacks
                .iter()
                .any(|p| p == "vendor/gamesight-impression"),
            "{url}: {summary:?}"
        );
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .all(|v| !v.code.starts_with("vendor.gamesight-impression.")),
            "{url}: {summary:?}"
        );
    }
}

#[test]
fn query_cannot_replace_the_opaque_path_identity_and_macros_remain_deferred() {
    let engine = Engine::default();
    let summary = engine
        .validate(
            &request(
                ArtifactKind::Url,
                "https://gsght.com/imgp/?tracker_id=SYNTHETIC",
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code == "vendor.gamesight-impression.param.tracker_id.empty")
    );
    let summary = engine
        .validate(
            &request(
                ArtifactKind::Url,
                "https://gsght.com/imgp/SYNTHETIC?cost_model=[CACHEBUSTING]",
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .all(|v| v.code != "vendor.gamesight-impression.param.cost_model.invalid")
    );
}
