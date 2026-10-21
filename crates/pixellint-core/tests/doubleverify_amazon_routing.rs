use pixellint_core::{
    ArtifactKind, Engine, ValidationOptions, ValidationRequest, ValidationSummary,
};

fn validate(engine: &Engine, kind: ArtifactKind, url: &str) -> ValidationSummary {
    let artifact = if kind == ArtifactKind::NetworkRequest {
        serde_json::json!({"method": "GET", "url": url, "headers": {}}).to_string()
    } else {
        url.to_string()
    };
    engine
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

fn vendor_reports(summary: &ValidationSummary) -> Vec<&str> {
    summary
        .reports
        .iter()
        .filter(|report| report.plugin_id.starts_with("vendor/"))
        .map(|report| report.plugin_id.as_str())
        .collect()
}

#[test]
fn all_supported_doubleverify_hosts_select_the_endpoint_contract_across_inputs() {
    let engine = Engine::default();
    for host in [
        "tps.doubleverify.com",
        "tpsc-video.doubleverify.com",
        "tpsc-video-as.doubleverify.com",
        "tpsc-video-eu.doubleverify.com",
        "tpsc-video-ue.doubleverify.com",
        "tpsc-video-uw.doubleverify.com",
        "TPSC-VIDEO.DOUBLEVERIFY.COM",
    ] {
        for (path, query, pack) in [
            (
                "/visit.jpg",
                "ctx=12345&cmp=12345&plc=12345&sid=12345&vstevt=2",
                "vendor/doubleverify",
            ),
            (
                "/event.png",
                "vstevt=4&dup=synthetic-impression",
                "vendor/doubleverify-event",
            ),
        ] {
            for kind in [
                ArtifactKind::Url,
                ArtifactKind::VastTracker,
                ArtifactKind::NetworkRequest,
            ] {
                let url = format!("https://{host}{path}?{query}");
                let summary = validate(&engine, kind, &url);
                assert_eq!(vendor_reports(&summary), [pack], "{kind:?}: {url}");
                assert!(summary.is_ok(), "{kind:?}: {url}");
            }
        }
    }
}

#[test]
fn doubleverify_rejects_lookalike_hosts_and_non_endpoint_paths() {
    let engine = Engine::default();
    for host in [
        "tpsc-video.doubleverify.com.example.test",
        "fake-tpsc-video.doubleverify.com",
        "unreviewed.doubleverify.com",
    ] {
        for path in ["/visit.jpg", "/event.png"] {
            let summary = validate(&engine, ArtifactKind::Url, &format!("https://{host}{path}"));
            assert!(vendor_reports(&summary).is_empty(), "{host}{path}");
        }
    }
    for path in [
        "/visit.jpg.backup",
        "/visit.jpg/unrelated",
        "/event.png.backup",
        "/event.png/unrelated",
        "/unrelated/event.png",
    ] {
        let summary = validate(
            &engine,
            ArtifactKind::Url,
            &format!("https://tpsc-video.doubleverify.com{path}"),
        );
        assert!(vendor_reports(&summary).is_empty(), "{path}");
    }
}

#[test]
fn uncontracted_amazon_telemetry_preserves_core_privacy_errors() {
    let engine = Engine::default();
    let summary = validate(
        &engine,
        ArtifactKind::VastTracker,
        "https://vfw.amazon-adsystem.com/dv/vtrk.doubleverify.com/?ec=vast&ea=complete&gdpr=invalid",
    );
    assert!(vendor_reports(&summary).is_empty());
    let codes: Vec<_> = summary
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .map(|violation| violation.code.as_str())
        .collect();
    assert!(codes.contains(&"core.privacy.gdpr_invalid"));
    assert!(codes.contains(&"directory.no_rulepack_coverage"));
    assert!(!summary.is_ok());
}
