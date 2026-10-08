//! Bare JSON automatic routing needs vendor-specific Adobe evidence.

use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};

fn request(artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::JsonPayload,
        artifact: artifact.to_string(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

#[test]
fn openai_conversion_data_does_not_claim_adobe() {
    let artifact = include_str!("../../../fixtures/vendor-openai-conversions-api/clean-event.txt");
    let summary = Engine::default()
        .validate_at(
            &request(artifact),
            &ValidationOptions::default(),
            1770000000,
        )
        .unwrap();
    let ids: Vec<_> = summary
        .reports
        .iter()
        .map(|report| report.plugin_id.as_str())
        .collect();
    assert!(ids.contains(&"vendor/openai-conversions-api"), "{ids:?}");
    assert!(!ids.contains(&"vendor/adobe-web-sdk"), "{ids:?}");
    assert!(summary.is_ok(), "{:?}", summary.reports);
}

#[test]
fn adobe_xdm_and_published_edge_state_identify_automatic_payloads() {
    // The SDK injects this state envelope into a data-only event batch.
    // Sources: Alloy createCookieTransfer and createDataCollectionRequestPayload.
    for artifact in [
        include_str!("../../../fixtures/vendor-adobe-web-sdk/clean-collect.txt"),
        r#"{"events":[{"data":{"custom":"value"}}],"meta":{"state":{"cookiesEnabled":false}}}"#,
        r#"{"event":{"data":{"custom":"value"}}}"#,
    ] {
        let summary = Engine::default()
            .validate(&request(artifact), &ValidationOptions::default())
            .unwrap();
        assert!(
            summary
                .reports
                .iter()
                .any(|report| report.plugin_id == "vendor/adobe-web-sdk")
        );
        let adobe = summary
            .reports
            .iter()
            .find(|report| report.plugin_id == "vendor/adobe-web-sdk")
            .unwrap();
        assert!(adobe.violations.is_empty(), "{adobe:?}");
    }
}

#[test]
fn ambiguous_data_only_batches_remain_available_to_explicit_validation() {
    let artifact = r#"{"events":[{"data":{"custom":"value"}}]}"#;
    let engine = Engine::default();
    let automatic = engine
        .validate(&request(artifact), &ValidationOptions::default())
        .unwrap();
    assert!(
        !automatic
            .reports
            .iter()
            .any(|report| report.plugin_id == "vendor/adobe-web-sdk")
    );
    let selected = engine
        .validate(
            &request(artifact),
            &ValidationOptions {
                only_rulepacks: vec!["vendor/adobe-web-sdk".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert!(
        selected
            .reports
            .iter()
            .any(|report| report.plugin_id == "vendor/adobe-web-sdk")
    );
    assert!(selected.is_ok(), "{:?}", selected.reports);
    let malformed = r#"{"events":[{"data":false}]}"#;
    let selected = engine
        .validate(
            &request(malformed),
            &ValidationOptions {
                only_rulepacks: vec!["vendor/adobe-web-sdk".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert!(
        selected
            .reports
            .iter()
            .flat_map(|report| &report.violations)
            .any(|violation| violation.code == "vendor.adobe-web-sdk.body.data.invalid")
    );
}
