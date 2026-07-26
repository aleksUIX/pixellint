//! Consent and privacy signal checks.
//!
//! These rules live in `core` rather than in a vendor pack because the signals
//! are vendor-neutral: IAB Tech Lab specifies them, every endpoint in the chain
//! is expected to carry them, and the failure modes are the same everywhere.
//!
//! Every rule here cites the spec that defines the parameter. Values carrying an
//! unexpanded macro are skipped, because `gdpr=${GDPR}` in a template is correct
//! trafficking, not a malformed signal.

use crate::manifest::{ParamStyle, RawParam, contains_macro, extract_params};
use crate::{RuleSource, RuleSourceLevel, Severity, Violation, ViolationTarget};

const TCF_SPEC: &str = "https://github.com/InteractiveAdvertisingBureau/GDPR-Transparency-and-Consent-Framework/blob/master/TCFv2/IAB%20Tech%20Lab%20-%20Consent%20string%20and%20vendor%20list%20formats%20v2.md";
const USP_SPEC: &str = "https://github.com/InteractiveAdvertisingBureau/USPrivacy/blob/master/CCPA/US%20Privacy%20String.md";
const GPP_SPEC: &str = "https://github.com/InteractiveAdvertisingBureau/Global-Privacy-Platform/blob/main/Core/Consent%20String%20Specification.md";

/// Parameters the specs say must appear at most once in a URL.
const SIGNAL_PARAMS: [&str; 5] = ["gdpr", "gdpr_consent", "us_privacy", "gpp", "gpp_sid"];

pub(crate) fn apply_privacy_rules(artifact: &str, violations: &mut Vec<Violation>) {
    // Consent signals ride in the query on most pixels and in the path on
    // Floodlight-style tags, so both styles have to be read.
    let mut params = extract_params(artifact, ParamStyle::Query);
    params.extend(extract_params(artifact, ParamStyle::Matrix));

    if params.is_empty() {
        return;
    }

    check_duplicates(&params, violations);
    check_tcf(artifact, &params, violations);
    check_us_privacy(&params, violations);
    check_gpp(artifact, &params, violations);
}

fn find<'a>(params: &'a [RawParam], name: &str) -> Option<&'a RawParam> {
    params.iter().find(|param| param.name == name)
}

fn source(name: &str, reference: &str) -> RuleSource {
    RuleSource {
        level: RuleSourceLevel::Normative,
        name: name.to_string(),
        reference: Some(reference.to_string()),
    }
}

fn violation(
    code: &str,
    message: String,
    severity: Severity,
    field: &str,
    fix_hint: &str,
    source: RuleSource,
    targets: Vec<ViolationTarget>,
) -> Violation {
    Violation {
        code: code.to_string(),
        message,
        severity,
        field: Some(field.to_string()),
        fix_hint: Some(fix_hint.to_string()),
        source,
        targets,
    }
}

/// Both the TCF and GPP specs tell URL creators to add each signal exactly once.
/// A repeated signal leaves the callee choosing which copy to believe.
fn check_duplicates(params: &[RawParam], violations: &mut Vec<Violation>) {
    for name in SIGNAL_PARAMS {
        let matches: Vec<&RawParam> = params.iter().filter(|param| param.name == name).collect();

        if matches.len() > 1 {
            violations.push(violation(
                "core.privacy.duplicate_signal",
                format!(
                    "`{name}` appears {} times. A privacy signal must appear once, or the callee has to guess which copy is authoritative.",
                    matches.len()
                ),
                Severity::Warning,
                &format!("param.{name}"),
                &format!("Keep one `{name}` parameter and drop the rest."),
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                matches.iter().map(|param| param.target()).collect(),
            ));
        }
    }
}

