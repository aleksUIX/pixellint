//! Destination-supported consent names remain local to the selected endpoint.

use pixellint_core::{
    ArtifactKind, CoreRulePack, Engine, ExpansionState, Severity, ValidationOptions,
    ValidationRequest, ValidationSummary, Violation,
};
use serde_json::json;

const TCF: &str = "CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA";
const ENDPOINT: &str = "https://eb2.3lift.com/getuid?redir=https%3A%2F%2Fpublisher.example%2Fsync";

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.register(CoreRulePack::default());
    engine
        .register_manifest_json(include_str!("../rulepacks/vendor/triplelift-sync.json"))
        .unwrap();
    engine
}

fn validate(engine: &Engine, artifact: &str, options: &ValidationOptions) -> ValidationSummary {
    engine
        .validate_at(
            &ValidationRequest {
                artifact_kind: ArtifactKind::Url,
                artifact: artifact.to_string(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            options,
            1_800_000_000,
        )
        .unwrap()
}

fn core(summary: &ValidationSummary) -> Vec<&Violation> {
    summary
        .reports
        .iter()
        .filter(|report| report.plugin_id == "core")
        .flat_map(|report| &report.violations)
        .collect()
}

fn has(summary: &ValidationSummary, code: &str) -> bool {
    core(summary).iter().any(|finding| finding.code == code)
}

#[test]
fn documented_alias_can_supply_consent_without_changing_default_names() {
    let engine = engine();
    let options = ValidationOptions::default();
    let alias = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=1&cmp_cs={TCF}"),
        &options,
    );
    assert!(core(&alias).is_empty());
    let canonical = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=1&gdpr_consent={TCF}"),
        &options,
    );
    assert!(core(&canonical).is_empty());
    let unknown = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=1&CMP_CS={TCF}"),
        &options,
    );
    assert!(has(&unknown, "core.privacy.gdpr_consent_missing"));
    let encoded = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=1&%63mp_cs={TCF}"),
        &options,
    );
    assert!(core(&encoded).is_empty());
}

#[test]
fn aliases_require_the_matching_host_path_and_selected_pack() {
    let engine = engine();
    let forced = ValidationOptions {
        only_rulepacks: vec!["core".into(), "vendor/triplelift-sync".into()],
        except_rulepacks: vec![],
    };
    for url in [
        format!("https://unrelated.example/getuid?gdpr=1&cmp_cs={TCF}"),
        format!("https://eb2.3lift.com/not-the-sync-path?gdpr=1&cmp_cs={TCF}"),
        format!("https://noteb2.3lift.com/getuid?gdpr=1&cmp_cs={TCF}"),
    ] {
        assert!(has(
            &validate(&engine, &url, &ValidationOptions::default()),
            "core.privacy.gdpr_consent_missing"
        ));
        assert!(has(
            &validate(&engine, &url, &forced),
            "core.privacy.gdpr_consent_missing"
        ));
    }
    let url = format!("{ENDPOINT}&gdpr=1&cmp_cs={TCF}");
    let excluded = ValidationOptions {
        only_rulepacks: vec![],
        except_rulepacks: vec!["vendor/triplelift-sync".into()],
    };
    assert!(has(
        &validate(&engine, &url, &excluded),
        "core.privacy.gdpr_consent_missing"
    ));
    let core_only = ValidationOptions {
        only_rulepacks: vec!["core".into()],
        except_rulepacks: vec![],
    };
    assert!(has(
        &validate(&engine, &url, &core_only),
        "core.privacy.gdpr_consent_missing"
    ));
    let mut no_vendor = Engine::new();
    no_vendor.register(CoreRulePack::default());
    assert!(has(
        &validate(&no_vendor, &url, &ValidationOptions::default()),
        "core.privacy.gdpr_consent_missing"
    ));
}

#[test]
fn empty_alias_and_absent_alias_keep_the_missing_consent_error() {
    let engine = engine();
    for extra in ["", "&cmp_cs=", "&cmp_cs=&gdpr_consent="] {
        let summary = validate(
            &engine,
            &format!("{ENDPOINT}&gdpr=1{extra}"),
            &ValidationOptions::default(),
        );
        assert!(has(&summary, "core.privacy.gdpr_consent_missing"));
        assert_eq!(
            core(&summary)
                .iter()
                .filter(|finding| finding.code == "core.privacy.gdpr_consent_missing")
                .count(),
            1
        );
    }
    let summary = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=1&gdpr_consent=&cmp_cs={TCF}"),
        &ValidationOptions::default(),
    );
    assert!(!has(&summary, "core.privacy.gdpr_consent_missing"));
    assert_eq!(
        core(&summary)
            .iter()
            .filter(|finding| finding.code == "core.privacy.duplicate_signal")
            .count(),
        1
    );
}

