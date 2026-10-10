use pixellint_core::{ArtifactKind, Engine, ValidationOptions, ValidationRequest};

fn validate(url: &str) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::VastTracker,
                artifact: url.to_string(),
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

#[test]
fn installation_stub_is_an_error_even_before_template_expansion() {
    for value in [
        "[PLEASE_IMPLEMENT_UNIQUE_ADSERVER_IMPRESSION_ID_HERE]",
        "%5BPLEASE_IMPLEMENT_UNIQUE_ADSERVER_IMPRESSION_ID_HERE%5D",
    ] {
        let summary = validate(&format!(
            "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId={value}"
        ));
        let pack = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == "vendor/ias-video-pixel")
            .unwrap();
        assert!(
            pack.violations
                .iter()
                .any(|v| v.code == "vendor.ias-video-pixel.xsid.placeholder")
        );
        assert!(!summary.is_ok());
    }
}

#[test]
fn session_ids_and_real_template_macros_remain_accepted() {
    for value in [
        "session-123",
        "ac9e521e-3765-4e5f-a3d4-3fc3c6d035ed",
        "${AD_SESSION_ID}",
        "[TRANSACTIONID]",
    ] {
        let summary = validate(&format!(
            "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId={value}"
        ));
        let pack = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == "vendor/ias-video-pixel")
            .unwrap();
        assert!(pack.violations.is_empty(), "{value}: {:?}", pack.violations);
    }
    let summary = validate(
        "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=session-123&campaign_id=[PLEASE_IMPLEMENT_UNIQUE_ADSERVER_IMPRESSION_ID_HERE]",
    );
    assert!(
        summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "vendor/ias-video-pixel")
            .unwrap()
            .violations
            .is_empty()
    );
}

#[test]
fn observed_hosts_get_attribution_and_core_checks_without_invented_contracts() {
    let engine = Engine::default();
    for (host, vendor) in [
        ("00px.net", "adxspace"),
        ("t.stredeo.com", "stredeo"),
        ("us-east-1.event.prod.bidr.io", "beeswax"),
        ("rm.aarki.net", "aarki"),
        ("track.adwrap.io", "adwrap"),
        ("ads106.krushmedia.com", "krushmedia"),
        ("ads133.krushmedia.com", "krushmedia"),
    ] {
        let entry = engine.directory().lookup_host(host).unwrap();
        assert_eq!(entry.vendor, vendor);
        if vendor == "adxspace" {
            assert_eq!(entry.rulepack.as_deref(), Some("vendor/adxspace-pixel"));
        } else {
            assert!(entry.rulepack.is_none());
        }
        assert!(
            engine
                .directory()
                .lookup_host(&format!("{host}.example.test"))
                .is_none()
        );
        let summary = validate(&format!("https://{host}/?gdpr=NaN"));
        let attribution = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "directory")
            .unwrap();
        assert_eq!(attribution.detected_vendor.as_deref(), Some(vendor));
        assert_eq!(
            attribution.violations[0].code,
            "directory.no_rulepack_coverage"
        );
        assert!(!summary.is_ok(), "{host}: privacy checks still run");
        assert!(
            !summary
                .reports
                .iter()
                .any(|r| r.plugin_id.starts_with("vendor/"))
        );
    }
}
