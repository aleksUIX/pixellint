//! Engine-wide behavior lock.
//!
//! Golden fixtures already pin findings per pack. These tests pin how the
//! engine chooses packs, how it fails, and how `validate_many` relates to
//! `validate`, so a later hot-path rewrite cannot change observable results.

use pixellint_core::{
    ArtifactKind, DIRECTORY_ID, DocumentArtifactInput, DocumentRequest, Engine, EngineError,
    ExpansionState, ManifestRulePack, ValidationOptions, ValidationRequest, ValidatorPlugin,
};

fn request(kind: ArtifactKind, artifact: &str) -> ValidationRequest {
    ValidationRequest {
        artifact_kind: kind,
        artifact: artifact.to_string(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    }
}

fn url(artifact: &str) -> ValidationRequest {
    request(ArtifactKind::Url, artifact)
}

fn json_body(artifact: &str) -> ValidationRequest {
    request(ArtifactKind::JsonPayload, artifact)
}

fn plugin_ids(summary: &pixellint_core::ValidationSummary) -> Vec<&str> {
    summary
        .reports
        .iter()
        .map(|report| report.plugin_id.as_str())
        .collect()
}

fn codes(summary: &pixellint_core::ValidationSummary) -> Vec<&str> {
    summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
        .map(|violation| violation.code.as_str())
        .collect()
}

const META_PIXEL: &str = "https://www.facebook.com/tr?id=1234567890123456&ev=PageView";
const TIKTOK_PIXEL: &str = "https://analytics.tiktok.com/i18n/pixel/events.js?sdkid=CBBQ1234567890";
const FLOODLIGHT: &str =
    "https://ad.doubleclick.net/ddm/activity/src=1234567;type=convr0;cat=purch0;ord=8675309";
const UNKNOWN: &str = "https://pixel.example.com/collect?id=1";
const META_CAPI: &str =
    include_str!("../../../fixtures/vendor-meta-conversions-api/clean-server-event-payload.txt");
const TIKTOK_CAPI: &str =
    include_str!("../../../fixtures/vendor-tiktok-events-api/clean-track.txt");

#[test]
fn meta_pixel_selects_core_and_meta_only() {
    let summary = Engine::default()
        .validate(&url(META_PIXEL), &ValidationOptions::default())
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core", "vendor/meta"]);
    assert!(summary.is_ok());
}

#[test]
fn host_matching_is_case_insensitive_and_keeps_label_boundaries() {
    let engine = Engine::default();
    let options = ValidationOptions::default();

    let upper = engine
        .validate(
            &url("https://WWW.FACEBOOK.COM/tr?id=1234567890123456&ev=PageView"),
            &options,
        )
        .unwrap();
    assert_eq!(plugin_ids(&upper), ["core", "vendor/meta"]);

    let with_port = engine
        .validate(
            &url("https://www.facebook.com:443/tr?id=1234567890123456&ev=PageView"),
            &options,
        )
        .unwrap();
    assert_eq!(plugin_ids(&with_port), ["core", "vendor/meta"]);

    let with_userinfo = engine
        .validate(
            &url("https://user:pass@www.facebook.com/tr?id=1234567890123456&ev=PageView"),
            &options,
        )
        .unwrap();
    assert!(plugin_ids(&with_userinfo).contains(&"vendor/meta"));

    for artifact in [
        "https://notfacebook.com/tr?id=1234567890123456&ev=PageView",
        "https://facebook.com.evil.example/tr?id=1234567890123456&ev=PageView",
        "https://www.facebook.com.evil.example/tr?id=1234567890123456&ev=PageView",
    ] {
        let summary = engine.validate(&url(artifact), &options).unwrap();
        assert!(
            !plugin_ids(&summary).contains(&"vendor/meta"),
            "{artifact} must not claim the Meta pack: {:?}",
            plugin_ids(&summary)
        );
    }
}

#[test]
fn suffix_hosts_match_the_apex_and_subdomains() {
    let engine = Engine::default();
    let options = ValidationOptions::default();
    let summary = engine
        .validate(
            &url("https://example.sc.omtrdc.net/b/ss/examplersid/1/JS-2.22.0/s12345?AQB=1&mid=1234567890&pageName=Home&AQE=1"),
            &options,
        )
        .unwrap();
    assert!(
        plugin_ids(&summary).contains(&"vendor/adobe-analytics"),
        "{:?}",
        plugin_ids(&summary)
    );
}

#[test]
fn vendor_urls_do_not_cross_select() {
    let engine = Engine::default();
    let options = ValidationOptions::default();

    let meta = engine.validate(&url(META_PIXEL), &options).unwrap();
    assert!(!plugin_ids(&meta).contains(&"vendor/tiktok"));
    assert!(!plugin_ids(&meta).contains(&"vendor/floodlight"));

    let tiktok = engine.validate(&url(TIKTOK_PIXEL), &options).unwrap();
    assert!(plugin_ids(&tiktok).contains(&"vendor/tiktok"));
    assert!(!plugin_ids(&tiktok).contains(&"vendor/meta"));

    let floodlight = engine.validate(&url(FLOODLIGHT), &options).unwrap();
    assert!(plugin_ids(&floodlight).contains(&"vendor/floodlight"));
    assert!(!plugin_ids(&floodlight).contains(&"vendor/meta"));
}

#[test]
fn unknown_host_runs_core_only() {
    let summary = Engine::default()
        .validate(&url(UNKNOWN), &ValidationOptions::default())
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core"]);
}

#[test]
fn ipv6_and_localhost_stay_on_core() {
    let engine = Engine::default();
    let options = ValidationOptions::default();
    for artifact in [
        "https://[2001:db8::1]/pixel?id=1",
        "https://localhost:8443/pixel?id=1",
        "https://127.0.0.1/pixel?id=1",
    ] {
        let summary = engine.validate(&url(artifact), &options).unwrap();
        assert_eq!(plugin_ids(&summary), ["core"], "{artifact}");
        assert!(summary.is_ok(), "{artifact}");
    }
}

#[test]
fn macros_in_query_still_select_by_host() {
    let summary = Engine::default()
        .validate(
            &url("https://www.facebook.com/tr?id=[PIXEL_ID]&ev=Purchase&cd[value]=[VALUE]"),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core", "vendor/meta"]);
}

#[test]
fn json_payloads_select_by_shape_not_host() {
    let engine = Engine::default();
    let options = ValidationOptions::default();

    let meta = engine.validate(&json_body(META_CAPI), &options).unwrap();
    assert!(
        plugin_ids(&meta).contains(&"vendor/meta-conversions-api"),
        "{:?}",
        plugin_ids(&meta)
    );
    assert!(!plugin_ids(&meta).contains(&"vendor/tiktok-events-api"));

    let tiktok = engine.validate(&json_body(TIKTOK_CAPI), &options).unwrap();
    assert!(
        plugin_ids(&tiktok).contains(&"vendor/tiktok-events-api"),
        "{:?}",
        plugin_ids(&tiktok)
    );
    assert!(!plugin_ids(&tiktok).contains(&"vendor/meta-conversions-api"));
}

#[test]
fn unknown_kind_routes_json_bodies_and_urls() {
    let engine = Engine::default();
    let options = ValidationOptions::default();

    let as_url = engine
        .validate(&request(ArtifactKind::Unknown, META_PIXEL), &options)
        .unwrap();
    assert!(plugin_ids(&as_url).contains(&"vendor/meta"));

    let as_json = engine
        .validate(&request(ArtifactKind::Unknown, META_CAPI), &options)
        .unwrap();
    assert!(plugin_ids(&as_json).contains(&"vendor/meta-conversions-api"));
}

#[test]
fn url_like_kinds_share_selection_for_the_same_artifact() {
    let engine = Engine::default();
    let options = ValidationOptions::default();
    let mut sets = Vec::new();
    for kind in [
        ArtifactKind::Url,
        ArtifactKind::VastTracker,
        ArtifactKind::ServerPostback,
        ArtifactKind::NetworkRequest,
    ] {
        let summary = engine
            .validate(&request(kind, META_PIXEL), &options)
            .unwrap();
        sets.push(plugin_ids(&summary).join(","));
    }
    assert!(sets.iter().all(|set| set == &sets[0]), "{sets:?}");
}

#[test]
fn claimed_vendor_does_not_change_which_packs_run() {
    let engine = Engine::default();
    let mut claimed = url(META_PIXEL);
    claimed.claimed_vendor = Some("google".to_string());
    let summary = engine
        .validate(&claimed, &ValidationOptions::default())
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core", "vendor/meta"]);
    assert!(codes(&summary).contains(&"vendor.meta.claimed_vendor_mismatch"));
}

#[test]
fn except_and_only_toggles_still_force_packs() {
    let engine = Engine::default();

    let except = engine
        .validate(
            &url(META_PIXEL),
            &ValidationOptions {
                only_rulepacks: Vec::new(),
                except_rulepacks: vec!["vendor/meta".to_string()],
            },
        )
        .unwrap();
    assert!(!plugin_ids(&except).contains(&"vendor/meta"));
    assert!(plugin_ids(&except).contains(&"core"));
    assert!(plugin_ids(&except).contains(&DIRECTORY_ID));

    let only = engine
        .validate(
            &url(UNKNOWN),
            &ValidationOptions {
                only_rulepacks: vec!["vendor/meta".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(plugin_ids(&only), ["vendor/meta"]);
    assert!(codes(&only).contains(&"vendor.meta.endpoint_mismatch"));
}

#[test]
fn unknown_rulepack_id_is_an_error() {
    let error = Engine::default()
        .validate(
            &url(UNKNOWN),
            &ValidationOptions {
                only_rulepacks: vec!["vendor/does-not-exist".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(
        error,
        EngineError::PluginNotFound("vendor/does-not-exist".to_string())
    );
}

#[test]
fn an_empty_engine_has_no_matching_plugin() {
    let error = Engine::new()
        .validate(&url(UNKNOWN), &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(error, EngineError::NoMatchingPlugin);
}

#[test]
fn a_late_registered_manifest_is_selected() {
    let mut engine = Engine::default();
    engine
        .register_manifest_json(
            r#"{
                "id": "custom/acme",
                "display_name": "Acme pixel",
                "description": "Coverage lock for engine registration.",
                "vendor": "acme",
                "source_level": "heuristic",
                "match": { "hosts": ["px.acme.test"], "paths": ["/collect"] },
                "params": [{ "name": "aid", "requirement": "required" }]
            }"#,
        )
        .unwrap();

    let summary = engine
        .validate(
            &url("https://px.acme.test/collect?other=1"),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(plugin_ids(&summary).contains(&"custom/acme"));
    assert!(codes(&summary).contains(&"custom.acme.param.aid.missing"));
}

#[test]
fn a_custom_plugin_without_hosts_still_runs_in_auto_mode() {
    let mut engine = Engine::default();

    struct AlwaysOn;
    impl ValidatorPlugin for AlwaysOn {
        fn metadata(&self) -> &pixellint_core::RulePackMetadata {
            use std::sync::OnceLock;
            static META: OnceLock<pixellint_core::RulePackMetadata> = OnceLock::new();
            META.get_or_init(|| pixellint_core::RulePackMetadata {
                id: "custom/always".to_string(),
                display_name: "Always".to_string(),
                version: "0.0.0".to_string(),
                description: "Coverage lock for unindexed plugins.".to_string(),
                source_level: pixellint_core::RuleSourceLevel::Heuristic,
                vendor: None,
            })
        }

        fn supports(&self, _request: &ValidationRequest) -> bool {
            true
        }

        fn validate(&self, _request: &ValidationRequest) -> pixellint_core::ValidationReport {
            pixellint_core::ValidationReport {
                plugin_id: "custom/always".to_string(),
                detected_vendor: None,
                violations: Vec::new(),
            }
        }
    }

    engine.register(AlwaysOn);
    let summary = engine
        .validate(&url(UNKNOWN), &ValidationOptions::default())
        .unwrap();
    assert!(plugin_ids(&summary).contains(&"custom/always"));
}

#[test]
fn invalid_json_is_a_core_finding_and_claims_no_vendor_pack() {
    let summary = Engine::default()
        .validate(
            &json_body(r#"{"data":[{"event_name":}]}"#),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core"]);
    assert_eq!(codes(&summary), ["core.json.parse_error"]);
}

#[test]
fn html_snippets_do_not_select_vendor_packs_from_embedded_urls() {
    let summary = Engine::default()
        .validate(
            &request(
                ArtifactKind::HtmlSnippet,
                r#"<script src="https://www.facebook.com/tr?id=1"></script>"#,
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core"]);
}

#[test]
fn fat_query_strings_keep_the_same_pack() {
    let mut artifact = String::from("https://www.facebook.com/tr?id=1234567890123456&ev=PageView");
    for index in 0..200 {
        artifact.push_str("&p");
        artifact.push_str(&index.to_string());
        artifact.push_str("=x");
    }
    let summary = Engine::default()
        .validate(&url(&artifact), &ValidationOptions::default())
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["core", "vendor/meta"]);
}

#[test]
fn validate_many_matches_per_item_validate_after_dedupe() {
    let engine = Engine::default();
    let options = ValidationOptions::default();
    let artifacts = [
        META_PIXEL,
        META_PIXEL,
        UNKNOWN,
        "https://example.com/pixel?id=1#frag",
    ];

    let document = DocumentRequest {
        document_kind: "vast".to_string(),
        extractor: None,
        artifacts: artifacts
            .iter()
            .map(|artifact| DocumentArtifactInput {
                artifact_kind: ArtifactKind::Url,
                artifact: artifact.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Unknown,
                occurrences: Vec::new(),
            })
            .collect(),
    };

    let many = engine.validate_many(&document, &options).unwrap();
    assert_eq!(many.summary.artifacts_total, Some(4));
    assert_eq!(many.summary.unique_artifacts, Some(3));

    for unique in &many.artifacts {
        let single = engine
            .validate(&url(&unique.normalized_artifact), &options)
            .unwrap();
        assert_eq!(unique.reports, single.reports, "{}", unique.artifact_id);
    }
}

#[test]
fn privacy_rules_still_run_on_unclaimed_hosts() {
    let summary = Engine::default()
        .validate(
            &url("https://example.com/px?gdpr=1"),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert_eq!(codes(&summary), ["core.privacy.gdpr_consent_missing"]);
}

#[test]
fn explicit_pack_on_a_json_mismatch_reports_payload_mismatch() {
    let summary = Engine::default()
        .validate(
            &json_body(TIKTOK_CAPI),
            &ValidationOptions {
                only_rulepacks: vec!["vendor/meta-conversions-api".to_string()],
                except_rulepacks: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(plugin_ids(&summary), ["vendor/meta-conversions-api"]);
    assert!(codes(&summary).contains(&"vendor.meta-conversions-api.payload_mismatch"));
}

#[test]
fn independent_manifest_supports_agrees_with_the_engine_for_selection_cases() {
    let engine = Engine::default();
    let options = ValidationOptions::default();
    let cases = [
        url(META_PIXEL),
        url(TIKTOK_PIXEL),
        url(FLOODLIGHT),
        url(UNKNOWN),
        url("https://WWW.FACEBOOK.COM/tr?id=1234567890123456&ev=PageView"),
        json_body(META_CAPI),
        json_body(TIKTOK_CAPI),
        request(ArtifactKind::VastTracker, META_PIXEL),
        request(ArtifactKind::Unknown, META_CAPI),
    ];

    let packs: Vec<(&str, ManifestRulePack)> = pixellint_core::BUILTIN_VENDOR_MANIFESTS
        .iter()
        .map(|(id, json)| (*id, ManifestRulePack::from_json(json).unwrap()))
        .collect();

    for case in cases {
        let summary = engine.validate(&case, &options).unwrap();
        let engine_ids: Vec<&str> = summary
            .reports
            .iter()
            .filter(|report| report.plugin_id != DIRECTORY_ID)
            .map(|report| report.plugin_id.as_str())
            .collect();

        let mut independent = Vec::new();
        independent.push("core");
        for (id, pack) in &packs {
            if pack.supports(&case) {
                independent.push(*id);
            }
        }
        independent.sort();
        let mut sorted_engine = engine_ids;
        sorted_engine.sort();
        assert_eq!(sorted_engine, independent, "{}", case.artifact);
    }
}
