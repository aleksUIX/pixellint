//! Source-scoped Tapad template advice leaves other clients and core checks intact.

use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ExpansionState, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest,
};

#[test]
fn encoded_legacy_map_is_checked_on_url_vast_and_request_inputs() {
    let engine = Engine::default();
    let url = "https://tapestry.tapad.com/tapestry/1?ta_typed_did=%7B%22custom%22%3A3%7D";
    let capture = serde_json::json!({"url": url, "method": "GET", "headers": []}).to_string();
    for (kind, artifact) in [
        (ArtifactKind::Url, url),
        (ArtifactKind::VastTracker, url),
        (ArtifactKind::NetworkRequest, capture.as_str()),
    ] {
        let summary = engine
            .validate_at(
                &ValidationRequest {
                    artifact_kind: kind,
                    artifact: artifact.to_owned(),
                    claimed_vendor: None,
                    expansion_state: ExpansionState::Unknown,
                },
                &ValidationOptions::default(),
                1_792_886_400,
            )
            .unwrap();
        assert_eq!(
            summary.coverage.status,
            EndpointCoverageStatus::VendorChecked
        );
        let report = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == "vendor/tapad-tapestry")
            .unwrap();
        assert!(!report.violations.is_empty());
        assert!(report.violations.iter().all(|finding| {
            finding.severity == Severity::Warning
                && finding.source.level == RuleSourceLevel::OfficialTemplate
        }));
        assert!(summary.is_ok());
    }
}

#[test]
fn opaque_optional_ids_and_core_privacy_keep_their_existing_meaning() {
    let engine = Engine::default();
    let summary = engine
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::Url,
                artifact: "https://tapestry.tapad.com/tapestry/1?ta_typed_did=%7B%22future-id-type%22%3A%22%22%7D&ta_partner_id=&ta_guid=unknown-profile-field&gdpr=NaN".to_owned(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
        )
        .unwrap();
    let vendor = summary
        .reports
        .iter()
        .find(|report| report.plugin_id == "vendor/tapad-tapestry")
        .unwrap();
    assert!(vendor.violations.is_empty());
    assert!(summary.reports.iter().any(|report| {
        report.plugin_id == "core"
            && report.violations.iter().any(|finding| {
                finding.code == "core.privacy.gdpr_invalid" && finding.severity == Severity::Error
            })
    }));
}
