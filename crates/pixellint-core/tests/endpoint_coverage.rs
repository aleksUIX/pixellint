use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ValidationOptions, ValidationRequest,
    VendorDirectory, document_request_from_json,
};

fn request(url: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: ArtifactKind::Url,
        artifact: url.to_owned(),
        claimed_vendor: None,
        expansion_state: Default::default(),
    }
}

#[test]
fn coverage_distinguishes_clean_vendor_known_and_unknown_endpoints() {
    let engine = Engine::default();
    for (url, expected, pack) in [
        (
            "https://tpsc-video.doubleverify.com/event.png?vstevt=1",
            EndpointCoverageStatus::VendorChecked,
            Some("vendor/doubleverify-event"),
        ),
        (
            "https://unified.adsafeprotected.com/pixel.png?a=1",
            EndpointCoverageStatus::RecognizedUncontracted,
            None,
        ),
        (
            "https://unknown.example/pixel.png",
            EndpointCoverageStatus::Unrecognized,
            None,
        ),
    ] {
        let summary = engine.validate(&request(url), &Default::default()).unwrap();
        assert!(summary.is_ok(), "{url}");
        let coverage = summary.coverage();
        assert_eq!(coverage.status, expected, "{url}");
        assert!(coverage.core_checks_ran);
        assert_eq!(
            coverage.vendor_rulepacks,
            pack.into_iter().collect::<Vec<_>>()
        );
        let serialized = serde_json::to_value(&summary).unwrap();
        assert_eq!(serialized["coverage"]["status"], expected.as_str());
    }
}

#[test]
fn coverage_is_independent_of_errors_and_tracks_the_reports_actually_selected() {
    let engine = Engine::default();
    let url = "https://tpsc-video.doubleverify.com/event.png?gdpr=NaN";
    for (options, expected, core, ok) in [
        (
            ValidationOptions::default(),
            EndpointCoverageStatus::VendorChecked,
            true,
            false,
        ),
        (
            ValidationOptions {
                only_rulepacks: vec!["directory".into()],
                except_rulepacks: vec![],
            },
            EndpointCoverageStatus::RecognizedUncontracted,
            false,
            true,
        ),
        (
            ValidationOptions {
                only_rulepacks: vec!["core".into()],
                except_rulepacks: vec![],
            },
            EndpointCoverageStatus::Unrecognized,
            true,
            false,
        ),
        (
            ValidationOptions {
                only_rulepacks: vec![],
                except_rulepacks: vec!["vendor/doubleverify-event".into()],
            },
            EndpointCoverageStatus::RecognizedUncontracted,
            true,
            false,
        ),
    ] {
        let summary = engine.validate(&request(url), &options).unwrap();
        assert_eq!(summary.coverage().status, expected);
        assert_eq!(summary.coverage().core_checks_ran, core);
        assert_eq!(summary.is_ok(), ok);
    }
    let mismatch = engine
        .validate(
            &request("https://unknown.example/event.png"),
            &ValidationOptions {
                only_rulepacks: vec!["vendor/doubleverify-event".into()],
                except_rulepacks: vec![],
            },
        )
        .unwrap();
    assert_eq!(
        mismatch.coverage().status,
        EndpointCoverageStatus::Unrecognized
    );
    assert!(!mismatch.coverage().core_checks_ran);
    assert!(mismatch.coverage().vendor_rulepacks.is_empty());
}

