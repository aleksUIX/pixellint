//! Neighboring vendor templates do not establish contracts for opaque collectors.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
    ValidationSummary,
};

const ROUTES: [(&str, &str); 4] = [
    ("https://unified.adsafeprotected.com/pixel.png", "ias"),
    ("https://ad-events.flashtalking.com/ft.stat", "flashtalking"),
    (
        "https://vfw.amazon-adsystem.com/ias/ivt/u/pixel.png",
        "amazon",
    ),
    (
        "https://vfw.amazon-adsystem.com/ias/ivt/fw/rfw/amazon/1111111/66666666/ias/s/skeleton.gif",
        "amazon",
    ),
];

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

fn assert_core_only(summary: &ValidationSummary, vendor: &str) {
    assert!(summary.reports.iter().any(|r| r.plugin_id == "core"));
    assert!(
        summary
            .reports
            .iter()
            .all(|r| !r.plugin_id.starts_with("vendor/")),
        "{summary:?}"
    );
    let directory = summary
        .reports
        .iter()
        .find(|r| r.plugin_id == "directory")
        .unwrap();
    assert_eq!(directory.detected_vendor.as_deref(), Some(vendor));
    assert_eq!(directory.violations.len(), 1);
    assert_eq!(
        directory.violations[0].code,
        "directory.no_rulepack_coverage"
    );
    assert_eq!(directory.violations[0].severity, Severity::Info);
}

#[test]
fn opaque_verification_collectors_expose_core_only_coverage_for_all_url_transports() {
    let engine = Engine::default();
    for (url, vendor) in ROUTES {
        let capture = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
        for (kind, artifact) in [
            (ArtifactKind::Url, url),
            (ArtifactKind::VastTracker, url),
            (ArtifactKind::NetworkRequest, capture.as_str()),
        ] {
            let summary = validate(&engine, kind, artifact);
            assert_core_only(&summary, vendor);
            assert!(summary.is_ok());
        }
    }
}

#[test]
fn unsupported_fields_do_not_inherit_sibling_templates_or_event_enumerations() {
    let engine = Engine::default();
    for (url, vendor) in [
        (
            "https://unified.adsafeprotected.com/pixel.png?advEntityId=opaque&pubEntityId=&a=custom",
            "ias",
        ),
        (
            "https://ad-events.flashtalking.com/ft.stat?opaque;tuple;without;an;invented;event;enum",
            "flashtalking",
        ),
        (
            "https://vfw.amazon-adsystem.com/ias/ivt/u/pixel.png?advEntityId=opaque&pubEntityId=&a=custom",
            "amazon",
        ),
        (
            "https://vfw.amazon-adsystem.com/ias/ivt/fw/rfw/amazon/1111111/66666666/ias/s/skeleton.gif?xsId=opaque",
            "amazon",
        ),
    ] {
        let summary = validate(&engine, ArtifactKind::VastTracker, url);
        assert_core_only(&summary, vendor);
        assert!(summary.is_ok(), "{url}: {summary:?}");
        assert!(
            summary.reports.iter().flat_map(|r| &r.violations).all(|v| {
                !v.code.contains("vstevt")
                    && !v.code.contains("advertiser_id")
                    && !v.code.contains("publisher_id")
            }),
            "{url}: {summary:?}"
        );
    }
}

#[test]
fn core_privacy_and_broken_macro_findings_survive_missing_vendor_contracts() {
    let engine = Engine::default();
    for (url, vendor) in ROUTES {
        let summary = validate(
            &engine,
            ArtifactKind::VastTracker,
            &format!("{url}?gdpr=NaN&cb=!!CACHEBUSTER!"),
        );
        assert_core_only(&summary, vendor);
        let core = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "core")
            .unwrap();
        for (code, severity) in [
            ("core.privacy.gdpr_invalid", Severity::Error),
            ("core.macro.broken_delimiter", Severity::Warning),
        ] {
            assert!(
                core.violations
                    .iter()
                    .any(|v| v.code == code && v.severity == severity),
                "{url}: missing {code}: {summary:?}"
            );
        }
        assert!(!summary.is_ok());
    }
}

#[test]
fn covered_sibling_routes_still_select_their_own_contracts() {
    let engine = Engine::default();
    for (url, pack) in [
        (
            "https://unified.adsafeprotected.com/v2/1111111/66666666",
            "vendor/ias-video",
        ),
        (
            "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=session-example",
            "vendor/ias-video-pixel",
        ),
        (
            "https://servedby.flashtalking.com/imp/1/111;222;333;pixel;site;placement/",
            "vendor/flashtalking-impression",
        ),
        (
            "https://vfw.amazon-adsystem.com/dv/event.png?vstevt=1",
            "vendor/amazon-vfw",
        ),
    ] {
        let summary = validate(&engine, ArtifactKind::VastTracker, url);
        assert!(
            summary.reports.iter().any(|r| r.plugin_id == pack),
            "{url}: {summary:?}"
        );
        assert!(summary.reports.iter().all(|r| r.plugin_id != "directory"));
    }
}

#[test]
fn unsupported_route_attribution_does_not_extend_to_host_lookalikes() {
    let engine = Engine::default();
    for host in [
        "unified.adsafeprotected.com.example.test",
        "ad-events.flashtalking.com.example.test",
        "vfw.amazon-adsystem.com.example.test",
    ] {
        assert!(engine.directory().lookup_host(host).is_none());
        let summary = validate(
            &engine,
            ArtifactKind::VastTracker,
            &format!("https://{host}/pixel.png?gdpr=NaN"),
        );
        assert!(summary.reports.iter().all(|r| r.plugin_id == "core"));
        assert!(!summary.is_ok());
    }
}