fn check_tcf(artifact: &str, params: &[RawParam], violations: &mut Vec<Violation>) {
    let gdpr = find(params, "gdpr");
    let consent = find(params, "gdpr_consent");

    // An empty signal is a template waiting to be filled by an ad server, which
    // is how Floodlight and VAST tags ship. Only a populated value is a claim.
    let applies = match gdpr {
        Some(param) if !contains_macro(&param.value) && !param.value.is_empty() => {
            match param.value.as_str() {
                "0" => Some(false),
                "1" => Some(true),
                other => {
                    violations.push(violation(
                        "core.privacy.gdpr_invalid",
                        format!(
                            "`gdpr` is `{other}`. The TCF specifies `0` when GDPR does not apply and `1` when it does."
                        ),
                        Severity::Error,
                        "param.gdpr",
                        "Send `gdpr=1` in scope of GDPR, `gdpr=0` outside it.",
                        source("IAB Tech Lab TCF v2", TCF_SPEC),
                        vec![param.target()],
                    ));
                    None
                }
            }
        }
        _ => None,
    };

    match (applies, consent) {
        (Some(true), None) => {
            violations.push(violation(
                "core.privacy.gdpr_consent_missing",
                "`gdpr=1` says GDPR applies, but the request carries no `gdpr_consent`. The callee has no TC String to check a legal basis against.".to_string(),
                Severity::Error,
                "param.gdpr_consent",
                "Add `gdpr_consent=${GDPR_CONSENT_XXXXX}` with the receiving vendor's GVL ID, and expand it before firing.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![whole_url_target(artifact)],
            ));
        }
        (Some(true), Some(param)) if param.value.is_empty() => {
            violations.push(violation(
                "core.privacy.gdpr_consent_missing",
                "`gdpr=1` says GDPR applies, but `gdpr_consent` is empty.".to_string(),
                Severity::Error,
                "param.gdpr_consent",
                "Populate `gdpr_consent` with the TC String obtained from the CMP.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![param.target()],
            ));
        }
        (Some(false), Some(param)) if !param.value.is_empty() && !contains_macro(&param.value) => {
            violations.push(violation(
                "core.privacy.gdpr_consent_ignored",
                "`gdpr=0` says GDPR does not apply, so the TC String in `gdpr_consent` is not meaningful for this call.".to_string(),
                Severity::Info,
                "param.gdpr_consent",
                "Leave `gdpr_consent` out when `gdpr=0`, or check that the flag is right.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![param.target()],
            ));
        }
        _ => {}
    }

    if let Some(param) = consent
        && !param.value.is_empty()
        && !contains_macro(&param.value)
    {
        if gdpr.is_none() {
            violations.push(violation(
                "core.privacy.gdpr_consent_without_flag",
                "`gdpr_consent` is present but `gdpr` is not, so the callee cannot tell whether GDPR applies to this call.".to_string(),
                Severity::Warning,
                "param.gdpr",
                "Send `gdpr=1` alongside the TC String, or `gdpr=0` when GDPR does not apply.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![param.target()],
            ));
        }

        if !is_tc_string(&param.value) {
            violations.push(violation(
                "core.privacy.gdpr_consent_malformed",
                format!(
                    "`gdpr_consent` is `{}`, which is not a URL-safe base64 TC String.",
                    truncate(&param.value)
                ),
                Severity::Warning,
                "param.gdpr_consent",
                "Pass the TC String from the CMP unmodified, without re-encoding it.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![param.target()],
            ));
        }
    }
}

fn check_us_privacy(params: &[RawParam], violations: &mut Vec<Violation>) {
    let Some(param) = find(params, "us_privacy") else {
        return;
    };

    if contains_macro(&param.value) || param.value.is_empty() {
        return;
    }

    violations.push(violation(
        "core.privacy.us_privacy_deprecated",
        "`us_privacy` carries the US Privacy signal, which IAB Tech Lab deprecated on 31 January 2024 in favor of the Global Privacy Platform.".to_string(),
        Severity::Warning,
        "param.us_privacy",
        "Move to `gpp` and `gpp_sid`. Sending both during a migration is fine.",
        source("IAB Tech Lab US Privacy String", USP_SPEC),
        vec![param.target()],
    ));

    if !is_us_privacy_string(&param.value) {
        violations.push(violation(
            "core.privacy.us_privacy_malformed",
            format!(
                "`us_privacy` is `{}`. The string is a version digit followed by three characters, each `Y`, `N`, or `-`, for notice, opt-out of sale, and LSPA coverage.",
                truncate(&param.value)
            ),
            Severity::Error,
            "param.us_privacy",
            "Send a value such as `1YNN`, or `1---` when no US privacy jurisdiction applies.",
            source("IAB Tech Lab US Privacy String", USP_SPEC),
            vec![param.target()],
        ));
    }
}

