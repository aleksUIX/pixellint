use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest,
};

const PACK: &str = "vendor/innovid-video-progress";
const SOURCE: &str = "https://static.innovid.com/mobileapps/js/vpaid/1h41kg";

#[test]
fn progress_advice_is_source_scoped_for_vast_and_captured_requests() {
    let engine = Engine::default();
    let url = "https://s.innovid.com/1x1.gif?action=vpoint&event_id=percent&event_value=101";
    let capture = serde_json::json!({"url": url, "method": "GET", "headers": {}}).to_string();

    for (artifact_kind, artifact) in [
        (ArtifactKind::VastTracker, url),
        (ArtifactKind::NetworkRequest, capture.as_str()),
    ] {
        let result = engine
            .validate(
                &ValidationRequest {
                    artifact_kind,
                    artifact: artifact.to_owned(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            result.coverage().status,
            EndpointCoverageStatus::VendorChecked
        );
        assert_eq!(result.coverage().vendor_rulepacks, [PACK]);
        assert!(result.is_ok());
        let findings = &result
            .reports
            .iter()
            .find(|report| report.plugin_id == PACK)
            .unwrap()
            .violations;
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].code,
            "vendor.innovid-video-progress.param.event_value.invalid"
        );
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].source.level, RuleSourceLevel::OfficialTemplate);
        assert_eq!(findings[0].source.reference.as_deref(), Some(SOURCE));
    }
}