#[test]
fn directory_paths_do_not_attribute_unrelated_pages_or_lookalike_hosts() {
    let directory = VendorDirectory::from_json(
        r#"{"entries":[{
        "vendor":"shared-service","display_name":"Shared service","category":"video",
        "hosts":["shared.example"],"path_prefixes":["/adfox/"]
    }]}"#,
    )
    .unwrap();
    assert!(directory.lookup_host("shared.example").is_none());
    assert!(
        directory
            .lookup_endpoint("SHARED.EXAMPLE", "/adfox/123/event")
            .is_some()
    );
    assert!(
        directory
            .lookup_endpoint("cdn.shared.example", "/adfox/123/event")
            .is_some()
    );
    for (host, path) in [
        ("shared.example", "/"),
        ("shared.example", "/adfoxtest/123/event"),
        ("shared.example", "/Adfox/123/event"),
        ("shared.example", "/adfox/../search"),
        ("shared.example", "/adfox/%2e%2e/search"),
        ("shared.example.attacker.test", "/adfox/123/event"),
        ("fake-shared.example", "/adfox/123/event"),
    ] {
        assert!(
            directory.lookup_endpoint(host, path).is_none(),
            "{host}{path}"
        );
    }
    let mut engine = Engine::default();
    engine.set_directory(directory);
    for (url, status) in [
        (
            "https://shared.example/adfox/123/event?gdpr=NaN",
            EndpointCoverageStatus::RecognizedUncontracted,
        ),
        (
            "https://shared.example/other/../adfox/123/event?gdpr=NaN",
            EndpointCoverageStatus::RecognizedUncontracted,
        ),
        (
            "https://shared.example/adfox/../search?gdpr=NaN",
            EndpointCoverageStatus::Unrecognized,
        ),
        (
            "https://shared.example/adfox/%2e%2e/search?gdpr=NaN",
            EndpointCoverageStatus::Unrecognized,
        ),
        (
            "https://shared.example/other?redirect=/adfox/123/event&gdpr=NaN",
            EndpointCoverageStatus::Unrecognized,
        ),
    ] {
        let summary = engine.validate(&request(url), &Default::default()).unwrap();
        assert_eq!(summary.coverage().status, status);
        assert!(!summary.is_ok());
        assert!(
            summary
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| v.code == "core.privacy.gdpr_invalid")
        );
    }
    for prefix in ["adfox/", "/adfox/?q=", "/adfox/#fragment", ""] {
        let json = serde_json::json!({"entries":[{"vendor":"shared","display_name":"Shared","category":"video","hosts":["shared.example"],"path_prefixes":[prefix]}]});
        assert!(VendorDirectory::from_json(&json.to_string()).is_err());
    }
}

fn engine_with_custom_contract() -> Engine {
    let mut engine = Engine::default();
    engine
        .register_manifest_json(
            r#"{
        "id":"custom/local","display_name":"Local contract",
        "description":"An authored synthetic collector contract without vendor attribution.",
        "source_level":"heuristic",
        "match":{"hosts":["unified.adsafeprotected.com"],"paths":["/pixel.png"]},
        "params":[{"name":"id","requirement":"required"}],
        "rules":[{
            "code":"custom.local.endpoint_mismatch","kind":"forbid_value_pattern",
            "params":["id"],"pattern":"^blocked$","severity":"error",
            "message":"Synthetic local policy excludes this identifier."
        }]
    }"#,
        )
        .unwrap();
    engine
}

#[test]
fn vendorless_custom_contracts_have_coverage_independent_of_findings_or_rule_codes() {
    let engine = engine_with_custom_contract();
    for (suffix, ok) in [("?id=valid", true), ("", false), ("?id=blocked", false)] {
        let url = format!("https://unified.adsafeprotected.com/pixel.png{suffix}");
        let summary = engine
            .validate(&request(&url), &Default::default())
            .unwrap();
        assert_eq!(
            summary.coverage().status,
            EndpointCoverageStatus::VendorChecked
        );
        assert_eq!(summary.coverage().vendor_rulepacks, ["custom/local"]);
        assert!(summary.coverage().core_checks_ran);
        assert_eq!(summary.is_ok(), ok, "{url}");
        assert!(summary.reports.iter().all(|r| r.plugin_id != "directory"));
        assert!(summary.reports.iter().all(|r| r.detected_vendor.is_none()));
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["coverage"]["status"], "vendor_checked");
        assert_eq!(json["coverage"]["vendor_rulepacks"][0], "custom/local");
    }
}

