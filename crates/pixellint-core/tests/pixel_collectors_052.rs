use pixellint_core::{ArtifactKind, Engine, ValidationOptions, ValidationRequest};

fn validate(artifact: &str, kind: ArtifactKind) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact: artifact.into(),
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

fn codes(result: &pixellint_core::ValidationSummary) -> Vec<&str> {
    result
        .reports
        .iter()
        .flat_map(|r| &r.violations)
        .map(|v| v.code.as_str())
        .collect()
}

#[test]
fn quantcast_aliases_keep_denied_identity_and_core_privacy_checks() {
    for host in ["pixel.quantcount.com", "prreqcroab.icu"] {
        let result = validate(
            &format!(
                "https://{host}/pixel;a=p-synthetic;uh=u;uht=u;fpan=u;revenue=-2.5e2;gdpr=bad"
            ),
            ArtifactKind::Url,
        );
        assert_eq!(result.coverage().vendor_rulepacks, ["vendor/quantcast"]);
        assert_eq!(codes(&result), ["core.privacy.gdpr_invalid"]);
    }
}

#[test]
fn quantcast_beacon_keeps_matrix_parameters_on_the_url() {
    let capture = serde_json::json!({"url":"https://pixel.quantserve.com/pixel;a=p-synthetic;r=-1", "method":"POST", "headers":{}}).to_string();
    let result = validate(&capture, ArtifactKind::NetworkRequest);
    assert_eq!(codes(&result), ["vendor.quantcast.param.r.invalid"]);
}

#[test]
fn taboola_forms_validate_metrics_and_path_account_cannot_be_spoofed() {
    let capture = serde_json::json!({"url":"https://trc.taboola.com/wrong/log/3/unip", "method":"POST", "headers":{"Content-Type":"application/x-www-form-urlencoded"}, "body":"publisher_id=1234567&en=Custom&tim=clock&sd=opaque&ref=null"}).to_string();
    let result = validate(&capture, ArtifactKind::NetworkRequest);
    assert_eq!(
        codes(&result),
        [
            "vendor.taboola-unip.param.publisher_id.invalid",
            "vendor.taboola-unip.param.tim.invalid"
        ]
    );
}

#[test]
fn ttd_merchant_initialization_does_not_inherit_legacy_advertiser_requirements() {
    let result = validate(
        "https://insight.adsrvr.org/track/cei?merchant_id=synthetic&cookie_sync=0&ref=https%3A%2F%2Fexample.test%2F&upv=3.0.0",
        ArtifactKind::Url,
    );
    assert_eq!(
        result.coverage().vendor_rulepacks,
        ["vendor/the-trade-desk"]
    );
    assert!(codes(&result).is_empty());
}

#[test]
fn ttd_unset_processing_option_is_nullable_but_configured_objects_keep_members() {
    for (option, expected) in [
        (serde_json::Value::Null, vec![]),
        (
            serde_json::json!({}),
            vec![
                "vendor.the-trade-desk-conversions.body.policies.missing",
                "vendor.the-trade-desk-conversions.body.region.missing",
            ],
        ),
    ] {
        let capture = serde_json::json!({"url":"https://insight.adsrvr.org/track/realtimeconversion", "method":"POST", "headers":{"Content-Type":"application/json"}, "body":serde_json::json!({"data":[{"adv":"synthetic","pixel_ids":["synthetic"],"data_processing_option":option}]}).to_string()}).to_string();
        assert_eq!(
            codes(&validate(&capture, ArtifactKind::NetworkRequest)),
            expected
        );
    }
}

#[test]
fn ttd_get_fallback_does_not_inherit_json_body_or_processing_region_contracts() {
    let capture = serde_json::json!({"url":"https://insight.adsrvr.org/track/rt?adv=synthetic&pixel_ids=opaque&country=United%20States&region=TX&rt_fallback=1&eds=JsSdkGet", "method":"GET", "headers":{"Content-Type":"text/plain"}, "body":"unrelated captured text"}).to_string();
    let result = validate(&capture, ArtifactKind::NetworkRequest);
    assert_eq!(
        result.coverage().vendor_rulepacks,
        ["vendor/the-trade-desk-realtime-fallback"]
    );
    assert!(codes(&result).is_empty());
}

#[test]
fn deeper_collectors_leave_neighbor_routes_uncontracted_and_alias_lookalikes_unclaimed() {
    for url in [
        "https://pixel.quantcount.com/pixel-extra",
        "https://prreqcroab.icu/token",
        "https://trc.taboola.com/123/log/3/unip-extra",
        "https://insight.adsrvr.org/track/rt-extra",
    ] {
        let result = validate(url, ArtifactKind::Url);
        assert!(result.coverage().vendor_rulepacks.is_empty());
        assert_eq!(codes(&result), ["directory.no_rulepack_coverage"]);
    }
    let result = validate(
        "https://prreqcroab.icu.example.test/pixel;a=p-synthetic",
        ArtifactKind::Url,
    );
    assert!(result.coverage().vendor_rulepacks.is_empty());
    assert!(codes(&result).is_empty());
}
