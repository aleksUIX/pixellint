use pixellint_core::{
    ArtifactKind, ExpansionState, ManifestRulePack, RuleSourceLevel, Severity, ValidationRequest,
    ValidatorPlugin,
};
use serde_json::Value;

const TD: &str = include_str!("../rulepacks/vendor/the-trade-desk-static.json");
const RTB: &str = include_str!("../rulepacks/vendor/rtbhouse-gtm-tags.json");
const WCM: &str = include_str!("../rulepacks/vendor/weborama-wcm-js-impression.json");

fn request(url: &str) -> ValidationRequest {
    ValidationRequest {
        artifact: url.into(),
        artifact_kind: ArtifactKind::Url,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn independent_oracle_outputs_keep_compound_ids_repeated_tags_and_native_coercions() {
    for (manifest, source) in [
        (
            TD,
            include_str!("../../../fixtures/vendor-the-trade-desk-static/source-cases.json"),
        ),
        (
            RTB,
            include_str!("../../../fixtures/vendor-rtbhouse-gtm-tags/source-cases.json"),
        ),
        (
            WCM,
            include_str!("../../../fixtures/vendor-weborama-wcm-js-impression/source-cases.json"),
        ),
    ] {
        let pack = ManifestRulePack::from_json(manifest).unwrap();
        let source: Value = serde_json::from_str(source).unwrap();
        assert_eq!(source["engine_derived"], false);
        for case in source["cases"].as_array().unwrap() {
            let input = request(case["url"].as_str().unwrap());
            assert!(pack.supports(&input), "{}", case["id"]);
            let report = pack.validate(&input);
            for finding in &report.violations {
                assert_eq!(
                    finding.code, "vendor.the-trade-desk-static.legacy_route",
                    "{}: {report:?}",
                    case["id"]
                );
                assert_eq!(finding.severity, Severity::Warning);
            }
        }
    }
}

#[test]
fn ttd_static_never_claims_other_existing_transports_or_generic_bodies() {
    let pack = ManifestRulePack::from_json(TD).unwrap();
    for url in [
        "https://insight.adsrvr.org/track/up?adv=x&upid=y&ref=https%3A%2F%2Fexample.test&upv=x",
        "https://insight.adsrvr.org/track/cei?advertiser_id=x&cookie_sync=1&upv=x",
        "https://insight.adsrvr.org/track/realtimeconversion",
        "https://insight.adsrvr.org/track/rt?adv=x&pixel_ids=y",
        "https://enduser.adsrvr.org/enduser/pie/?rtb=synthetic",
        "https://enduser.adsrvr.org/enduser/video/?rtb=synthetic",
        "https://vam-bid.adsrvr.org/bid/feedback/google?t=synthetic",
        "https://insight.adsrvr.org/track/pxl?adv=x&ct=0:y&fmt=3",
    ] {
        assert!(!pack.supports(&request(url)), "{url}");
    }
    let input = ValidationRequest {
        artifact: r#"{"adv":"x","ct":"0:y","fmt":"3"}"#.into(),
        artifact_kind: ArtifactKind::JsonPayload,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    };
    assert!(!pack.supports(&input));
}

#[test]
fn gtm_and_wcm_exclude_opaque_callbacks_siblings_and_conflicting_markers() {
    let rtb = ManifestRulePack::from_json(RTB).unwrap();
    for url in [
        "https://ams.creativecdn.com/ad/imp-tags?type=iframe",
        "https://ams.creativecdn.com/ad/win-notify?type=iframe",
        "https://ash.creativecdn.com/ad/win-notify?type=iframe",
        "https://ams.creativecdn.com/imp-delivery?type=iframe",
        "https://ash.creativecdn.com/tags?type=iframe",
        "https://ams.creativecdn.com/tags?type=iframe&type=image",
        "https://ams.creativecdn.com/tags?type=iframe&type",
    ] {
        assert!(!rtb.supports(&request(url)), "{url}");
    }
    let wcm = ManifestRulePack::from_json(WCM).unwrap();
    for url in [
        "https://wcm.weborama-tech.ru/fcgi-bin/dispatch.fcgi?a.A=im&a.hr=js",
        "https://customer.solution.weborama.fr/fcgi-bin/dispatch.fcgi?a.A=im&a.hr=js",
        "https://wcmvideotutorials.solution.weborama.fr/fcgi-bin/dispatch.fcgi?a.A=cl&a.hr=js",
        "https://wcmvideotutorials.solution.weborama.fr/fcgi-bin/dispatch.fcgi?a.A=im&a.hr=R",
        "https://wcmvideotutorials.solution.weborama.fr/fcgi-bin/dispatch.fcgi?a.A=im&a.hr=js&a.A=cl",
        "https://wcmvideotutorials.solution.weborama.fr/fcgi-bin/dispatch.fcgi?a.A=im&a.hr=js&a.hr",
    ] {
        assert!(!wcm.supports(&request(url)), "{url}");
    }
}

#[test]
fn new_profile_completeness_diagnostics_keep_warning_severity_and_exact_provenance() {
    for (manifest, url, code, level) in [
        (
            TD,
            "https://insight.adsrvr.org/track/pxl/?ct=0:synthetic&fmt=3",
            "vendor.the-trade-desk-static.param.adv.missing",
            RuleSourceLevel::OfficialVendor,
        ),
        (
            RTB,
            "https://ams.creativecdn.com/tags?type=iframe&su=&sr=&ts=0",
            "vendor.rtbhouse-gtm-tags.param.id.missing",
            RuleSourceLevel::OfficialTemplate,
        ),
        (
            WCM,
            "https://wcmvideotutorials.solution.weborama.fr/fcgi-bin/dispatch.fcgi?a.A=im&a.hr=js",
            "vendor.weborama-wcm-js-impression.param.a.si.missing",
            RuleSourceLevel::OfficialTemplate,
        ),
    ] {
        let pack = ManifestRulePack::from_json(manifest).unwrap();
        let report = pack.validate(&request(url));
        let finding = report.violations.iter().find(|f| f.code == code).unwrap();
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, level);
        assert!(
            finding
                .source
                .reference
                .as_deref()
                .is_some_and(|doc| doc.starts_with("https://"))
        );
    }
}
