//! The source supports a narrow query producer, not every Naver transport.
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, ValidatorPlugin,
};
const MANIFEST: &str = include_str!("../rulepacks/vendor/naver-wcs-query.json");
const PACK: &str = "vendor/naver-wcs-query";
fn request(url: &str, kind: ArtifactKind) -> ValidationRequest {
    ValidationRequest {
        artifact: url.to_owned(),
        artifact_kind: kind,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}
#[test]
fn source_pageview_and_event_query_profiles_work_with_optional_legacy_identifiers() {
    let engine = Engine::default();
    for query in [
        "?u=https%3A%2F%2Fshop.example%2F&vs=0.8.32&nt=synthetic&wa=synthetic-account&e=",
        "?u=about%3Ablank&vs=vNext&nt=not-a-clock&i=legacy-id",
        "?u=about%3Ablank&vs=vNext&nt=not-a-clock&t=event&e_cat=x&e_act=y",
    ] {
        let url = format!("https://wcs.naver.com/m{query}");
        let capture = serde_json::json!({"url":url,"method":"GET","headers":{}}).to_string();
        for (input, kind) in [
            (url.as_str(), ArtifactKind::Url),
            (capture.as_str(), ArtifactKind::NetworkRequest),
        ] {
            let summary = engine
                .validate(&request(input, kind), &ValidationOptions::default())
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
        }
    }
}
#[test]
fn missing_generated_slots_and_submitted_empty_account_warn_only() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for (query, code) in [
        ("?vs=x&nt=y", "param.u.missing"),
        ("?u=about%3Ablank&nt=x", "param.vs.missing"),
        ("?u=about%3Ablank&vs=x", "param.nt.missing"),
        ("?u=&vs=x&nt=y", "param.u.empty"),
        ("?u=about%3Ablank&vs=&nt=y", "param.vs.empty"),
        ("?u=about%3Ablank&vs=x&nt=", "param.nt.empty"),
        ("?u=about%3Ablank&vs=x&nt=y&wa=", "param.wa.empty"),
    ] {
        let report = pack.validate(&request(
            &format!("https://wcs.naver.com/m{query}"),
            ArtifactKind::Url,
        ));
        assert_eq!(report.violations.len(), 1, "{query}");
        let finding = &report.violations[0];
        assert_eq!(finding.code, format!("vendor.naver-wcs-query.{code}"));
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
    }
}
#[test]
fn loaders_body_routes_configurable_hosts_and_nearby_keys_remain_outside_query_scope() {
    let pack = ManifestRulePack::from_json(MANIFEST).unwrap();
    for url in [
        "https://wcs.naver.net/wcslog.js",
        "https://wcs.naver.com/b?u=x&vs=x&nt=x",
        "https://wcs.naver.net/m?u=x&vs=x&nt=x",
        "https://wcs.naver.com/m?wa=synthetic",
        "https://wcs.naver.com/m?url=x&vs_extra=x&nt_extra=x",
        "https://wcs.naver.com/m/?u=x&vs=x&nt=x",
        "https://wcs.naver.com/m/extra?u=x&vs=x&nt=x",
        "https://wcs.naver.com.example.test/m?u=x&vs=x&nt=x",
    ] {
        assert!(!pack.supports(&request(url, ArtifactKind::Url)), "{url}");
    }
}
#[test]
fn captured_query_method_warns_and_core_privacy_checks_still_apply() {
    let engine = Engine::default();
    let capture=serde_json::json!({"url":"https://wcs.naver.com/m?u=x&vs=x&nt=x","method":"POST","headers":{}}).to_string();
    let summary = engine
        .validate(
            &request(&capture, ArtifactKind::NetworkRequest),
            &ValidationOptions::default(),
        )
        .unwrap();
    let finding = &summary
        .reports
        .iter()
        .find(|r| r.plugin_id == PACK)
        .unwrap()
        .violations[0];
    assert_eq!(finding.code, "vendor.naver-wcs-query.http.method.invalid");
    assert_eq!(finding.severity, Severity::Warning);
    let summary = engine
        .validate(
            &request(
                "https://wcs.naver.com/m?u=x&vs=x&nt=x&gdpr=maybe",
                ArtifactKind::Url,
            ),
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
