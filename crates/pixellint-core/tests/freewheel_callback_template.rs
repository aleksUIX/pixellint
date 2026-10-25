//! The SDK callback profile is separate from opaque callbacks and ad requests.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, ValidatorPlugin,
};

const PACK: &str = "vendor/freewheel-callback";
const MANIFEST: &str = include_str!("../rulepacks/vendor/freewheel-callback.json");

fn request(url: &str, kind: ArtifactKind, state: ExpansionState) -> ValidationRequest {
    ValidationRequest {
        artifact: url.to_owned(),
        artifact_kind: kind,
        claimed_vendor: None,
        expansion_state: state,
    }
}

#[test]
fn published_network_callbacks_select_in_url_vast_and_capture_forms() {
    let engine = Engine::default();
    // Sources and their hashes live in the review. Values here are synthetic.
    for host in ["g1.v.fwmrm.net", "demo.v.fwmrm.net", "5e529.v.fwmrm.net"] {
        let url = format!("https://{host}/ad/l/1?cn=defaultImpression&et=i&s=synthetic-opaque");
        let capture = serde_json::json!({"url":url,"method":"GET","headers":{}}).to_string();
        for state in [
            ExpansionState::Unknown,
            ExpansionState::Template,
            ExpansionState::Fired,
        ] {
            for (input, kind) in [
                (url.as_str(), ArtifactKind::Url),
                (url.as_str(), ArtifactKind::VastTracker),
                (capture.as_str(), ArtifactKind::NetworkRequest),
            ] {
                let summary = engine
                    .validate_at(
                        &request(input, kind, state),
                        &ValidationOptions::default(),
                        1792886400,
                    )
                    .unwrap();
                assert!(
                    summary
                        .reports
                        .iter()
                        .find(|r| r.plugin_id == PACK)
                        .unwrap()
                        .violations
                        .is_empty()
                );
                assert!(
                    !summary
                        .reports
                        .iter()
                        .any(|r| r.plugin_id == "vendor/freewheel" || r.plugin_id == "directory")
                );
            }
        }
    }
}

#[test]
fn explicit_profile_completeness_is_advisory_and_checks_empty_fields() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for (query, code) in [
        ("?cn=&et=i&s=x", "param.cn.empty"),
        ("?cn=x&et=&s=x", "param.et.empty"),
        ("?cn=x&et=i&s=", "param.s.empty"),
        ("?et=i&s=x", "param.cn.missing"),
        ("?cn=x&s=x", "param.et.missing"),
        ("?cn=x&et=i", "param.s.missing"),
        ("?cn=complete;et=i&s=x", "param.et.missing"),
    ] {
        let input = request(
            &format!("https://g1.v.fwmrm.net/ad/l/1{query}"),
            ArtifactKind::Url,
            ExpansionState::Unknown,
        );
        assert!(pack.supports(&input));
        let report = pack.validate(&input);
        assert_eq!(report.violations.len(), 1, "{query}");
        let finding = &report.violations[0];
        assert_eq!(finding.code, format!("vendor.freewheel-callback.{code}"));
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
    }
}

#[test]
fn profile_selection_uses_real_decoded_query_keys_and_exact_route_boundaries() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    assert!(pack.supports(&request(
        "https://g1.v.fwmrm.net/ad/l/1?%63n=x&%65t=i&s=x",
        ArtifactKind::Url,
        ExpansionState::Unknown
    )));
    for url in [
        "https://g1.v.fwmrm.net/ad/l/1?s=opaque",
        "https://g1.v.fwmrm.net/ad/l/1?s=x;cn=x;et=i",
        "https://g1.v.fwmrm.net/ad/l/1?s=x#cn=x&et=i",
        "https://g1.v.fwmrm.net/ad/l/1?cn_extra=x&et_extra=i&s=x",
        "https://g1.v.fwmrm.net/ad/g/1?cn=x&et=i&s=x",
        "https://g1.v.fwmrm.net/ad/l/1/?cn=x&et=i&s=x",
        "https://g1.v.fwmrm.net/ad/l/1/extra?cn=x&et=i&s=x",
        "https://g1.v.fwmrm.net/ad/l/10?cn=x&et=i&s=x",
        "https://g1.v.fwmrm.net.example.test/ad/l/1?cn=x&et=i&s=x",
        "https://example.test/ad/l/1?cn=x&et=i&s=x",
    ] {
        assert!(
            !pack.supports(&request(url, ArtifactKind::Url, ExpansionState::Unknown)),
            "{url}"
        );
    }
}

#[test]
fn events_tokens_and_unreviewed_identifiers_remain_open_and_core_checks_continue() {
    let engine = Engine::default();
    let url = "https://g1.v.fwmrm.net/ad/l/1?cn=future-event&et=future-type&s=not-a-signature!&adid=not-a-number&reid=&t=not-a-time";
    let report = engine
        .validate(
            &request(url, ArtifactKind::Url, ExpansionState::Unknown),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        report
            .reports
            .iter()
            .find(|r| r.plugin_id == PACK)
            .unwrap()
            .violations
            .is_empty()
    );
    for url in [
        "https://g1.v.fwmrm.net/ad/l/1?cn=x&et=i&s=x&gdpr=maybe",
        "https://g1.v.fwmrm.net/ad/l/1?s=opaque&gdpr=maybe",
    ] {
        let summary = engine
            .validate(
                &request(url, ArtifactKind::VastTracker, ExpansionState::Template),
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(summary.reports.iter().any(|r| {
            r.plugin_id == "core"
                && r.violations
                    .iter()
                    .any(|v| v.code == "core.privacy.gdpr_invalid" && v.severity == Severity::Error)
        }));
        assert!(!summary.is_ok());
    }
}
