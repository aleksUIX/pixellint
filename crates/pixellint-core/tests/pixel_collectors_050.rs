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

#[test]
fn multipart_ndjson_keeps_record_identity_and_later_failures() {
    let capture =
        include_str!("../../../fixtures/vendor-woopra/depth050-producer-woopra-push-0.txt");
    let result = validate(capture, ArtifactKind::NetworkRequest);
    assert_eq!(result.coverage().vendor_rulepacks, ["vendor/woopra"]);
    assert!(result.reports.iter().all(|r| r.violations.is_empty()));
    let result = validate(
        include_str!("../../../fixtures/vendor-woopra/depth050-push-second-record.txt"),
        ArtifactKind::NetworkRequest,
    );
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "vendor.woopra.body.params.event.invalid");
    assert_eq!(
        findings[0].field.as_deref(),
        Some("param.payload.[1].params.event")
    );
}

#[test]
fn piwik_pro_bulk_events_use_their_own_limits_and_independent_rows() {
    let result = validate(
        include_str!("../../../fixtures/vendor-piwik-pro/depth050-bulk-second-row.txt"),
        ArtifactKind::NetworkRequest,
    );
    assert_eq!(result.coverage().vendor_rulepacks, ["vendor/piwik-pro"]);
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "vendor.piwik-pro.param.h.invalid");
    assert!(
        findings[0]
            .field
            .as_deref()
            .unwrap()
            .contains("requests[1]")
    );
    let result = validate(
        include_str!("../../../fixtures/vendor-piwik-pro/depth050-product-null-optionals.txt"),
        ArtifactKind::Url,
    );
    assert!(result.reports.iter().all(|r| r.violations.is_empty()));
}

#[test]
fn x_onetag_and_yahoo_browser_profiles_keep_route_boundaries() {
    for (url, owner) in [
        (
            "https://analytics.x.com/1/i/adsct?txn_id=opaque&p_id=Twitter&event={}",
            "vendor/x",
        ),
        (
            "https://sp.analytics.yahoo.com/sp.pl?a=opaque&.yp=12345",
            "vendor/yahoo-browser",
        ),
        (
            "https://sp.analytics.yahoo.com/spp.pl?a=opaque&.yp=12345",
            "vendor/yahoo-dot",
        ),
    ] {
        let result = validate(url, ArtifactKind::Url);
        assert_eq!(result.coverage().vendor_rulepacks, [owner]);
    }
    let result = validate("https://analytics.x.com/1/i/adsctive", ArtifactKind::Url);
    assert!(result.coverage().vendor_rulepacks.is_empty());
    assert!(
        result
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code == "directory.no_rulepack_coverage")
    );
}

#[test]
fn woopra_current_milliseconds_allow_the_validators_second_resolution() {
    let engine = Engine::default();
    for (timestamp, future) in [(1_792_560_000_999i64, false), (1_792_560_002_001, true)] {
        let result = engine.validate_at(&ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: format!("https://www.woopra.com/track/ce?project=example.test&event=pageview&cookie=opaque&timestamp={timestamp}"),
            claimed_vendor: None,
            expansion_state: Default::default(),
        }, &ValidationOptions::default(), 1_792_560_000).unwrap();
        assert_eq!(
            result
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "vendor.woopra.future_timestamp"),
            future
        );
    }
}
