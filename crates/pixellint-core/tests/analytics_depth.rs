//! Vendor contract regressions cover wrong JSON types and documented boundaries.

use std::{fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct ContractCase {
    id: String,
    kind: String,
    artifact: String,
    code: String,
    expect_violation: bool,
    expect_no_errors: bool,
    source_url: String,
    #[serde(default, alias = "reference_time")]
    reference_time_unix_seconds: Option<i64>,
}

fn check_case(engine: &Engine, name: &str, case: ContractCase) -> Option<String> {
    assert!(case.source_url.starts_with("https://"));
    let request = ValidationRequest {
        artifact_kind: match case.kind.as_str() {
            "url" => ArtifactKind::Url,
            "json" => ArtifactKind::JsonPayload,
            "request" => ArtifactKind::NetworkRequest,
            other => panic!("unsupported fixture kind {other}"),
        },
        artifact: case.artifact,
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    };
    let options = ValidationOptions {
        only_rulepacks: vec![format!("vendor/{name}")],
        except_rulepacks: Vec::new(),
    };
    let summary = match case.reference_time_unix_seconds {
        Some(reference_time) => engine.validate_at(&request, &options, reference_time),
        None => engine.validate(&request, &options),
    }
    .unwrap();
    let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
    let count = violations.iter().filter(|v| v.code == case.code).count();
    if (count > 0) != case.expect_violation || (case.expect_no_errors && !summary.is_ok()) {
        Some(format!(
            "{name}/{}: expected {} for {}, saw {count}, clean {}; reports {:?}",
            case.id, case.expect_violation, case.code, case.expect_no_errors, summary.reports
        ))
    } else {
        None
    }
}

#[test]
fn analytics_vendor_contracts_reject_wrong_types_and_respect_boundaries() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    let names = [
        "adobe-web-sdk",
        "amplitude",
        "braze",
        "brevo",
        "google-ads-call-conversions",
        "google-ads-click-conversions",
        "google-ads-conversion-adjustments",
        "google-analytics",
        "heap-account-properties",
        "heap-identify",
        "heap-track",
        "heap-user-properties",
        "intercom-events",
        "klaviyo",
        "mixpanel-engage",
        "mixpanel-groups",
        "mixpanel-import",
        "mixpanel",
        "plausible",
        "posthog",
        "rudderstack",
        "segment",
    ];
    let mut checked = 0;
    let mut failures = Vec::new();
    for name in names {
        let file = root.join(format!("vendor-{name}/depth-cases.json"));
        let cases: Vec<ContractCase> =
            serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
        for case in cases {
            if let Some(failure) = check_case(&engine, name, case) {
                failures.push(failure);
            }
            checked += 1;
        }
    }
    assert!(checked > 1000);
    assert!(
        failures.is_empty(),
        "{} contract regressions failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn analytics_independent_vendor_examples_and_dependencies() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    let mut checked = 0;
    let mut failures = Vec::new();
    for dir in fs::read_dir(root).unwrap() {
        let dir = dir.unwrap().path();
        let file = dir.join("spec-cases.json");
        if !file.exists() {
            continue;
        }
        let name = dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("vendor-")
            .unwrap();
        let cases: Vec<ContractCase> =
            serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
        for case in cases {
            if let Some(failure) = check_case(&engine, name, case) {
                failures.push(failure);
            }
            checked += 1;
        }
    }
    assert!(checked >= 83);
    assert!(
        failures.is_empty(),
        "{} vendor regressions failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn analytics_existing_transport_fixtures_preserve_selection_and_findings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let engine = Engine::default();
    let names = [
        "adobe-analytics",
        "adobe-dcs-event",
        "adobe-dcs-id",
        "adobe-ecid",
        "adobe-web-sdk",
        "amplitude-group-identify",
        "amplitude-identify",
        "amplitude",
        "braze",
        "brevo-js",
        "brevo",
        "chartbeat",
        "cm360-tracking-ad",
        "cm360-vast-event",
        "floodlight",
        "google-ad-manager",
        "google-ads-call-conversions",
        "google-ads-click-conversions",
        "google-ads-conversion-adjustments",
        "google-ads-conversion",
        "google-analytics-collect",
        "google-analytics",
        "google-tag-manager",
        "heap-account-properties",
        "heap-classic",
        "heap-identify",
        "heap-track",
        "heap-user-properties",
        "heap",
        "intercom-events",
        "intercom",
        "klaviyo",
        "matomo",
        "mixpanel-engage",
        "mixpanel-groups",
        "mixpanel-import",
        "mixpanel",
        "parsely-collect",
        "parsely",
        "plausible",
        "posthog",
        "rudderstack",
        "segment",
        "yandex-metrica",
        "yandex-watch",
    ];
    let mut checked = 0;
    let mut failures = Vec::new();
    for name in names {
        let directory = root.join(format!("vendor-{name}"));
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(directory.join("manifest.json")).unwrap())
                .unwrap();
        for case in cases {
            let request = ValidationRequest {
                artifact_kind: match case["kind"].as_str().unwrap() {
                    "url" => ArtifactKind::Url,
                    "json" => ArtifactKind::JsonPayload,
                    "request" => ArtifactKind::NetworkRequest,
                    kind => panic!("unsupported fixture kind {kind}"),
                },
                artifact: fs::read_to_string(directory.join(case["fixture"].as_str().unwrap()))
                    .unwrap(),
                claimed_vendor: case["claimed_vendor"].as_str().map(str::to_owned),
                expansion_state: if case["expansion_state"].as_str() == Some("template") {
                    ExpansionState::Template
                } else {
                    ExpansionState::Unknown
                },
            };
            let options = ValidationOptions {
                only_rulepacks: serde_json::from_value(
                    case.get("rulepacks")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!([])),
                )
                .unwrap(),
                except_rulepacks: serde_json::from_value(
                    case.get("except_rulepacks")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!([])),
                )
                .unwrap(),
            };
            let summary = match case["reference_time_unix_seconds"]
                .as_i64()
                .or_else(|| case["reference_time"].as_i64())
            {
                Some(reference_time) => engine.validate_at(&request, &options, reference_time),
                None => engine.validate(&request, &options),
            }
            .unwrap();
            let violations: Vec<_> = summary
                .reports
                .iter()
                .flat_map(|report| &report.violations)
                .collect();
            let actual = serde_json::json!({
                "expected_plugins": summary.reports.iter().map(|report| &report.plugin_id).collect::<Vec<_>>(),
                "expected_ok": summary.is_ok(),
                "expected_errors": violations.iter().filter(|v| v.severity == Severity::Error).count(),
                "expected_warnings": violations.iter().filter(|v| v.severity == Severity::Warning).count(),
                "expected_infos": violations.iter().filter(|v| v.severity == Severity::Info).count(),
                "expected_codes": violations.iter().map(|v| &v.code).collect::<Vec<_>>(),
            });
            if actual
                .as_object()
                .unwrap()
                .iter()
                .any(|(key, value)| case.get(key).is_some_and(|expected| expected != value))
            {
                failures.push(format!("{name}/{}: actual {actual}", case["id"]));
            }
            checked += 1;
        }
    }
    assert!(checked >= 436);
    assert!(
        failures.is_empty(),
        "{} transport regressions failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn adobe_dcs_and_ecid_query_discriminators_preserve_documented_surfaces() {
    let engine = Engine::default();
    for (artifact, pack) in [
        (
            "https://dpm.demdex.net/id?d_cid=123%01987",
            "vendor/adobe-dcs-id",
        ),
        (
            "https://dpm.demdex.net/id?d_dpid=123&d_dpuuid=987",
            "vendor/adobe-dcs-id",
        ),
        (
            "https://dpm.demdex.net/id?d_ver=2&d_orgid=EXAMPLE%40AdobeOrg",
            "vendor/adobe-ecid",
        ),
        (
            "https://dpm.demdex.net/id?d_mid=existing&d_ver=2",
            "vendor/adobe-ecid",
        ),
        ("https://dpm.demdex.net/id?d_%76er=", "vendor/adobe-ecid"),
        (
            "https://yourcompany.demdex.net/event?d_cid=123%01987",
            "vendor/adobe-dcs-event",
        ),
        (
            "https://use.demdex.net/event?d_mid=existing",
            "vendor/adobe-dcs-event",
        ),
    ] {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let summary = engine
            .validate(&request, &ValidationOptions::default())
            .unwrap();
        let selected: Vec<_> = summary
            .reports
            .iter()
            .map(|r| r.plugin_id.as_str())
            .collect();
        assert!(selected.contains(&pack), "{artifact}: {selected:?}");
        let other = if pack == "vendor/adobe-ecid" {
            "vendor/adobe-dcs-id"
        } else {
            "vendor/adobe-ecid"
        };
        assert!(!selected.contains(&other), "{artifact}: {selected:?}");
    }
    let request = ValidationRequest {
        artifact_kind: ArtifactKind::Url,
        artifact: "https://dpm.demdex.net/id?d_cid=123%01987".to_string(),
        claimed_vendor: None,
        expansion_state: ExpansionState::Unknown,
    };
    let options = ValidationOptions {
        only_rulepacks: vec!["vendor/adobe-ecid".to_string()],
        except_rulepacks: Vec::new(),
    };
    let summary = engine.validate(&request, &options).unwrap();
    assert!(
        summary
            .reports
            .iter()
            .flat_map(|r| &r.violations)
            .any(|v| v.code == "vendor.adobe-ecid.param.d_ver.missing")
    );
}

#[test]
fn posthog_event_shape_preserves_native_destination_routing() {
    let engine = Engine::default();
    let cases = [
        (r#"{"event":{"data":{"custom":"value"}}}"#, false),
        (r#"{"event":"pageview"}"#, true),
        (r#"{"event":""}"#, true),
        (r#"{"batch":[{"event":"pageview"}]}"#, true),
        (
            r#"{"batch":[{"event":{"data":{"custom":"value"}}}]}"#,
            false,
        ),
    ];
    for (artifact, expected_posthog) in cases {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::JsonPayload,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };
        let summary = engine
            .validate(&request, &ValidationOptions::default())
            .unwrap();
        assert_eq!(
            summary
                .reports
                .iter()
                .any(|report| report.plugin_id == "vendor/posthog"),
            expected_posthog,
            "PostHog routing disagrees for {artifact}"
        );
    }
}