#[test]
fn custom_contract_coverage_respects_explicit_mismatches_and_exclusions() {
    let engine = engine_with_custom_contract();
    let only_custom = ValidationOptions {
        only_rulepacks: vec!["custom/local".into()],
        except_rulepacks: vec![],
    };
    let matched = engine
        .validate(
            &request("https://unified.adsafeprotected.com/pixel.png?id=valid"),
            &only_custom,
        )
        .unwrap();
    assert_eq!(
        matched.coverage().status,
        EndpointCoverageStatus::VendorChecked
    );
    assert_eq!(matched.coverage().vendor_rulepacks, ["custom/local"]);
    assert!(!matched.coverage().core_checks_ran);

    for url in [
        "https://unknown.example/pixel.png?id=valid",
        "https://unified.adsafeprotected.com/other?id=valid",
    ] {
        let mismatch = engine.validate(&request(url), &only_custom).unwrap();
        assert_eq!(
            mismatch.coverage().status,
            EndpointCoverageStatus::Unrecognized
        );
        assert!(mismatch.coverage().vendor_rulepacks.is_empty());
        assert!(!mismatch.coverage().core_checks_ran);
        assert!(
            mismatch
                .reports
                .iter()
                .flat_map(|r| &r.violations)
                .any(|v| { v.code == "custom.local.endpoint_mismatch" })
        );
    }
    let excluded = engine
        .validate(
            &request("https://unified.adsafeprotected.com/pixel.png?id=valid"),
            &ValidationOptions {
                only_rulepacks: vec![],
                except_rulepacks: vec!["custom/local".into()],
            },
        )
        .unwrap();
    assert_eq!(
        excluded.coverage().status,
        EndpointCoverageStatus::RecognizedUncontracted
    );
    assert!(excluded.coverage().vendor_rulepacks.is_empty());
    assert!(excluded.coverage().core_checks_ran);
}

#[test]
fn captured_custom_contracts_require_a_matching_endpoint_and_valid_capture_envelope() {
    let engine = engine_with_custom_contract();
    for (url, ok) in [
        (
            "https://unified.adsafeprotected.com/pixel.png?id=valid",
            true,
        ),
        ("https://unified.adsafeprotected.com/pixel.png", false),
    ] {
        let mut capture =
            request(&serde_json::json!({"url":url,"method":"GET","headers":{}}).to_string());
        capture.artifact_kind = ArtifactKind::NetworkRequest;
        let summary = engine.validate(&capture, &Default::default()).unwrap();
        assert_eq!(
            summary.coverage().status,
            EndpointCoverageStatus::VendorChecked
        );
        assert_eq!(summary.coverage().vendor_rulepacks, ["custom/local"]);
        assert!(summary.coverage().core_checks_ran);
        assert_eq!(summary.is_ok(), ok);
        assert!(summary.reports.iter().all(|r| r.plugin_id != "directory"));
    }
    let only_custom = ValidationOptions {
        only_rulepacks: vec!["custom/local".into()],
        except_rulepacks: vec![],
    };
    let mut mismatch = request(
        r#"{"url":"https://unknown.example/pixel.png?id=valid","method":"GET","headers":{}}"#,
    );
    mismatch.artifact_kind = ArtifactKind::NetworkRequest;
    let summary = engine.validate(&mismatch, &only_custom).unwrap();
    assert_eq!(
        summary.coverage().status,
        EndpointCoverageStatus::Unrecognized
    );
    assert!(summary.coverage().vendor_rulepacks.is_empty());
    assert!(!summary.coverage().core_checks_ran);

    let mut invalid =
        request(r#"{"url":"https://unified.adsafeprotected.com/pixel.png?id=valid"}"#);
    invalid.artifact_kind = ArtifactKind::NetworkRequest;
    let summary = engine.validate(&invalid, &only_custom).unwrap();
    assert_eq!(
        summary.coverage().status,
        EndpointCoverageStatus::Unrecognized
    );
    assert!(summary.coverage().vendor_rulepacks.is_empty());
    assert!(summary.coverage().core_checks_ran);
    assert!(!summary.is_ok());
}

#[test]
fn serialized_capture_and_document_results_retain_endpoint_coverage() {
    let engine = Engine::default();
    let mut capture =
        request(r#"{"url":"https://unknown.example/pixel","method":"GET","headers":{}}"#);
    capture.artifact_kind = ArtifactKind::NetworkRequest;
    let summary = engine.validate(&capture, &Default::default()).unwrap();
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["coverage"]["status"], "unrecognized");
    assert_eq!(json["coverage"]["core_checks_ran"], true);

    let document = document_request_from_json(
        r#"[
        "https://unknown.example/pixel",
        "https://unified.adsafeprotected.com/pixel.png",
        "https://tpsc-video.doubleverify.com/event.png?vstevt=1"
    ]"#,
    )
    .unwrap();
    let result = serde_json::to_value(
        engine
            .validate_many(&document, &Default::default())
            .unwrap(),
    )
    .unwrap();
    for (i, expected) in ["unrecognized", "recognized_uncontracted", "vendor_checked"]
        .iter()
        .enumerate()
    {
        assert_eq!(result["artifacts"][i]["coverage"]["status"], *expected);
    }
}
