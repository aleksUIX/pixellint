use pixellint_core::{
    ArtifactKind, EndpointCoverageStatus, Engine, ValidationOptions, ValidationRequest,
};

fn request(kind: ArtifactKind, artifact: String) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact,
        claimed_vendor: None,
        expansion_state: Default::default(),
    }
}

#[test]
fn quantcast_identity_carriers_select_one_contract_and_resist_query_shadowing() {
    let engine = Engine::default();
    for (url, owner, code) in [
        (
            "https://pixel.quantserve.com/pixel;a=p-synthetic",
            "vendor/quantcast",
            None,
        ),
        (
            "https://pixel.quantserve.com/pixel/bad.gif?pcode=p-synthetic&a=p-synthetic",
            "vendor/quantcast-image",
            Some("vendor.quantcast-image.param.pcode.invalid"),
        ),
    ] {
        let result = engine
            .validate(&request(ArtifactKind::Url, url.into()), &Default::default())
            .unwrap();
        assert_eq!(result.coverage().vendor_rulepacks, [owner]);
        assert_eq!(result.is_ok(), code.is_none());
        if let Some(code) = code {
            assert!(
                result
                    .reports
                    .iter()
                    .flat_map(|r| &r.violations)
                    .any(|v| v.code == code)
            );
        }
    }
}

#[test]
fn generic_conversion_arrays_do_not_select_quantcast_without_destination_context() {
    let result = Engine::default()
        .validate(
            &request(
                ArtifactKind::JsonPayload,
                r#"[{"name":"Purchase","user":{"external_id":"synthetic"}}]"#.into(),
            ),
            &Default::default(),
        )
        .unwrap();
    assert!(
        !result
            .reports
            .iter()
            .any(|r| r.plugin_id == "vendor/quantcast-conversions-api")
    );
}

#[test]
fn excluded_collectors_retain_directory_coverage_disclosure() {
    let engine = Engine::default();
    for (owner, url) in [
        ("vendor/nextdoor-pixel", "https://flask.nextdoor.com/pixel"),
        (
            "vendor/spotify-ingest",
            "https://pixels.spotify.com/v1/ingest",
        ),
        (
            "vendor/quantcast-image",
            "https://pixel.quantserve.com/pixel/p-synthetic.gif",
        ),
        (
            "vendor/quantcast-conversions-api",
            "https://pixel.quantserve.com/conversion?a=p-synthetic",
        ),
        ("vendor/mntn-image", "https://px.mountain.com/sti"),
    ] {
        let result = engine
            .validate(
                &request(ArtifactKind::Url, url.into()),
                &ValidationOptions {
                    only_rulepacks: vec![],
                    except_rulepacks: vec![owner.into()],
                },
            )
            .unwrap();
        assert_eq!(
            result.coverage().status,
            EndpointCoverageStatus::RecognizedUncontracted,
            "{owner}"
        );
        assert!(result.coverage().vendor_rulepacks.is_empty());
    }
}

#[test]
fn spotify_producer_advice_does_not_mask_core_privacy_errors() {
    let artifact = serde_json::json!({
        "url":"https://pixels.spotify.com/v1/ingest?gdpr=NaN",
        "method":"POST",
        "headers":{"Content-Type":"application/json"},
        "body":r#"{"batch":[{"pixel_id":"synthetic","events":[{"action":"purchase","body":{"value":"bad"}}]}]}"#
    });
    let result = Engine::default()
        .validate(
            &request(ArtifactKind::NetworkRequest, artifact.to_string()),
            &Default::default(),
        )
        .unwrap();
    assert!(!result.is_ok());
    assert_eq!(
        result.coverage().vendor_rulepacks,
        ["vendor/spotify-ingest"]
    );
    let violations: Vec<_> = result.reports.iter().flat_map(|r| &r.violations).collect();
    assert!(
        violations
            .iter()
            .any(|v| v.code == "core.privacy.gdpr_invalid")
    );
    assert!(violations.iter().any(
        |v| v.code == "vendor.spotify-ingest.body.body.value.invalid"
            && v.severity == pixellint_core::Severity::Warning
    ));
}
