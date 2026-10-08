//! Vendor boundary and regression checks for conversion payload contracts.
use pixellint_core::{ArtifactKind, Engine, ValidationOptions, ValidationRequest};
use serde_json::{Value, json};

fn codes(pack: &str, payload: Value) -> Vec<String> {
    Engine::default()
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::JsonPayload,
                artifact: payload.to_string(),
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions {
                only_rulepacks: vec![format!("vendor/{pack}")],
                except_rulepacks: vec![],
            },
            1770000060,
        )
        .unwrap()
        .reports
        .into_iter()
        .flat_map(|r| r.violations.into_iter().map(|v| v.code))
        .collect()
}

#[test]
fn openai_ignored_identifier_entries_do_not_reject_events() {
    let engine = Engine::default();
    let options = ValidationOptions {
        only_rulepacks: vec!["vendor/openai-conversions-api".into()],
        except_rulepacks: vec![],
    };
    for values in [
        json!(["a".repeat(64), "b".repeat(64), "c".repeat(64), 123]),
        json!([
            123,
            "a".repeat(64),
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64)
        ]),
    ] {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::JsonPayload,
            artifact: json!({"events":[{"id":"identity-test","type":"page_viewed",
                "timestamp_ms":1770000000000u64,"data":{"type":"contents"},
                "user":{"emails_sha256":values}}]})
            .to_string(),
            claimed_vendor: None,
            expansion_state: Default::default(),
        };
        let summary = engine.validate_at(&request, &options, 1770000060).unwrap();
        assert!(
            summary.is_ok(),
            "ignored list entries must not reject ingestion"
        );
        assert!(
            summary.reports.iter().flat_map(|r| &r.violations).any(
                |v| v.code == "vendor.openai-conversions-api.body.user.emails_sha256[].invalid"
            )
        );
    }
}

#[test]
fn documented_batch_limits_use_inclusive_boundaries() {
    let linked = json!({"conversion":"urn:lla:llaPartnerConversion:123", "conversionHappenedAt":1770000000000u64,
        "user":{"userIds":[],"lead":"urn:li:leadGenFormResponse:test"}});
    for (pack, envelope, event, limit, code) in [
        (
            "linkedin-conversions-api",
            "elements",
            linked,
            5000,
            "vendor.linkedin-conversions-api.body.elements.invalid",
        ),
        (
            "microsoft-conversions-api",
            "data",
            json!({"eventType":"custom","eventTime":1770000000,
            "userData":{"anonymousId":"customer"}}),
            1000,
            "vendor.microsoft-conversions-api.body.data.invalid",
        ),
        (
            "pinterest-conversions-api",
            "data",
            json!({"event_name":"page_visit","action_source":"web","event_id":"event",
            "event_time":1770000000,"user_data":{"em":["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]}}),
            1000,
            "vendor.pinterest-conversions-api.body.data.invalid",
        ),
        (
            "openai-conversions-api",
            "events",
            json!({"id":"event","type":"page_viewed","timestamp_ms":1770000000000u64,
            "data":{"type":"contents"}}),
            1000,
            "vendor.openai-conversions-api.body.events.invalid",
        ),
    ] {
        let payload = |count| {
            let mut value = json!({});
            value[envelope] = Value::Array(vec![event.clone(); count]);
            value
        };
        assert!(
            !codes(pack, payload(limit))
                .iter()
                .any(|found| found == code),
            "{pack} accepts the published limit"
        );
        assert!(
            codes(pack, payload(limit + 1))
                .iter()
                .any(|found| found == code),
            "{pack} rejects the first excess item"
        );
    }
}

#[test]
fn uet_loader_does_not_run_collection_parameter_contracts() {
    let summary = Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::Url,
                artifact: "https://bat.bing.com/bat.js".to_owned(),
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(
        !summary
            .reports
            .iter()
            .any(|report| report.plugin_id == "vendor/microsoft-uet")
    );
}
