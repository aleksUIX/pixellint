use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};
use serde_json::json;

fn validate(artifact: String, kind: ArtifactKind) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

#[test]
fn ispot_default_event_is_valid_on_both_documented_conversion_hosts() {
    for host in ["pt.ispot.tv", "staging-pt.ispot.tv"] {
        let result = validate(
            format!("https://{host}/v2/TC-1234-1.gif"),
            ArtifactKind::Url,
        );
        assert!(result.is_ok());
        assert_eq!(
            result.coverage().vendor_rulepacks,
            ["vendor/ispot-conversion"]
        );
        assert!(result.reports.iter().all(|r| r.violations.is_empty()));
    }
}

#[test]
fn ispot_amount_advice_retains_url_targets_across_tracker_and_postback_inputs() {
    let url = "https://pt.ispot.tv/v2/TC-1234-1.gif?amount=money";
    for kind in [
        ArtifactKind::Url,
        ArtifactKind::VastTracker,
        ArtifactKind::ServerPostback,
    ] {
        let result = validate(url.into(), kind);
        assert!(result.is_ok());
        let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].code,
            "vendor.ispot-conversion.param.amount.invalid"
        );
        assert!(
            findings[0]
                .targets
                .iter()
                .any(|t| t.name.as_deref() == Some("amount"))
        );
    }
}

#[test]
fn browser_and_query_ip_requests_do_not_inherit_server_header_requiredness() {
    for extra in ["", "?ip=192.0.2.1", "?ip=2001:db8::1"] {
        let result = validate(
            json!({"url":format!("https://pt.ispot.tv/v2/TC-1234-1.gif{extra}"),"method":"GET","headers":{}}).to_string(),
            ArtifactKind::NetworkRequest,
        );
        assert!(result.reports.iter().all(|r| r.violations.is_empty()));
    }
    let result = validate(
        json!({"url":"https://pt.ispot.tv/v2/TC-1234-1.gif","method":"GET","headers":{"X-Forwarded-For":"not-ip"}}).to_string(),
        ArtifactKind::NetworkRequest,
    );
    let finding = result
        .reports
        .iter()
        .flat_map(|r| &r.violations)
        .next()
        .unwrap();
    assert_eq!(
        finding.code,
        "vendor.ispot-conversion.http.headers.x-forwarded-for.invalid"
    );
    assert!(finding.targets.is_empty());
}

#[test]
fn neighboring_ispot_assets_remain_explicitly_uncovered() {
    for host in ["pt.ispot.tv", "pi.ispot.tv"] {
        for path in [
            "/v2/TC-1234-1.gif-extra",
            "/v2/TC-1234-1.gif/other",
            "/v2/loader.js",
        ] {
            let result = validate(format!("https://{host}{path}"), ArtifactKind::Url);
            assert!(result.coverage().vendor_rulepacks.is_empty());
            assert!(
                result
                    .reports
                    .iter()
                    .flat_map(|r| &r.violations)
                    .any(|v| v.code == "directory.no_rulepack_coverage")
            );
        }
    }
}
