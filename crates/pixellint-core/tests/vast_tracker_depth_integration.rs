use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};

#[test]
fn supported_tracking_routes_keep_core_privacy_errors_and_advisory_evidence() {
    let engine = Engine::default();
    for (url, pack, warning) in [
        (
            "https://track.adctv.com/?gdpr=NaN",
            "vendor/adctv-tracker",
            "vendor.adctv-tracker.param.event.missing",
        ),
        (
            "https://googleads4.g.doubleclick.net/pcs/view?xai=x&sai=y&gdpr=NaN",
            "vendor/google-cm360-pcs-view",
            "vendor.google-cm360-pcs-view.param.sig.missing",
        ),
        (
            "https://00px.net/pixel//e.gif?gdpr=NaN",
            "vendor/adxspace-pixel",
            "vendor.adxspace-pixel.param.creative_token.empty",
        ),
    ] {
        let summary = engine
            .validate(
                &ValidationRequest {
                    artifact_kind: ArtifactKind::VastTracker,
                    artifact: url.into(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Template,
                },
                &ValidationOptions::default(),
            )
            .unwrap();
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "core.privacy.gdpr_invalid" && v.severity == Severity::Error)
        );
        let report = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == pack)
            .unwrap();
        let finding = report
            .violations
            .iter()
            .find(|v| v.code == warning)
            .unwrap();
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, RuleSourceLevel::OfficialTemplate);
        assert!(!summary.is_ok());
    }
}

#[test]
fn disqo_attribution_does_not_claim_a_tracker_contract_or_lookalikes() {
    let engine = Engine::default();
    let entry = engine
        .directory()
        .lookup_host("track.activemetering.com")
        .unwrap();
    assert_eq!(entry.vendor, "disqo");
    assert!(entry.rulepack.is_none());
    assert!(
        engine
            .directory()
            .lookup_host("track.activemetering.com.invalid")
            .is_none()
    );
    assert!(
        engine
            .directory()
            .lookup_host("other.activemetering.com")
            .is_none()
    );
    let summary = engine
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::VastTracker,
                artifact: "https://track.activemetering.com/pixel/v1/all/pixel.gif?gdpr=NaN".into(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Template,
            },
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        summary
            .reports
            .iter()
            .all(|r| !r.plugin_id.starts_with("vendor/"))
    );
    assert!(
        summary
            .reports
            .iter()
            .any(|r| r.plugin_id == "directory" && r.detected_vendor.as_deref() == Some("disqo"))
    );
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code == "core.privacy.gdpr_invalid")
    );
}