fn check_gpp(artifact: &str, params: &[RawParam], violations: &mut Vec<Violation>) {
    let gpp = find(params, "gpp");
    let sid = find(params, "gpp_sid");

    if let Some(param) = gpp
        && !param.value.is_empty()
        && !contains_macro(&param.value)
    {
        if !is_gpp_string(&param.value) {
            violations.push(violation(
                "core.privacy.gpp_malformed",
                format!(
                    "`gpp` is `{}`, which is not a URL-safe base64 GPP string. Sections are separated by `~`.",
                    truncate(&param.value)
                ),
                Severity::Warning,
                "param.gpp",
                "Pass the GPP string from the CMP unmodified.",
                source("IAB Tech Lab GPP", GPP_SPEC),
                vec![param.target()],
            ));
        }

        if sid.is_none() {
            violations.push(violation(
                "core.privacy.gpp_sid_missing",
                "`gpp` is present without `gpp_sid`, so the callee does not know which section of the string is in force.".to_string(),
                Severity::Warning,
                "param.gpp_sid",
                "Add `gpp_sid=${GPP_SID}` next to the GPP string.",
                source("IAB Tech Lab GPP", GPP_SPEC),
                vec![whole_url_target(artifact)],
            ));
        }
    }

    if let Some(param) = sid
        && !param.value.is_empty()
        && !contains_macro(&param.value)
        && !is_gpp_sid(&param.value)
    {
        violations.push(violation(
            "core.privacy.gpp_sid_malformed",
            format!(
                "`gpp_sid` is `{}`. It carries the section IDs in force, normally one, at most two separated by a comma.",
                truncate(&param.value)
            ),
            Severity::Warning,
            "param.gpp_sid",
            "Send the section ID the CMP reports, such as `gpp_sid=2`.",
            source("IAB Tech Lab GPP", GPP_SPEC),
            vec![param.target()],
        ));
    }
}

fn whole_url_target(artifact: &str) -> ViolationTarget {
    ViolationTarget {
        component: crate::ViolationTargetComponent::WholeUrl,
        name: None,
        value: None,
        start: 0,
        end: artifact.len(),
    }
}

fn truncate(value: &str) -> String {
    if value.chars().count() <= 32 {
        return value.to_string();
    }

    let head: String = value.chars().take(32).collect();
    format!("{head}...")
}

fn is_base64url_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

/// A TC String is URL-safe base64. Optional segments are appended after `.`.
fn is_tc_string(value: &str) -> bool {
    value.split('.').all(is_base64url_segment)
}

/// A GPP string is URL-safe base64 with sections separated by `~`.
fn is_gpp_string(value: &str) -> bool {
    value
        .split('~')
        .all(|section| section.split('.').all(is_base64url_segment))
}

fn is_us_privacy_string(value: &str) -> bool {
    let bytes = value.as_bytes();

    bytes.len() == 4
        && bytes[0].is_ascii_digit()
        && bytes[1..]
            .iter()
            .all(|byte| matches!(byte.to_ascii_uppercase(), b'Y' | b'N' | b'-'))
}

