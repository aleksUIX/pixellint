use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ManifestRulePack, RuleSourceLevel, Severity,
    ValidationOptions, ValidationRequest, ValidatorPlugin, VendorDirectory,
};

const REMERGE: &str = include_str!("../rulepacks/vendor/remerge-event.json");
const HYBRID: &str = include_str!("../rulepacks/vendor/hybrid-dss-pixel.json");

fn input(artifact: &str, artifact_kind: ArtifactKind) -> ValidationRequest {
    ValidationRequest {
        artifact: artifact.into(),
        artifact_kind,
        expansion_state: ExpansionState::Unknown,
        claimed_vendor: None,
    }
}

#[test]
fn source_examples_preserve_empty_content_and_platform_context() {
    let remerge = ManifestRulePack::from_json(REMERGE).unwrap();
    let hybrid = ManifestRulePack::from_json(HYBRID).unwrap();
    for (pack, artifact) in [
        (
            &remerge,
            include_str!("../../../fixtures/vendor-remerge-event/source-event-template.txt"),
        ),
        (
            &remerge,
            include_str!("../../../fixtures/vendor-remerge-event/source-bi-template.txt"),
        ),
        (
            &remerge,
            include_str!(
                "../../../fixtures/vendor-remerge-event/common-fields-only-identity-context-open.txt"
            ),
        ),
        (
            &remerge,
            include_str!(
                "../../../fixtures/vendor-remerge-event/json-looking-string-remains-open.txt"
            ),
        ),
        (
            &hybrid,
            include_str!("../../../fixtures/vendor-hybrid-dss-pixel/source-pixel-template.txt"),
        ),
        (
            &hybrid,
            include_str!("../../../fixtures/vendor-hybrid-dss-pixel/source-opaque-event.txt"),
        ),
        (
            &hybrid,
            include_str!("../../../fixtures/vendor-hybrid-dss-pixel/missing-cnt.txt"),
        ),
    ] {
        let request = input(artifact, ArtifactKind::Url);
        assert!(pack.supports(&request));
        assert!(pack.validate(&request).violations.is_empty(), "{artifact}");
    }
}

#[test]
fn each_decoded_duplicate_retains_its_own_advisory_without_receiver_policy() {
    for (manifest, url, code, level) in [
        (
            REMERGE,
            "https://remerge.events/event?app_id=x&event=y&partner=z&key=synthetic&ts=1792886400&%74s=bad",
            "vendor.remerge-event.param.ts.invalid",
            RuleSourceLevel::OfficialVendor,
        ),
        (
            REMERGE,
            "https://remerge.events/event?app_id=x&event=y&partner=z&key=synthetic&event=",
            "vendor.remerge-event.param.event.empty",
            RuleSourceLevel::OfficialVendor,
        ),
        (
            HYBRID,
            "https://dss.hybrid.ai/pixel/trackapi2?pid=x&evt=y&txid=0123456789abcdef0123&%74xid=bad",
            "vendor.hybrid-dss-pixel.param.txid.invalid",
            RuleSourceLevel::OfficialVendor,
        ),
    ] {
        let pack = ManifestRulePack::from_json(manifest).unwrap();
        let report = pack.validate(&input(url, ArtifactKind::Url));
        assert_eq!(report.violations.len(), 1, "{report:?}");
        let finding = &report.violations[0];
        assert_eq!(finding.code, code);
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.source.level, level);
    }
}

#[test]
fn capture_methods_and_native_bodies_do_not_import_adjacent_api_contracts() {
    let engine = Engine::default();
    for method in ["GET", "POST", "PATCH"] {
        let capture = serde_json::json!({
            "url": "https://dss.hybrid.ai/pixel/trackapi2?pid=opaque&evt=custom&txid=0123456789abcdef0123&cnt=",
            "method": method,
            "headers": {"Content-Type": "application/json"},
            "body": r#"{"pid":null,"txid":false,"evt":[]}"#,
        });
        let request = input(&capture.to_string(), ArtifactKind::NetworkRequest);
        let summary = engine
            .validate_at(&request, &ValidationOptions::default(), 1792886400)
            .unwrap();
        let report = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "vendor/hybrid-dss-pixel")
            .unwrap();
        assert!(report.violations.is_empty());
    }
    let capture = serde_json::json!({
        "url": "https://remerge.events/event?app_id=x&event=y&partner=z&key=synthetic",
        "method": "POST", "headers": {},
    });
    let summary = engine
        .validate_at(
            &input(&capture.to_string(), ArtifactKind::NetworkRequest),
            &ValidationOptions::default(),
            1792886400,
        )
        .unwrap();
    let report = summary
        .reports
        .iter()
        .find(|r| r.plugin_id == "vendor/remerge-event")
        .unwrap();
    assert_eq!(report.violations.len(), 1);
    assert_eq!(
        report.violations[0].code,
        "vendor.remerge-event.http.method.invalid"
    );
}

#[test]
fn directory_pointers_leave_uncovered_routes_and_bodies_unselected() {
    let directory = VendorDirectory::builtin();
    for (host, owner, pointer) in [
        ("remerge.events", "remerge", "vendor/remerge-event"),
        ("dss.hybrid.ai", "hybrid", "vendor/hybrid-dss-pixel"),
    ] {
        let entry = directory.lookup_host(host).unwrap();
        assert_eq!(entry.vendor, owner);
        assert_eq!(entry.rulepack.as_deref(), Some(pointer));
    }
    for (manifest, urls) in [
        (
            REMERGE,
            vec![
                "https://eu5.remerge.events/event",
                "https://eu5.remerge.events/a/b/complete",
                "https://remerge.events/event/",
                "https://remerge.events/other?url=/event",
            ],
        ),
        (
            HYBRID,
            vec![
                "https://dsa-eu.hybrid.ai/pixel/trackapi2",
                "https://ssp.hybrid.ai/pixel/trackapi2",
                "https://capi.hybrid.ai/audience/create",
                "https://dss.hybrid.ai/pixel/trackapi2/",
                "https://dss.hybrid.ai.example/pixel/trackapi2",
            ],
        ),
    ] {
        let pack = ManifestRulePack::from_json(manifest).unwrap();
        for url in urls {
            assert!(!pack.supports(&input(url, ArtifactKind::Url)), "{url}");
        }
        assert!(!pack.supports(&input(
            r#"{"app_id":"x","pid":"x","txid":"0123456789abcdef0123"}"#,
            ArtifactKind::JsonPayload
        )));
    }
}
