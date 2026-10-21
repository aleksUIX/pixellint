//! Public ownership evidence attributes observed endpoints without inventing wire contracts.

use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, Severity, ValidationOptions,
    ValidationRequest, ValidationSummary, VendorDirectory,
};

const ATTRIBUTED: &[(&str, &str)] = &[
    ("https://ams.creativecdn.com/ad/imp-tags", "rtbhouse"),
    (
        "https://kimberlite.io/rtb/pix/synthetic-event/opaque-token",
        "kimberlite",
    ),
    ("https://yandex.ru/adfox/999999/event", "yandex-adfox"),
    ("https://yandex.ru/adfox/999999/goLink", "yandex-adfox"),
];

fn validate(engine: &Engine, url: &str) -> ValidationSummary {
    engine
        .validate(
            &ValidationRequest {
                artifact: url.to_owned(),
                artifact_kind: ArtifactKind::VastTracker,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

fn assert_attribution(summary: &ValidationSummary, vendor: &str) {
    assert_eq!(
        summary.coverage().status,
        EndpointCoverageStatus::RecognizedUncontracted
    );
    assert!(summary.coverage().core_checks_ran);
    assert!(summary.coverage().vendor_rulepacks.is_empty());
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
    assert!(
        !summary
            .reports
            .iter()
            .any(|r| r.plugin_id.starts_with("vendor/"))
    );
}

#[test]
fn observed_hosts_and_scoped_adfox_routes_get_attribution_without_contract_claims() {
    let engine = Engine::default();
    for &(url, vendor) in ATTRIBUTED {
        let summary = validate(&engine, url);
        assert_attribution(&summary, vendor);
        assert!(summary.is_ok(), "{url}");
    }
    let directory = engine.directory();
    assert_eq!(
        directory.lookup_host("ams.creativecdn.com").unwrap().vendor,
        "rtbhouse"
    );
    assert_eq!(
        directory.lookup_host("kimberlite.io").unwrap().vendor,
        "kimberlite"
    );
    assert_eq!(
        directory
            .lookup_host("tpsc-video.doubleverify.com")
            .unwrap()
            .vendor,
        "doubleverify"
    );
    assert!(directory.lookup_host("yandex.ru").is_none());
    let scoped = directory
        .lookup_endpoint("yandex.ru", "/adfox/999999/event")
        .unwrap();
    assert_eq!(scoped.vendor, "yandex-adfox");
    assert_eq!(scoped.path_prefixes, ["/adfox/", "/ads/adfox/"]);
    assert_eq!(
        scoped.rulepack.as_deref(),
        Some("vendor/yandex-adfox-links")
    );
}

#[test]
fn new_attribution_preserves_the_complete_core_report() {
    let engine = Engine::default();
    let mut baseline = Engine::default();
    baseline.set_directory(VendorDirectory::default());
    for &(url, vendor) in ATTRIBUTED {
        let url = format!("{url}?gdpr=NaN&cb=!!CACHEBUSTER!");
        let attributed = validate(&engine, &url);
        let unattributed = validate(&baseline, &url);
        assert_attribution(&attributed, vendor);
        let core = attributed
            .reports
            .iter()
            .find(|r| r.plugin_id == "core")
            .unwrap();
        let original = unattributed
            .reports
            .iter()
            .find(|r| r.plugin_id == "core")
            .unwrap();
        assert_eq!(
            core, original,
            "{url}: attribution must preserve core checks"
        );
        for (code, severity) in [
            ("core.privacy.gdpr_invalid", Severity::Error),
            ("core.macro.broken_delimiter", Severity::Warning),
        ] {
            assert!(
                core.violations
                    .iter()
                    .any(|v| v.code == code && v.severity == severity),
                "{url}: {code}"
            );
        }
        assert!(!attributed.is_ok());
    }
}

#[test]
fn attribution_rejects_lookalike_hosts_and_unreviewed_sibling_domains() {
    let engine = Engine::default();
    for url in [
        "https://ams.creativecdn.com.attacker.test/ad/imp-tags",
        "https://fake-ams.creativecdn.com/ad/imp-tags",
        "https://kimberlite.io.attacker.test/rtb/pix/event",
        "https://fake-kimberlite.io/rtb/pix/event",
        "https://yandex.ru.attacker.test/adfox/999999/event",
        "https://fake-yandex.ru/adfox/999999/event",
        "https://fra.creativecdn.com/ad/imp-tags",
        "https://creativecdn.com/ad/imp-tags",
        "https://im1.ufouxbwn.com/",
        "https://ufouxbwn.com/",
    ] {
        let summary = validate(&engine, url);
        assert_eq!(
            summary.coverage().status,
            EndpointCoverageStatus::Unrecognized,
            "{url}"
        );
        assert!(summary.coverage().core_checks_ran);
        assert!(summary.coverage().vendor_rulepacks.is_empty());
        assert!(
            !summary.reports.iter().any(|r| r.plugin_id == "directory"),
            "{url}"
        );
    }
}

#[test]
fn adfox_scope_uses_the_browser_path_and_excludes_other_yandex_services() {
    let engine = Engine::default();
    for url in [
        "https://yandex.ru/",
        "https://yandex.ru/search",
        "https://yandex.com/search",
        "https://yandex.ru/ads/adfox/../search",
        "https://yandex.ru/adfox",
        "https://yandex.ru/adfox-other/999999/event",
        "https://yandex.ru/Adfox/999999/event",
        "https://yandex.ru/search?redirect=/adfox/999999/event",
        "https://yandex.ru/#/adfox/999999/event",
        "https://yandex.ru/adfox/../search",
        "https://yandex.ru/adfox/%2e%2e/search",
        "https://yandex.ru/adfox/%2E./search",
    ] {
        let summary = validate(&engine, url);
        assert_eq!(
            summary.coverage().status,
            EndpointCoverageStatus::Unrecognized,
            "{url}"
        );
        assert!(
            !summary.reports.iter().any(|r| r.plugin_id == "directory"),
            "{url}"
        );
    }
    for url in [
        "https://yandex.ru/other/../adfox/999999/event",
        "https://yandex.ru/ads/adfox/999999/event",
        "https://YANDEX.RU/adfox/999999/goLink",
    ] {
        assert_attribution(&validate(&engine, url), "yandex-adfox");
    }
}