fn is_gpp_sid(value: &str) -> bool {
    let sections: Vec<&str> = value.split(',').collect();

    sections.len() <= 2
        && sections
            .iter()
            .all(|section| !section.is_empty() && section.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use crate::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};

    fn codes(artifact: &str) -> Vec<String> {
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };

        Engine::default()
            .validate(&request, &ValidationOptions::default())
            .expect("validate")
            .reports
            .iter()
            .flat_map(|report| report.violations.iter())
            .map(|violation| violation.code.clone())
            .filter(|code| code.starts_with("core.privacy."))
            .collect()
    }

    #[test]
    fn a_complete_tcf_signal_is_clean() {
        assert!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA").is_empty()
        );
        assert!(codes("https://example.com/px?gdpr=0").is_empty());
    }

    #[test]
    fn gdpr_flag_values_are_constrained() {
        assert_eq!(
            codes("https://example.com/px?gdpr=true&gdpr_consent=CPXxRfAPXxRf"),
            vec!["core.privacy.gdpr_invalid"]
        );
    }

    #[test]
    fn gdpr_applying_without_a_tc_string_is_an_error() {
        assert_eq!(
            codes("https://example.com/px?gdpr=1"),
            vec!["core.privacy.gdpr_consent_missing"]
        );
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr_consent="),
            vec!["core.privacy.gdpr_consent_missing"]
        );
    }

    #[test]
    fn a_tc_string_without_a_flag_is_ambiguous() {
        assert_eq!(
            codes("https://example.com/px?gdpr_consent=CPXxRfAPXxRf"),
            vec!["core.privacy.gdpr_consent_without_flag"]
        );
    }

    #[test]
    fn a_tc_string_outside_gdpr_is_informational() {
        assert_eq!(
            codes("https://example.com/px?gdpr=0&gdpr_consent=CPXxRfAPXxRf"),
            vec!["core.privacy.gdpr_consent_ignored"]
        );
    }

    #[test]
    fn malformed_tc_strings_are_flagged() {
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=not a tc string"),
            vec!["core.privacy.gdpr_consent_malformed"]
        );
    }

    #[test]
    fn unexpanded_macros_are_left_to_the_macro_rules() {
        assert!(
            codes("https://example.com/px?gdpr=${GDPR}&gdpr_consent=${GDPR_CONSENT_123}")
                .is_empty()
        );
        assert!(codes("https://example.com/px?us_privacy=${US_PRIVACY}").is_empty());
        assert!(
            codes("https://example.com/px?gpp=${GPP_STRING_123}&gpp_sid=${GPP_SID}").is_empty()
        );
    }

    #[test]
    fn us_privacy_is_deprecated_and_format_checked() {
        assert_eq!(
            codes("https://example.com/px?us_privacy=1YNN"),
            vec!["core.privacy.us_privacy_deprecated"]
        );
        assert_eq!(
            codes("https://example.com/px?us_privacy=YES"),
            vec![
                "core.privacy.us_privacy_deprecated",
                "core.privacy.us_privacy_malformed"
            ]
        );
        assert_eq!(
            codes("https://example.com/px?us_privacy=1---"),
            vec!["core.privacy.us_privacy_deprecated"]
        );
    }

    #[test]
    fn gpp_needs_a_section_id() {
        assert_eq!(
            codes("https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRf~1YNN"),
            vec!["core.privacy.gpp_sid_missing"]
        );
        assert!(codes("https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRf~1YNN&gpp_sid=2").is_empty());
        assert!(codes("https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRf&gpp_sid=2,6").is_empty());
    }

    #[test]
    fn gpp_values_are_format_checked() {
        assert_eq!(
            codes("https://example.com/px?gpp=not a gpp string&gpp_sid=2"),
            vec!["core.privacy.gpp_malformed"]
        );
        assert_eq!(
            codes("https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRf&gpp_sid=usnat"),
            vec!["core.privacy.gpp_sid_malformed"]
        );
        assert_eq!(
            codes("https://example.com/px?gpp=DBACNYA&gpp_sid=2,6,8"),
            vec!["core.privacy.gpp_sid_malformed"]
        );
    }

    #[test]
    fn repeated_signals_are_flagged_once_per_parameter() {
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr=0&gdpr_consent=CPXxRfAPXxRf"),
            vec!["core.privacy.duplicate_signal"]
        );
    }

    #[test]
    fn signals_carried_on_the_path_are_checked_too() {
        assert_eq!(
            codes("https://ad.doubleclick.net/ddm/activity/src=123;type=a;cat=b;gdpr=1;ord=1?"),
            vec!["core.privacy.gdpr_consent_missing"]
        );
    }

    #[test]
    fn empty_signals_are_treated_as_unfilled_templates() {
        assert!(
            codes(
                "https://ad.doubleclick.net/ddm/activity/src=123;type=a;cat=b;dc_rdid=;tfua=;npa=;gdpr=;gdpr_consent=;ord=1?"
            )
            .is_empty()
        );
    }

    #[test]
    fn artifacts_without_privacy_signals_are_untouched() {
        assert!(codes("https://example.com/px?id=1&ev=PageView").is_empty());
    }
}
