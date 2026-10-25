use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};
use serde_json::json;

fn capture(url: &str, method: &str, body: Option<&str>) -> ValidationRequest {
    let mut artifact = json!({"url": url, "method": method, "headers": {}});
    if let Some(body) = body {
        artifact["body"] = json!(body);
    }
    ValidationRequest {
        artifact: artifact.to_string(),
        artifact_kind: ArtifactKind::NetworkRequest,
        expansion_state: ExpansionState::Unknown,
        claimed_vendor: None,
    }
}

#[test]
fn exact_primary_template_substitutions_remain_clean_without_a_body_schema() {
    let engine = Engine::default();
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-sourcepoint-gdpr-choice/source-cases.json"
    ))
    .unwrap();
    assert_eq!(source["engine_derived"], false);
    for case in source["cases"].as_array().unwrap() {
        let request = ValidationRequest {
            artifact: case["capture"].to_string(),
            artifact_kind: ArtifactKind::NetworkRequest,
            expansion_state: ExpansionState::Unknown,
            claimed_vendor: None,
        };
        let summary = engine
            .validate_at(&request, &ValidationOptions::default(), 1792886400)
            .unwrap();
        assert_eq!(summary.reports.len(), 2);
        let report = &summary.reports[1];
        assert_eq!(report.plugin_id, "vendor/sourcepoint-gdpr-choice");
        assert!(report.violations.is_empty(), "{}", case["id"]);
    }
}

#[test]
fn literal_env_occurrences_are_checked_once_with_vendor_provenance() {
    let request = capture(
        "https://cdn.privacy-mgmt.com/wrapper/v2/messages/choice/gdpr/future?requestUUID=opaque&hasCsp=other&env=prod&%65nv=public",
        "POST",
        None,
    );
    let summary = Engine::default()
        .validate_at(&request, &ValidationOptions::default(), 1792886400)
        .unwrap();
    let report = &summary.reports[1];
    assert_eq!(report.violations.len(), 1);
    let finding = &report.violations[0];
    assert_eq!(
        finding.code,
        "vendor.sourcepoint-gdpr-choice.param.env.invalid"
    );
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.source.level, RuleSourceLevel::OfficialVendor);
}

#[test]
fn identifiers_callback_members_and_has_csp_meaning_stay_open() {
    let engine = Engine::default();
    for body in [
        "null",
        "[]",
        r#"{"message_type":false,"choice_id":null,"pmData":[]}"#,
        "opaque-body",
    ] {
        let request = capture(
            "https://cdn.privacy-mgmt.com/wrapper/v2/messages/choice/gdpr/future-action?requestUUID=&hasCsp=not-a-boolean&env=",
            "POST",
            Some(body),
        );
        let summary = engine
            .validate_at(&request, &ValidationOptions::default(), 1792886400)
            .unwrap();
        assert!(summary.reports[1].violations.is_empty(), "{body}");
    }
}

#[test]
fn inventory_pointer_does_not_select_current_sdk_routes_or_bare_urls() {
    let engine = Engine::default();
    let options = ValidationOptions::default();
    for url in [
        "https://cdn.privacy-mgmt.com/wrapper/v2/choice/gdpr/11",
        "https://cdn.privacy-mgmt.com/wrapper/v2/messages/choice/usnat/11",
        "https://cdn.privacy-mgmt.com/wrapper/v2/messages/choice/gdpr/11/extra",
    ] {
        let summary = engine
            .validate_at(&capture(url, "POST", None), &options, 1792886400)
            .unwrap();
        assert_eq!(summary.reports[1].plugin_id, "directory");
        assert_eq!(
            summary.reports[1].violations[0].code,
            "directory.no_rulepack_coverage"
        );
    }
    let request = ValidationRequest {
        artifact: "https://cdn.privacy-mgmt.com/wrapper/v2/messages/choice/gdpr/11?requestUUID=opaque&env=prod".into(),
        artifact_kind: ArtifactKind::Url,
        expansion_state: ExpansionState::Unknown,
        claimed_vendor: None,
    };
    let summary = engine.validate_at(&request, &options, 1792886400).unwrap();
    assert_eq!(summary.reports[1].plugin_id, "directory");
}
