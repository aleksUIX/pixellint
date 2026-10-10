//! Published request record limits, independently of serialized byte quotas.

use base64::Engine as _;
use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde_json::{Value, json};

fn batch(pack: &str, count: usize) -> Value {
    let records: Vec<_> = (0..count)
        .map(|index| match pack {
            "mixpanel" => json!({
                "event": "contract_boundary",
                "properties": {"token": "LOCAL_TEST_KEY", "distinct_id": format!("user-{index}"), "$insert_id": format!("event-{index}")}
            }),
            "segment" => json!({
                "type": "track", "event": "contract_boundary", "userId": format!("user-{index}")
            }),
            _ => unreachable!(),
        })
        .collect();
    match pack {
        "mixpanel" => json!(records),
        "segment" => json!({"writeKey": "LOCAL_TEST_KEY", "batch": records}),
        _ => unreachable!(),
    }
}

fn findings(pack: &str, body: Value, capture: bool) -> Vec<(String, Severity)> {
    let (artifact_kind, artifact) = if capture {
        let url = match pack {
            "mixpanel" => "https://api.mixpanel.com/track",
            "segment" => "https://api.segment.io/v1/batch",
            _ => unreachable!(),
        };
        (
            ArtifactKind::NetworkRequest,
            json!({"url": url, "method": "POST", "headers": {"Content-Type": "application/json"}, "body": body.to_string()}).to_string(),
        )
    } else {
        (ArtifactKind::JsonPayload, body.to_string())
    };
    Engine::default()
        .validate_at(
            &ValidationRequest {
                artifact_kind,
                artifact,
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
            },
            &ValidationOptions {
                only_rulepacks: vec![format!("vendor/{pack}")],
                except_rulepacks: vec![],
            },
            1_791_600_000,
        )
        .unwrap()
        .reports
        .into_iter()
        .flat_map(|report| report.violations)
        .map(|finding| (finding.code, finding.severity))
        .collect()
}

#[test]
fn mixpanel_track_counts_records_at_the_request_root() {
    for capture in [false, true] {
        assert!(findings("mixpanel", batch("mixpanel", 2000), capture).is_empty());
        assert_eq!(
            findings("mixpanel", batch("mixpanel", 2001), capture),
            vec![("vendor.mixpanel.body.root.invalid".into(), Severity::Error)]
        );
    }
}

#[test]
fn mixpanel_data_transports_count_decoded_arrays_and_preserve_single_objects() {
    let engine = Engine::default();
    for base64 in [false, true] {
        for count in [1, 2000, 2001] {
            let body = batch("mixpanel", count);
            let objects = if count == 1 {
                vec![body.clone(), body[0].clone()]
            } else {
                vec![body]
            };
            for body in objects {
                let json = body.to_string();
                let encoded = if base64 {
                    base64::engine::general_purpose::STANDARD.encode(json)
                } else {
                    json
                };
                let data: String =
                    url::form_urlencoded::byte_serialize(encoded.as_bytes()).collect();
                let summary = engine
                    .validate_at(
                        &ValidationRequest {
                            artifact_kind: ArtifactKind::Url,
                            artifact: format!("https://api.mixpanel.com/track?data={data}"),
                            claimed_vendor: None,
                            expansion_state: ExpansionState::Unknown,
                        },
                        &ValidationOptions {
                            only_rulepacks: vec!["vendor/mixpanel".into()],
                            except_rulepacks: vec![],
                        },
                        1_791_600_000,
                    )
                    .unwrap();
                let codes: Vec<_> = summary
                    .reports
                    .iter()
                    .flat_map(|report| &report.violations)
                    .map(|finding| finding.code.as_str())
                    .collect();
                assert_eq!(
                    codes,
                    if count > 2000 {
                        vec!["vendor.mixpanel.body.root.invalid"]
                    } else {
                        vec![]
                    }
                );
            }
        }
    }
}

#[test]
fn segment_batch_counts_records_once_across_supported_json_inputs() {
    for capture in [false, true] {
        assert!(findings("segment", batch("segment", 2500), capture).is_empty());
        assert_eq!(
            findings("segment", batch("segment", 2501), capture),
            vec![("vendor.segment.body.batch.invalid".into(), Severity::Error)]
        );
    }
}

#[test]
fn oversized_batches_retain_member_checks_and_nested_arrays_are_not_batches() {
    for (pack, limit, cap_code) in [
        ("mixpanel", 2000, "vendor.mixpanel.body.root.invalid"),
        ("segment", 2500, "vendor.segment.body.batch.invalid"),
    ] {
        let mut body = batch(pack, limit + 1);
        let records = if pack == "mixpanel" {
            body.as_array_mut().unwrap()
        } else {
            body["batch"].as_array_mut().unwrap()
        };
        records.last_mut().unwrap()["event"] = json!("");
        let codes = findings(pack, body, false);
        assert_eq!(codes.iter().filter(|(code, _)| code == cap_code).count(), 1);
        assert!(
            codes
                .iter()
                .any(|(code, _)| code.ends_with("body.event.empty"))
        );
    }
    let mut single = batch("segment", 1);
    single["batch"][0]["properties"] = json!({"items": vec!["product"; 2501]});
    assert!(findings("segment", single, false).is_empty());
}
