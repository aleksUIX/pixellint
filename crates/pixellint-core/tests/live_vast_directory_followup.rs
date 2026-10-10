use pixellint_core::{
    ArtifactKind, Engine, Severity, ValidationOptions, ValidationRequest, ValidationSummary,
    VendorDirectory,
};

const OBSERVED_HOSTS: &[(&str, &str)] = &[
    ("ads55.adtelligent.com", "adtelligent"),
    ("ads228.adtelligent.com", "adtelligent"),
    ("ads283.adtelligent.com", "adtelligent"),
    ("br-trk.smadex.com", "smadex"),
    ("cr-err.smadex.com", "smadex"),
    ("ec-ed.smadex.com", "smadex"),
    ("geo-tracker.smadex.com", "smadex"),
    ("pixel-ed.smadex.com", "smadex"),
    ("va-trk.smadex.com", "smadex"),
    ("n.adv.lijit.com", "sovrn"),
    ("ads161.krushmedia.com", "krushmedia"),
    ("segment.prod.bidr.io", "beeswax"),
];

fn validate(engine: &Engine, url: String) -> ValidationSummary {
    engine
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::VastTracker,
                artifact: url,
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

#[test]
fn attributed_hosts_report_core_only_coverage_without_vendor_contracts() {
    let engine = Engine::default();
    for &(host, vendor) in OBSERVED_HOSTS {
        let entry = engine.directory().lookup_host(host).unwrap();
        assert_eq!(entry.vendor, vendor);
        assert!(entry.rulepack.is_none());

        let summary = validate(&engine, format!("https://{host}/?gdpr=0"));
        let attribution = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == "directory")
            .unwrap();
        assert_eq!(attribution.detected_vendor.as_deref(), Some(vendor));
        assert_eq!(attribution.violations.len(), 1);
        assert_eq!(
            attribution.violations[0].code,
            "directory.no_rulepack_coverage"
        );
        assert_eq!(attribution.violations[0].severity, Severity::Info);
        assert!(
            summary
                .reports
                .iter()
                .any(|report| report.plugin_id == "core")
        );
        assert!(
            summary
                .reports
                .iter()
                .all(|report| !report.plugin_id.starts_with("vendor/"))
        );
        assert!(summary.is_ok(), "{host}: attribution adds no error");
    }
}

#[test]
fn attribution_preserves_the_full_core_report_and_invalid_gdpr_error() {
    let engine = Engine::default();
    let mut without_directory = Engine::default();
    without_directory.set_directory(VendorDirectory::default());

    for &(host, _) in OBSERVED_HOSTS {
        let url = format!("https://{host}/?gdpr=NaN");
        let attributed = validate(&engine, url.clone());
        let unattributed = validate(&without_directory, url);
        let core = attributed
            .reports
            .iter()
            .find(|report| report.plugin_id == "core")
            .unwrap();
        let baseline = unattributed
            .reports
            .iter()
            .find(|report| report.plugin_id == "core")
            .unwrap();
        assert_eq!(
            core, baseline,
            "{host}: attribution preserves core findings"
        );
        assert!(
            core.violations.iter().any(|violation| {
                violation.code == "core.privacy.gdpr_invalid"
                    && violation.severity == Severity::Error
            }),
            "{host}: the malformed privacy signal remains an error"
        );
        assert!(!attributed.is_ok());
    }
}

#[test]
fn new_hosts_do_not_claim_lookalikes_or_unreviewed_sibling_domains() {
    let engine = Engine::default();
    for &(host, _) in OBSERVED_HOSTS {
        for lookalike in [format!("{host}.example.test"), format!("fake-{host}")] {
            assert!(
                engine.directory().lookup_host(&lookalike).is_none(),
                "{lookalike}: host boundaries prevent attribution"
            );
        }
    }
    for unreviewed in [
        "adtelligent.com",
        "ads999.adtelligent.com",
        "smadex.com",
        "unreviewed.smadex.com",
        "lijit.com",
        "unreviewed.adv.lijit.com",
        "ads999.krushmedia.com",
        "unreviewed.prod.bidr.io",
    ] {
        assert!(
            engine.directory().lookup_host(unreviewed).is_none(),
            "{unreviewed}: only observed hosts are listed"
        );
    }
}
