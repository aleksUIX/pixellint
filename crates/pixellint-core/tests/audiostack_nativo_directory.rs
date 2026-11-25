use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ValidationOptions, ValidationRequest,
};
use serde_json::json;

fn validate(kind: ArtifactKind, artifact: String) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact,
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

#[test]
fn published_service_roots_have_attribution_and_core_only_coverage() {
    let engine = Engine::default();
    for (host, vendor, pack) in [
        ("vast.audiostack.ai", "audiostack", None),
        ("track.audiostack.ai", "audiostack", None),
        (
            "jadserve.postrelease.com",
            "nativo",
            Some("vendor/nativo-event-pixel"),
        ),
    ] {
        let entry = engine.directory().lookup_host(host).unwrap();
        assert_eq!(entry.vendor, vendor);
        assert_eq!(entry.rulepack.as_deref(), pack);
        for kind in [ArtifactKind::Url, ArtifactKind::VastTracker] {
            let summary = validate(kind, format!("https://{host}/synthetic/route?opaque=test"));
            assert!(summary.is_ok());
            assert_eq!(
                summary.coverage().status,
                EndpointCoverageStatus::RecognizedUncontracted
            );
            assert!(summary.coverage().core_checks_ran);
            assert!(summary.coverage().vendor_rulepacks.is_empty());
            let directory = summary
                .reports
                .iter()
                .find(|report| report.plugin_id == "directory")
                .unwrap();
            assert_eq!(directory.detected_vendor.as_deref(), Some(vendor));
            assert_eq!(
                directory.violations[0].code,
                "directory.no_rulepack_coverage"
            );
        }
    }
}

#[test]
fn nativo_pixel_selects_only_the_exact_host_route_and_query_profile() {
    const PACK: &str = "vendor/nativo-event-pixel";
    for kind in [ArtifactKind::Url, ArtifactKind::VastTracker] {
        let summary = validate(
            kind,
            "https://jadserve.postrelease.com/trk.gif?ntv_at=sample-event&ntv_a=sample-ad&ntv_ht=sample-token"
                .to_owned(),
        );
        assert_eq!(summary.coverage().vendor_rulepacks, [PACK]);
        assert!(
            summary
                .reports
                .iter()
                .all(|report| report.violations.is_empty())
        );
    }
    for url in [
        "https://jadserve.postrelease.com/trk?ntv_at=sample-event",
        "https://jadserve.postrelease.com/trk.gif/extra?ntv_at=sample-event",
        "https://jadserve.postrelease.com/trk.gif?other=sample-event",
        "https://child.jadserve.postrelease.com/trk.gif?ntv_at=sample-event",
        "https://jadserve.postrelease.com.evil.test/trk.gif?ntv_at=sample-event",
    ] {
        let summary = validate(ArtifactKind::Url, url.to_owned());
        assert!(summary.coverage().vendor_rulepacks.is_empty(), "{url}");
    }
}

#[test]
fn complete_captures_keep_core_privacy_errors_without_invented_field_contracts() {
    for host in [
        "vast.audiostack.ai",
        "track.audiostack.ai",
        "jadserve.postrelease.com",
    ] {
        let summary = validate(
            ArtifactKind::NetworkRequest,
            json!({
                "url":format!("https://{host}/synthetic?gdpr=NaN"),
                "method":"POST",
                "headers":{"Content-Type":"application/json"},
                "body":"{}"
            })
            .to_string(),
        );
        assert!(!summary.is_ok(), "{host}: core privacy validation must run");
        assert_eq!(
            summary.coverage().status,
            EndpointCoverageStatus::RecognizedUncontracted
        );
        assert!(summary.coverage().core_checks_ran);
        assert!(summary.coverage().vendor_rulepacks.is_empty());
        assert!(
            !summary
                .reports
                .iter()
                .any(|report| report.plugin_id.starts_with("vendor/"))
        );
    }
}

#[test]
fn parent_domains_lookalikes_and_staging_siblings_remain_unknown() {
    let engine = Engine::default();
    for host in [
        "audiostack.ai",
        "postrelease.com",
        "track.audiostack.ai.evil.test",
        "jadserve.postrelease.com.evil.test",
        "fake-track.audiostack.ai",
        "staging-track.audiostack.ai",
        "staging-vast.audiostack.ai",
        "staging-jadserve.postrelease.com",
    ] {
        assert!(engine.directory().lookup_host(host).is_none(), "{host}");
        let summary = validate(ArtifactKind::Url, format!("https://{host}/synthetic"));
        assert_eq!(
            summary.coverage().status,
            EndpointCoverageStatus::Unrecognized,
            "{host}"
        );
        assert!(summary.coverage().vendor_rulepacks.is_empty());
    }
}

#[test]
fn existing_child_subdomain_semantics_are_preserved() {
    let engine = Engine::default();
    for (host, vendor) in [
        ("child.track.audiostack.ai", "audiostack"),
        ("child.vast.audiostack.ai", "audiostack"),
        ("child.jadserve.postrelease.com", "nativo"),
    ] {
        assert_eq!(engine.directory().lookup_host(host).unwrap().vendor, vendor);
    }
}
