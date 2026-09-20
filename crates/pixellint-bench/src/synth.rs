use pixellint_core::{ArtifactKind, ExpansionState};
use serde_json::{Value, json};

use crate::case::Case;

const HASHED_EMAIL: &str = "a85e9ca18f34935ab9b0381b25bfad2455444112b0149270fd88e3da172fe196";
const HASHED_PHONE: &str = "d6736136ea896c1bfdc553e0e86e702c70d060d805696ca3e4e9e0961353860a";

pub fn synth_cases() -> Vec<Case> {
    let mut cases = vec![
        url(
            "d1-meta-pageview",
            "https://www.facebook.com/tr?id=1234567890123456&ev=PageView&noscript=1",
        ),
        url(
            "d1-meta-purchase-unhashed",
            "https://www.facebook.com/tr?id=1234567890123456&ev=Purchase&em=buyer@example.com&cd[value]=129.99&cd[currency]=USD",
        ),
        url(
            "d1-ga4-collect",
            "https://www.google-analytics.com/g/collect?v=2&tid=G-ABC1234&cid=1234567890.1234567890&en=page_view",
        ),
        url(
            "d1-floodlight",
            "https://ad.doubleclick.net/ddm/activity/src=1234567;type=convr0;cat=purch0;ord=8675309?",
        ),
        url(
            "d1-linkedin",
            "https://px.ads.linkedin.com/collect?pid=123456&conversionId=7890123&fmt=gif",
        ),
        url(
            "d1-tiktok-loader",
            "https://analytics.tiktok.com/i18n/pixel/events.js?sdkid=CBBQ1234567890&lib=ttq",
        ),
        url(
            "d1-cm360-vast",
            "https://ade.googlesyndication.com/ddm/activity/dc_oe=ChMIexamplepayload;met=1;",
        ),
        url(
            "d1-ias-display",
            "https://pixel.adsafeprotected.com/rjss/st/12345/67890/skeleton.js",
        ),
        url(
            "d1-doubleverify-visit",
            "https://tps.doubleverify.com/visit.jpg?ctx=12345&cmp=12345&plc=12345&sid=12345",
        ),
        url(
            "d1-unknown-tracker",
            "https://trk.cdn.example.net/pixel.gif?id=42&r=8675309",
        ),
        Case {
            name: "d1-template-macros".to_string(),
            kind: ArtifactKind::Url,
            artifact: "https://www.facebook.com/tr?id=[PIXEL_ID]&ev=Purchase&cd[value]=[VALUE]&cd[currency]=USD&ord=[CACHEBUSTING]".to_string(),
            expansion_state: ExpansionState::Template,
            claimed_vendor: None,
        },
        json("d1-meta-capi", meta_capi_batch(1)),
        json("d1-tiktok-capi", tiktok_capi_event("order-10492")),
        json("synth-capi-batch-10", meta_capi_batch(10)),
        json("synth-capi-batch-100", meta_capi_batch(100)),
        json("synth-capi-batch-1000", meta_capi_batch(1000)),
        json("synth-capi-batch-1000-dirty", meta_capi_batch_dirty(1000)),
        url("synth-fat-query-100", fat_query(100)),
        url("synth-fat-query-1000", fat_query(1000)),
        url("synth-fat-query-5000", fat_query(5000)),
        url("synth-url-8kb", padded_url(8 * 1024)),
        url("synth-url-64kb", padded_url(64 * 1024)),
        url(
            "synth-unknown-long-tail",
            "https://px-east.internal-tracker.example/v2/collect?aid=99&sid=session-1&cb=1",
        ),
    ];
    cases.sort_by_key(|case| case.bytes());
    cases
}

fn url(name: &str, artifact: impl Into<String>) -> Case {
    Case {
        name: name.to_string(),
        kind: ArtifactKind::Url,
        artifact: artifact.into(),
        expansion_state: ExpansionState::Unknown,
        claimed_vendor: None,
    }
}

fn json(name: &str, artifact: String) -> Case {
    Case {
        name: name.to_string(),
        kind: ArtifactKind::JsonPayload,
        artifact,
        expansion_state: ExpansionState::Unknown,
        claimed_vendor: None,
    }
}

fn meta_capi_event(index: usize, dirty: bool) -> Value {
    let purchase = index.is_multiple_of(5);
    let event_time = if dirty && index.is_multiple_of(11) {
        1_770_000_000_000u64
    } else {
        1_770_000_000
    };
    let email = if dirty && index.is_multiple_of(17) {
        json!("buyer@example.com")
    } else {
        json!([HASHED_EMAIL])
    };
    let mut event = json!({
        "event_name": if purchase { "Purchase" } else { "PageView" },
        "event_time": event_time,
        "event_id": format!("bench-{index}"),
        "action_source": "website",
        "event_source_url": "https://shop.example/checkout/thank-you",
        "user_data": {
            "em": email,
            "ph": [HASHED_PHONE],
            "client_ip_address": "203.0.113.42",
            "client_user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            "fbp": "fb.1.1758300000000.1234567890"
        }
    });
    if purchase {
        event["custom_data"] = json!({ "value": 129.99, "currency": "USD" });
    }
    event
}

fn meta_capi_batch(events: usize) -> String {
    let data: Vec<Value> = (0..events)
        .map(|index| meta_capi_event(index, false))
        .collect();
    serde_json::to_string(&json!({ "data": data })).expect("serialize capi batch")
}

fn meta_capi_batch_dirty(events: usize) -> String {
    let data: Vec<Value> = (0..events)
        .map(|index| meta_capi_event(index, true))
        .collect();
    serde_json::to_string(&json!({ "data": data })).expect("serialize dirty capi batch")
}

fn tiktok_capi_event(event_id: &str) -> String {
    serde_json::to_string(&json!({
        "pixel_code": "C3ABCDEF1234567890",
        "event": "CompletePayment",
        "event_id": event_id,
        "timestamp": "2026-07-26T06:00:00Z",
        "context": {
            "ip": "203.0.113.42",
            "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            "user": {
                "email": HASHED_EMAIL,
                "phone_number": HASHED_PHONE
            },
            "page": { "url": "https://example.com/checkout/thank-you" }
        },
        "properties": { "value": 129.99, "currency": "USD" }
    }))
    .expect("serialize tiktok capi")
}

fn fat_query(params: usize) -> String {
    let mut artifact = String::with_capacity(80 + params * 12);
    artifact.push_str("https://www.facebook.com/tr?id=1234567890123456&ev=PageView");
    for index in 0..params {
        artifact.push_str("&p");
        artifact.push_str(&index.to_string());
        artifact.push_str("=x");
    }
    artifact
}

fn padded_url(target_bytes: usize) -> String {
    let mut artifact = String::from(
        "https://ad.doubleclick.net/ddm/activity/src=1234567;type=convr0;cat=purch0;ord=1;u1=",
    );
    if artifact.len() >= target_bytes {
        return artifact;
    }
    artifact.push_str(&"a".repeat(target_bytes - artifact.len()));
    artifact
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixellint_core::{Engine, ValidationOptions};

    #[test]
    fn synth_fixtures_run_on_the_default_engine() {
        let engine = Engine::default();
        let options = ValidationOptions::default();
        let cases = synth_cases();
        assert!(cases.len() >= 20);
        assert!(
            cases
                .iter()
                .any(|case| case.name == "synth-capi-batch-1000")
        );
        for case in &cases {
            engine
                .validate(&case.request(), &options)
                .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        }
    }
}
