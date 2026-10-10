//! Tester followups preserve destination requirements and accepted privacy controls.

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde_json::{Value, json};

const HASH: &str = "a85e9ca18f34935ab9b0381b25bfad2455444112b0149270fd88e3da172fe196";

fn error_codes(engine: &Engine, vendor: &str, kind: ArtifactKind, artifact: String) -> Vec<String> {
    let request = ValidationRequest {
        artifact_kind: kind,
        artifact,
        claimed_vendor: Some(vendor.to_owned()),
        expansion_state: ExpansionState::Unknown,
    };
    let options = ValidationOptions {
        only_rulepacks: vec![format!("vendor/{vendor}")],
        except_rulepacks: Vec::new(),
    };
    let result = engine
        .validate_at(&request, &options, 1_770_000_000)
        .unwrap();
    assert_eq!(result.reports.len(), 1);
    assert_eq!(result.reports[0].plugin_id, format!("vendor/{vendor}"));
    let mut codes: Vec<_> = result
        .reports
        .iter()
        .flat_map(|report| &report.violations)
        .filter(|violation| violation.severity == Severity::Error)
        .map(|violation| violation.code.clone())
        .collect();
    codes.sort();
    codes
}

#[test]
fn appsflyer_event_value_is_required_but_an_empty_string_is_valid() {
    // https://dev.appsflyer.com/hc/reference/s2s-events-api3-overview
    let engine = Engine::default();
    for (value, expected) in [
        (None, vec!["vendor.appsflyer.body.eventValue.missing"]),
        (Some(json!("")), vec![]),
        (Some(json!("{}")), vec![]),
        (
            Some(Value::Null),
            vec!["vendor.appsflyer.body.eventValue.invalid"],
        ),
        (
            Some(json!({})),
            vec!["vendor.appsflyer.body.eventValue.invalid"],
        ),
    ] {
        let mut payload = json!({"appsflyer_id":"install-123", "eventName":"af_purchase"});
        if let Some(value) = value {
            payload["eventValue"] = value;
        }
        assert_eq!(
            error_codes(
                &engine,
                "appsflyer",
                ArtifactKind::JsonPayload,
                payload.to_string()
            ),
            expected,
            "payload {payload}"
        );
    }
}

#[test]
fn adjust_ip_is_optional_but_populated_values_need_raw_ipv4() {
    // https://dev.adjust.com/en/api/s2s-api/events/
    let engine = Engine::default();
    for (ip, expected) in [
        (None, vec![]),
        (Some("192.0.2.1"), vec![]),
        (
            Some("2001:db8::1"),
            vec!["vendor.adjust.param.ip_address.invalid"],
        ),
        (
            Some(HASH),
            vec![
                "vendor.adjust.hashed_plaintext_field",
                "vendor.adjust.param.ip_address.invalid",
            ],
        ),
    ] {
        let mut artifact = "https://s2s.adjust.com/event?app_token=testapp&event_token=testevent&s2s=1&adid=18546f6171f67e29d1cb983322ad1329".to_owned();
        if let Some(ip) = ip {
            artifact.push_str("&ip_address=");
            artifact.push_str(ip);
        }
        assert_eq!(
            error_codes(&engine, "adjust", ArtifactKind::Url, artifact),
            expected,
            "IP {ip:?}"
        );
    }
}

#[test]
fn posthog_accepts_null_optional_values_and_hash_shaped_identifiers() {
    // URL redaction: https://posthog.com/docs/privacy/data-collection
    // Nullable event properties: https://github.com/PostHog/posthog/blob/d0c3c16556500062c29489d312e97862d240bc70/rust/common/types/src/event.rs
    let engine = Engine::default();
    for event in [
        json!({"event":"$pageview", "distinct_id":"person", "properties":{"$current_url":null}}),
        json!({"event":"user signed up", "distinct_id":"person", "properties":{"$ip":null}}),
        json!({"event":"user signed up", "distinct_id":HASH, "properties":{"email_hash":HASH,"custom_digest":HASH,"$ip":"2001:db8::1"}}),
        json!({"event":"user signed up", "properties":{"distinct_id":HASH,"email_hash":HASH}}),
    ] {
        let mut single = event.clone();
        single["api_key"] = json!("phc_fixture");
        assert!(
            error_codes(
                &engine,
                "posthog",
                ArtifactKind::JsonPayload,
                single.to_string()
            )
            .is_empty(),
            "single {single}"
        );
        let batch = json!({"api_key":"phc_fixture", "batch":[event]});
        assert!(
            error_codes(
                &engine,
                "posthog",
                ArtifactKind::JsonPayload,
                batch.to_string()
            )
            .is_empty(),
            "batch {batch}"
        );
    }

    for (property, value, expected) in [
        (
            "$ip",
            json!(HASH),
            vec![
                "vendor.posthog.body.hashed_plaintext_field",
                "vendor.posthog.body.properties.$ip.invalid",
            ],
        ),
        (
            "$ip",
            json!([]),
            vec!["vendor.posthog.body.properties.$ip.invalid"],
        ),
        (
            "$current_url",
            json!([]),
            vec!["vendor.posthog.body.properties.$current_url.invalid"],
        ),
    ] {
        let event = json!({"event":"user signed up", "distinct_id":"person", "properties":{property:value}});
        let batch = json!({"api_key":"phc_fixture", "batch":[
            {"event":"$pageview","distinct_id":HASH,"properties":{"$current_url":null,"$ip":null}},
            event
        ]});
        assert_eq!(
            error_codes(
                &engine,
                "posthog",
                ArtifactKind::JsonPayload,
                batch.to_string()
            ),
            expected,
            "batch {batch}"
        );
    }
}