#[test]
fn every_literal_carrier_is_validated_and_duplicates_form_one_group() {
    let engine = engine();
    for extra in [
        format!("&gdpr_consent={TCF}&cmp_cs=not%20a%20tc"),
        format!("&gdpr_consent=not%20a%20tc&cmp_cs={TCF}"),
        format!("&cmp_cs={TCF}&cmp_cs=not%20a%20tc"),
        format!("&gdpr_consent={TCF}&gdpr_consent=not%20a%20tc&cmp_cs={TCF}"),
    ] {
        let summary = validate(
            &engine,
            &format!("{ENDPOINT}&gdpr=1{extra}"),
            &ValidationOptions::default(),
        );
        let findings = core(&summary);
        assert!(has(&summary, "core.privacy.gdpr_consent_malformed"));
        let duplicate: Vec<_> = findings
            .iter()
            .filter(|finding| finding.code == "core.privacy.duplicate_signal")
            .collect();
        assert_eq!(duplicate.len(), 1);
        assert_eq!(duplicate[0].severity, Severity::Warning);
        assert_eq!(duplicate[0].targets.len(), extra.matches('=').count());
    }
}

#[test]
fn aliases_keep_their_original_names_and_encoded_byte_spans() {
    let engine = engine();
    let artifact = format!("{ENDPOINT}&gdpr=1&cmp_cs=not%20a%20tc");
    let summary = validate(&engine, &artifact, &ValidationOptions::default());
    let finding = core(&summary)
        .into_iter()
        .find(|finding| finding.code == "core.privacy.gdpr_consent_malformed")
        .unwrap();
    assert_eq!(finding.targets.len(), 1);
    assert_eq!(finding.targets[0].name.as_deref(), Some("cmp_cs"));
    assert_eq!(
        &artifact[finding.targets[0].start..finding.targets[0].end],
        "cmp_cs=not%20a%20tc"
    );
    let version = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=1&cmp_cs=1"),
        &ValidationOptions::default(),
    );
    let finding = core(&version)
        .into_iter()
        .find(|finding| finding.code == "core.privacy.tc_string_version")
        .unwrap();
    assert_eq!(finding.targets[0].name.as_deref(), Some("cmp_cs"));
}

#[test]
fn macros_and_redaction_defer_only_their_own_carrier() {
    let engine = engine();
    for value in ["${GDPR_CONSENT_28}", "REDACTED", "redacted"] {
        let summary = validate(
            &engine,
            &format!("{ENDPOINT}&gdpr=1&cmp_cs={value}"),
            &ValidationOptions::default(),
        );
        assert!(
            !core(&summary)
                .iter()
                .any(|finding| finding.code.starts_with("core.privacy."))
        );
        let summary = validate(
            &engine,
            &format!("{ENDPOINT}&gdpr=1&cmp_cs={value}&gdpr_consent=1"),
            &ValidationOptions::default(),
        );
        assert!(has(&summary, "core.privacy.tc_string_version"));
        assert!(has(&summary, "core.privacy.duplicate_signal"));
    }
    let summary = validate(
        &engine,
        &format!("{ENDPOINT}&gdpr=0&cmp_cs={TCF}"),
        &ValidationOptions::default(),
    );
    assert!(has(&summary, "core.privacy.gdpr_consent_ignored"));
    let summary = validate(
        &engine,
        &format!("{ENDPOINT}&cmp_cs={TCF}"),
        &ValidationOptions::default(),
    );
    assert!(has(&summary, "core.privacy.gdpr_consent_without_flag"));
}

#[test]
fn complete_requests_preserve_alias_context_without_synthetic_targets() {
    let engine = engine();
    let url = format!("{ENDPOINT}&gdpr=1&cmp_cs={TCF}");
    let capture = json!({"url":url,"method":"GET","headers":{}}).to_string();
    let summary = engine
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: capture,
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
        )
        .unwrap();
    assert!(!has(&summary, "core.privacy.gdpr_consent_missing"));
    let artifact =
        json!({"url":format!("{ENDPOINT}&gdpr=1&cmp_cs=not%20a%20tc"),"method":"GET","headers":{}})
            .to_string();
    let summary = engine
        .validate(
            &ValidationRequest {
                artifact_kind: ArtifactKind::NetworkRequest,
                artifact: artifact.clone(),
                claimed_vendor: None,
                expansion_state: ExpansionState::Fired,
            },
            &ValidationOptions::default(),
        )
        .unwrap();
    let finding = core(&summary)
        .into_iter()
        .find(|finding| finding.code == "core.privacy.gdpr_consent_malformed")
        .unwrap();
    assert!(
        finding.targets.is_empty(),
        "complete captures must not expose normalized offsets"
    );
}
