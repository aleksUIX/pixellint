use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, Severity, ValidationOptions, ValidationRequest,
};

fn request(kind: ArtifactKind, artifact: impl Into<String>) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact: artifact.into(),
        claimed_vendor: None,
        expansion_state: Default::default(),
    }
}

#[test]
fn regional_amazon_routes_do_not_cross_claim_each_other() {
    let engine = Engine::default();
    for (host, path, expected) in [
        ("s.amazon-adsystem.com", "/iu3", "vendor/amazon-ad-tag"),
        (
            "aax-eu.amazon-adsystem.com",
            "/s/iu3",
            "vendor/amazon-ad-tag-regional",
        ),
        (
            "aax-fe.amazon-adsystem.com",
            "/s/iu3",
            "vendor/amazon-ad-tag-regional",
        ),
    ] {
        let url = format!("https://{host}{path}?pid=opaque-server-tag&event=Custom");
        let summary = engine
            .validate(&request(ArtifactKind::Url, url), &Default::default())
            .unwrap();
        assert_eq!(summary.coverage().vendor_rulepacks, [expected]);
        assert!(summary.reports.iter().all(|r| r.violations.is_empty()));
        let wrong_path = if path == "/iu3" { "/s/iu3" } else { "/iu3" };
        let wrong = engine
            .validate(
                &request(ArtifactKind::Url, format!("https://{host}{wrong_path}")),
                &Default::default(),
            )
            .unwrap();
        assert_eq!(
            wrong.coverage().status,
            EndpointCoverageStatus::RecognizedUncontracted
        );
    }
}

#[test]
fn browser_outbrain_beacons_with_click_ids_do_not_inherit_s2s_get_requirement() {
    let engine = Engine::default();
    for (identity, ok) in [
        ("marketerId=synthetic&ob_click_id=synthetic", true),
        ("ob_click_id=synthetic", false),
    ] {
        let capture = serde_json::json!({"url":format!("https://tr.outbrain.com/unifiedPixel?{identity}&name=Purchase"),"method":"POST","headers":{}});
        let summary = engine
            .validate(
                &request(ArtifactKind::NetworkRequest, capture.to_string()),
                &Default::default(),
            )
            .unwrap();
        assert_eq!(summary.is_ok(), ok);
        assert_eq!(
            summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "vendor.outbrain.http.method.invalid"),
            !ok
        );
    }
}

#[test]
fn reddit_form_metadata_and_core_privacy_checks_both_run() {
    let capture = serde_json::json!({"url":"https://alb.reddit.com/rp?id=t2_synthetic&event=Purchase&gdpr=invalid","method":"POST","headers":{"Content-Type":"application/x-www-form-urlencoded"},"body":"m.products=%5B%7B%22quantity%22%3A%22bad%22%7D%5D"});
    let summary = Engine::default()
        .validate(
            &request(ArtifactKind::NetworkRequest, capture.to_string()),
            &Default::default(),
        )
        .unwrap();
    let findings: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
    assert!(
        findings
            .iter()
            .any(|v| v.code == "core.privacy.gdpr_invalid" && v.severity == Severity::Error)
    );
    assert!(findings.iter().any(
        |v| v.code == "vendor.reddit.body.quantity.invalid" && v.severity == Severity::Warning
    ));
    assert_eq!(summary.coverage().vendor_rulepacks, ["vendor/reddit"]);
}

#[test]
fn excluded_new_collectors_retain_coverage_disclosure() {
    let engine = Engine::default();
    for (owner, url) in [
        ("vendor/amazon-ad-tag", "https://s.amazon-adsystem.com/iu3"),
        (
            "vendor/amazon-ad-tag-regional",
            "https://aax-eu.amazon-adsystem.com/s/iu3",
        ),
        (
            "vendor/adroll-image",
            "https://d.adroll.com/ipixel/ADV/PIX?name=segment",
        ),
        ("vendor/adroll-pixel", "https://d.adroll.com/pixel/ADV/PIX"),
    ] {
        let result = engine
            .validate(
                &request(ArtifactKind::Url, url),
                &ValidationOptions {
                    only_rulepacks: vec![],
                    except_rulepacks: vec![owner.into()],
                },
            )
            .unwrap();
        assert_eq!(
            result.coverage().status,
            EndpointCoverageStatus::RecognizedUncontracted
        );
        assert!(result.coverage().vendor_rulepacks.is_empty());
    }
}
