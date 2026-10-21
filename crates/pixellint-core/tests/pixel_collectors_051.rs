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
fn linkedin_partner_lists_and_page_fires_do_not_require_a_conversion() {
    let result = validate(
        "https://px.ads.linkedin.com/collect?pid=123,456&fmt=js&v=2&time=1792595250825",
        ArtifactKind::Url,
    );
    assert_eq!(result.coverage().vendor_rulepacks, ["vendor/linkedin"]);
    assert!(result.reports.iter().all(|r| r.violations.is_empty()));
}

#[test]
fn pinterest_post_forms_check_later_native_line_items() {
    let body = serde_json::json!({
        "url": "https://ct.pinterest.com/v3/", "method": "POST",
        "headers": {"Content-Type": "application/x-www-form-urlencoded"},
        "body": "tid=1234567890123&event=checkout&ed=%7B%22line_items%22%3A%5B%7B%22product_quantity%22%3A1%7D%2C%7B%22product_quantity%22%3A1.5%7D%5D%7D"
    }).to_string();
    let result = validate(&body, ArtifactKind::NetworkRequest);
    let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].code,
        "vendor.pinterest.body.line_items[].product_quantity.invalid"
    );
    assert_eq!(
        findings[0].field.as_deref(),
        Some("param.ed.line_items[].product_quantity")
    );
}

#[test]
fn pinterest_custom_event_and_source_supported_hashes_remain_valid() {
    let url = format!(
        "https://ct.pinterest.com/v3/?tid=opaque&event=Custom%20Event&pd=%7B%22em%22%3A%5B%22{}%22%2C%22{}%22%2Cnull%2C%22%22%5D%7D",
        "a".repeat(32),
        "B".repeat(40)
    );
    let result = validate(&url, ArtifactKind::Url);
    assert!(result.reports.iter().all(|r| r.violations.is_empty()));
}

#[test]
fn quora_dwell_reason_is_a_lifecycle_string() {
    for (reason, invalid) in [("blur", false), ("pagehide", false), ("true", true)] {
        let result = validate(
            &format!("https://q.quora.com/_/ad/opaque/pixel?tag=DwellTime&dwt=84&ive={reason}"),
            ArtifactKind::Url,
        );
        assert_eq!(
            result
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "vendor.quora-pixel.param.ive.invalid"),
            invalid
        );
    }
}

#[test]
fn chartbeat_case_distinct_fields_keep_independent_contracts() {
    for (pair, code) in [
        ("E=bad&e=1", "E"),
        ("E=1&e=bad", "e"),
        ("L=bad&l=1", "L"),
        ("L=1&l=bad", "l"),
        ("R=1&W=0&I=0&r=", ""),
    ] {
        let result = validate(
            &format!(
                "https://ping.chartbeat.net/ping?h=example.test&g=12345&p=/article&d=example.test&t=opaque&{pair}"
            ),
            ArtifactKind::Url,
        );
        let findings: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
        if code.is_empty() {
            assert!(findings.is_empty());
        } else {
            assert_eq!(findings.len(), 1);
            assert_eq!(
                findings[0].code,
                format!("vendor.chartbeat.param.{code}.invalid")
            );
            assert_eq!(
                findings[0].field.as_deref(),
                Some(format!("param.{code}").as_str())
            );
        }
    }
}

#[test]
fn deeper_packs_leave_neighbor_protocols_recognized_but_uncontracted() {
    for url in [
        "https://px.ads.linkedin.com/wa/",
        "https://ct.pinterest.com/user/",
        "https://q.quora.com/_/ad/id/pixel/extra",
        "https://ping.chartbeat.net/ping/mab",
    ] {
        let result = validate(url, ArtifactKind::Url);
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
