//! Official tracker templates provide local boundaries, not remote acceptance.

use std::{fs, path::PathBuf};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest,
};
use serde::Deserialize;

const GOOGLE_PACK: &str = "vendor/google-cm360-pcs-view";
const ADCTV_PACK: &str = "vendor/adctv-tracker";
const IAS_WARNING: &str = "vendor.ias-video-pixel.xsid.unstable_macro";
const GOOGLE_SOURCE: &str = "https://support.google.com/campaignmanager/answer/9858133?hl=en";
const ADCTV_SOURCE: &str = "https://ads.adctv.com/scripts/src/tag.ins/tag.ins.js";
const IAS_SOURCE: &str = "https://assets.ctfassets.net/o1orzsgogjpz/4Q8TPW9OcTv2lg35DRf3DX/7a0bd2d4564e4fea7cff97cb6137088e/IAS_Video_Solutions_Guide_.pdf";

fn engine() -> Engine {
    let mut engine = Engine::default();
    engine
        .register_manifest_json(include_str!(
            "../rulepacks/vendor/google-cm360-pcs-view.json"
        ))
        .unwrap();
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/adctv-tracker.json"))
        .unwrap();
    engine
}

fn request(
    artifact: &str,
    artifact_kind: ArtifactKind,
    state: ExpansionState,
) -> ValidationRequest {
    ValidationRequest {
        artifact: artifact.to_owned(),
        artifact_kind,
        claimed_vendor: None,
        expansion_state: state,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCase {
    id: String,
    kind: String,
    fixture: String,
    reference_time: i64,
    expected_plugins: Vec<String>,
    expected_ok: bool,
    expected_errors: usize,
    expected_warnings: usize,
    expected_infos: usize,
    expected_codes: Vec<String>,
    source_urls: Vec<String>,
    source_access: String,
    source_requirement: String,
    case_origin: String,
    source_artifacts: Vec<SourceArtifact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceArtifact {
    url: String,
    sha256: String,
}

#[test]
fn cm360_pcs_view_source_cases_keep_templates_and_scope_separate() {
    source_cases("google-cm360-pcs-view", GOOGLE_PACK, GOOGLE_SOURCE);
}

#[test]
fn adctv_source_cases_keep_events_open_and_claim_only_the_root_endpoint() {
    source_cases("adctv-tracker", ADCTV_PACK, ADCTV_SOURCE);
}

fn source_cases(name: &str, pack_id: &str, source: &str) {
    let engine = engine();
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/vendor-{name}"));
    let cases: Vec<SourceCase> =
        serde_json::from_str(&fs::read_to_string(directory.join("source-cases.json")).unwrap())
            .unwrap();

    for case in cases {
        assert_eq!(case.kind, "url");
        assert_eq!(case.source_urls, [source]);
        assert!(!case.source_access.is_empty());
        assert!(!case.source_requirement.is_empty());
        assert_eq!(
            case.case_origin,
            "independent_primary_requirement_regression"
        );
        for artifact in &case.source_artifacts {
            assert_eq!(artifact.url, source);
            assert_eq!(artifact.sha256.len(), 64);
            assert!(artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
        let summary = engine
            .validate_at(
                &request(
                    &fs::read_to_string(directory.join(&case.fixture)).unwrap(),
                    ArtifactKind::Url,
                    ExpansionState::Unknown,
                ),
                &ValidationOptions::default(),
                case.reference_time,
            )
            .unwrap();
        assert_eq!(summary.is_ok(), case.expected_ok, "{} validity", case.id);
        assert_eq!(
            summary
                .reports
                .iter()
                .map(|r| r.plugin_id.clone())
                .collect::<Vec<_>>(),
            case.expected_plugins,
            "{} selection",
            case.id
        );
        let violations: Vec<_> = summary.reports.iter().flat_map(|r| &r.violations).collect();
        assert_eq!(
            violations
                .iter()
                .map(|v| v.code.clone())
                .collect::<Vec<_>>(),
            case.expected_codes,
            "{} findings",
            case.id
        );
        for (severity, expected) in [
            (Severity::Error, case.expected_errors),
            (Severity::Warning, case.expected_warnings),
            (Severity::Info, case.expected_infos),
        ] {
            assert_eq!(
                violations.iter().filter(|v| v.severity == severity).count(),
                expected,
                "{} severity",
                case.id
            );
        }
        for violation in summary
            .reports
            .iter()
            .filter(|r| r.plugin_id == pack_id)
            .flat_map(|r| &r.violations)
        {
            assert_eq!(violation.severity, Severity::Warning);
            assert_eq!(violation.source.level, RuleSourceLevel::OfficialTemplate);
            assert_eq!(violation.source.reference.as_deref(), Some(source));
        }
    }
}

#[test]
fn adctv_root_producer_does_not_silence_non_iab_vast_macros() {
    let engine = engine();
    let url = "https://track.adctv.com/?event=impression&cachebuster=[AV_TIMESTAMP]&gdpr=[AV_GDPR]";
    for state in [ExpansionState::Unknown, ExpansionState::Template] {
        let summary = engine
            .validate(
                &request(url, ArtifactKind::VastTracker, state),
                &ValidationOptions::default(),
            )
            .unwrap();
        let adctv = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == ADCTV_PACK)
            .unwrap();
        assert!(adctv.violations.is_empty());
        let core = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == "core")
            .unwrap();
        assert!(
            core.violations
                .iter()
                .any(|v| v.code == "core.macro.vast_unknown")
        );
        assert!(summary.is_ok());
    }
    let capture = serde_json::json!({
        "url": "https://track.adctv.com/?event=impression",
        "method": "POST",
        "headers": [
            {"name": "Accept", "value": "application/json"},
            {"name": "Content-Type", "value": "application/json"}
        ]
    })
    .to_string();
    let summary = engine
        .validate(
            &request(
                &capture,
                ArtifactKind::NetworkRequest,
                ExpansionState::Unknown,
            ),
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(summary.reports.iter().any(|r| r.plugin_id == ADCTV_PACK));
    assert!(summary.is_ok());
}

#[test]
fn cm360_pcs_view_applies_to_vast_urls_and_captured_requests() {
    let engine = engine();
    let url = "https://googleads4.g.doubleclick.net/pcs/view?xai=opaque-xai&sai=opaque-sai&sig=opaque-sig&adurl=";
    let capture = serde_json::json!({
        "url": url,
        "method": "GET",
        "headers": []
    })
    .to_string();
    for (kind, artifact) in [
        (ArtifactKind::VastTracker, url),
        (ArtifactKind::NetworkRequest, capture.as_str()),
    ] {
        let summary = engine
            .validate(
                &request(artifact, kind, ExpansionState::Unknown),
                &ValidationOptions::default(),
            )
            .unwrap();
        let report = summary
            .reports
            .iter()
            .find(|r| r.plugin_id == GOOGLE_PACK)
            .unwrap();
        assert!(report.violations.is_empty());
        assert!(summary.is_ok());
    }
}

#[test]
fn ias_warns_only_on_exact_unstable_session_macros_in_each_expansion_state() {
    let engine = engine();
    for state in [
        ExpansionState::Unknown,
        ExpansionState::Template,
        ExpansionState::Fired,
    ] {
        for value in [
            "[CACHEBUSTING]",
            "[TIMESTAMP]",
            "${CACHEBUSTER}",
            "%5BTIMESTAMP%5D",
        ] {
            let summary = engine
                .validate(
                    &request(
                        &format!("https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId={value}"),
                        ArtifactKind::VastTracker,
                        state,
                    ),
                    &ValidationOptions::default(),
                )
                .unwrap();
            let report = summary
                .reports
                .iter()
                .find(|r| r.plugin_id == "vendor/ias-video-pixel")
                .unwrap();
            assert_eq!(report.violations.len(), 1, "{state:?} {value}");
            let violation = &report.violations[0];
            assert_eq!(violation.code, IAS_WARNING);
            assert_eq!(violation.severity, Severity::Warning);
            assert_eq!(violation.targets.len(), 1);
            assert_eq!(violation.targets[0].name.as_deref(), Some("xsId"));
            assert_eq!(violation.source.level, RuleSourceLevel::OfficialVendor);
            assert_eq!(violation.source.reference.as_deref(), Some(IAS_SOURCE));
        }
    }
}

#[test]
fn ias_preserves_resolved_ids_session_macros_and_wrong_field_controls() {
    let engine = engine();
    for state in [
        ExpansionState::Unknown,
        ExpansionState::Template,
        ExpansionState::Fired,
    ] {
        for query in [
            "xsId=1791496800",
            "xsId=session-TIMESTAMP-123",
            "xsId=session-123&campaign_id=[TIMESTAMP]",
            "xsId=[TRANSACTIONID]",
            "xsId=${AD_SESSION_ID}",
            "xsId=session-123&xsid=[TIMESTAMP]",
            "xsId=session-123&redirect=https%3A%2F%2Fexample.test%2F%3FxsId%3D%5BTIMESTAMP%5D",
        ] {
            let summary = engine
                .validate(
                    &request(
                        &format!("https://unified.adsafeprotected.com/vevent/start/1111111/66666666?{query}"),
                        ArtifactKind::VastTracker,
                        state,
                    ),
                    &ValidationOptions::default(),
                )
                .unwrap();
            let report = summary
                .reports
                .iter()
                .find(|r| r.plugin_id == "vendor/ias-video-pixel")
                .unwrap();
            assert!(
                report.violations.is_empty(),
                "{state:?} {query}: {:?}",
                report.violations
            );
        }
    }
}
