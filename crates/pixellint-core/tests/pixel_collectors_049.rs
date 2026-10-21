use pixellint_core::{ArtifactKind, Engine, Severity, ValidationOptions, ValidationRequest};

fn validate(kind: ArtifactKind, artifact: String) -> pixellint_core::ValidationSummary {
    Engine::default()
        .validate(
            &ValidationRequest {
                artifact_kind: kind,
                artifact,
                claimed_vendor: None,
                expansion_state: Default::default(),
            },
            &ValidationOptions::default(),
        )
        .unwrap()
}

#[test]
fn configured_comscore_beacons_do_not_inherit_legacy_page_requirements() {
    for (query, owner, missing) in [
        ("c1=2&c2=1234567", "vendor/comscore", true),
        (
            "c1=2&c2=opaque&cs_cfg=0000110",
            "vendor/comscore-modern",
            false,
        ),
        (
            "c1=2&c2=opaque&cs_cfg=0001110&ns_cut=truncated",
            "vendor/comscore-modern",
            false,
        ),
    ] {
        let result = validate(
            ArtifactKind::Url,
            format!("https://sb.scorecardresearch.com/b?{query}"),
        );
        assert_eq!(result.coverage().vendor_rulepacks, [owner]);
        assert_eq!(
            result
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code.ends_with(".param.c7.missing")),
            missing
        );
    }
}

#[test]
fn clarity_custom_compression_keeps_coverage_deferral_visible() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/vendor-microsoft-clarity-collect/depth049-producer-clarity-compressed-0.txt"
    ))
    .unwrap();
    let result = validate(ArtifactKind::NetworkRequest, capture.to_string());
    assert_eq!(
        result.coverage().vendor_rulepacks,
        ["vendor/microsoft-clarity-collect"]
    );
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "core.request.unsupported_body_encoding");
    assert_eq!(findings[0].severity, Severity::Info);
}

#[test]
fn native_optimizely_types_survive_complete_capture_and_destination_selection() {
    let original = include_str!(
        "../../../fixtures/vendor-optimizely-events/depth049-event-type-timestamp-1792560000000.txt"
    );
    let result = validate(ArtifactKind::NetworkRequest, original.into());
    assert!(result.reports.iter().flat_map(|r| &r.violations).any(|v| {
        v.code == "vendor.optimizely-events.body.timestamp.invalid" && v.severity == Severity::Error
    }));
    let wrong_host = original.replace("logx.optimizely.com", "example.test");
    let result = validate(ArtifactKind::NetworkRequest, wrong_host);
    assert!(result.coverage().vendor_rulepacks.is_empty());
    assert!(result.reports.iter().all(|r| r.violations.is_empty()));
}

#[test]
fn numeric_ga4_properties_and_core_privacy_findings_both_run() {
    let result = validate(
        ArtifactKind::Url,
        "https://www.google-analytics.com/g/collect?v=2&tid=G-SYNTHETIC0&cid=opaque&en=custom&epn.value=free&gdpr=invalid".into(),
    );
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert!(findings.iter().any(|v| {
        v.code == "vendor.google-analytics-collect.param.epn.value.invalid"
            && v.severity == Severity::Warning
    }));
    assert!(
        findings
            .iter()
            .any(|v| { v.code == "core.privacy.gdpr_invalid" && v.severity == Severity::Error })
    );
}
