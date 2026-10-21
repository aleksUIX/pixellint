use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ValidationOptions, ValidationRequest,
    document_request_from_json,
};

const CASES: &[(&str, &str)] = &[
    (
        "vendor/spotify-image",
        "https://img.byspotify.com/?key=opaque&a=init",
    ),
    (
        "vendor/stackadapt-pixel",
        "https://tags.srv.stackadapt.com/conv?cid=opaque",
    ),
    (
        "vendor/quora-pixel",
        "https://q.quora.com/_/ad/opaque/pixel?tag=CustomEvent",
    ),
    (
        "vendor/yandex-adfox-links",
        "https://yandex.com/ads/adfox/123456/getCode?pr=1",
    ),
    (
        "vendor/google-ima-live-pcs",
        "https://securepubads.g.doubleclick.net/live/pcs/view?xai=x&sai=s&sig=g",
    ),
];

fn request(kind: ArtifactKind, artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact: artifact.into(),
        claimed_vendor: None,
        expansion_state: Default::default(),
    }
}

#[test]
fn new_pixel_routes_have_real_coverage_across_url_vast_and_http_captures() {
    let engine = Engine::default();
    for (pack, url) in CASES {
        for kind in [
            ArtifactKind::Url,
            ArtifactKind::VastTracker,
            ArtifactKind::NetworkRequest,
        ] {
            let artifact = if kind == ArtifactKind::NetworkRequest {
                serde_json::json!({"url":url,"method":"GET","headers":{}}).to_string()
            } else {
                (*url).to_owned()
            };
            let result = engine
                .validate(&request(kind, &artifact), &Default::default())
                .unwrap();
            assert!(result.is_ok(), "{pack}: {:?}", result.reports);
            assert_eq!(
                result.coverage().status,
                EndpointCoverageStatus::VendorChecked
            );
            assert_eq!(result.coverage().vendor_rulepacks, [*pack]);
        }
    }
}

#[test]
fn excluding_each_new_pack_does_not_claim_endpoint_checks() {
    let engine = Engine::default();
    for (pack, url) in CASES {
        let options = ValidationOptions {
            only_rulepacks: vec![],
            except_rulepacks: vec![(*pack).into()],
        };
        let result = engine
            .validate(&request(ArtifactKind::Url, url), &options)
            .unwrap();
        assert_eq!(
            result.coverage().status,
            EndpointCoverageStatus::RecognizedUncontracted,
            "{pack}"
        );
        assert!(result.coverage().vendor_rulepacks.is_empty());
        assert!(result.coverage().core_checks_ran);
    }
}

#[test]
fn path_identity_cannot_be_replaced_by_query_keys() {
    let engine = Engine::default();
    let result = engine
        .validate(
            &request(
                ArtifactKind::Url,
                "https://tags.srv.stackadapt.com/rt?pixel_type=conv&cid=opaque",
            ),
            &Default::default(),
        )
        .unwrap();
    let codes: Vec<_> = result
        .reports
        .iter()
        .flat_map(|r| r.violations.iter())
        .map(|v| v.code.as_str())
        .collect();
    assert_eq!(codes, ["vendor.stackadapt-pixel.param.sid.missing"]);
}

#[test]
fn aggregate_rows_keep_endpoint_identity_and_core_privacy_findings() {
    let engine = Engine::default();
    let document = document_request_from_json(
        &serde_json::json!([
            "https://img.byspotify.com/?key=opaque&a=init&gdpr=NaN",
            "https://img.byspotify.com/?key=opaque&a=init&gdpr=NaN",
            "https://yandex.ru/adfox/123456/event?pr=a"
        ])
        .to_string(),
    )
    .unwrap();
    let result = engine
        .validate_many(&document, &Default::default())
        .unwrap();
    let value = serde_json::to_value(result).unwrap();
    let rows = value["artifacts"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["coverage"]["status"], "vendor_checked");
    assert_eq!(rows[1]["coverage"]["status"], "recognized_uncontracted");
    assert!(value.to_string().contains("core.privacy.gdpr_invalid"));
    assert!(
        !value
            .to_string()
            .contains("vendor.yandex-adfox-links.param.pr.invalid")
    );
}
