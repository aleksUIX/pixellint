//! Consent and privacy signal checks.
//!
//! These rules live in `core` rather than in a vendor pack because the signals
//! are vendor-neutral: IAB Tech Lab specifies them, every endpoint in the chain
//! is expected to carry them, and the failure modes are the same everywhere.
//!
//! Every rule here cites the spec that defines the parameter. Values carrying an
//! unexpanded macro are skipped, because `gdpr=${GDPR}` in a template is correct
//! trafficking, not a malformed signal. The playground storage sentinel
//! `REDACTED` is skipped the same way: it is a scrubbed copy, not a consent
//! string, and must not be decoded as TCF, USP, or GPP.

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

/// Playground D1 replaces consent strings with this sentinel before storage.
fn is_redacted_sentinel(value: &str) -> bool {
    value.eq_ignore_ascii_case("REDACTED")
}

fn skip_signal_value(value: &str) -> bool {
    contains_macro(value) || is_redacted_sentinel(value)
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
        Some(param) if !skip_signal_value(&param.value) && !param.value.is_empty() => {
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
        (Some(false), Some(param))
            if !param.value.is_empty() && !skip_signal_value(&param.value) =>
        {
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
        && !skip_signal_value(&param.value)
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
        } else {
            check_tc_string_contents(param, violations);
        }
    }
}

/// Reads the fields the TC String spec fixes, once the value is known to be
/// base64.
///
/// The alphabet check above passes anything spelled with base64 characters, and
/// plenty of things are: `gdpr_consent=1` is a well-formed base64 segment that
/// decodes to nothing resembling consent. What separates a TC String from a
/// string is the Version field the spec pins to 2, and enough length to hold the
/// fields it lists as mandatory.
fn check_tc_string_contents(param: &RawParam, violations: &mut Vec<Violation>) {
    // Only the core segment is fixed. Optional segments follow a dot and carry
    // their own layout.
    let core = param.value.split('.').next().unwrap_or_default();

    match read_bits(core, 0, 6) {
        Some(2) => {}
        Some(1) => {
            violations.push(violation(
                "core.privacy.tc_string_version",
                "`gdpr_consent` carries a TCF v1 TC String. IAB Tech Lab sunset v1 on 15 August 2020, and v2 vendors cannot read it."
                    .to_string(),
                Severity::Error,
                "param.gdpr_consent",
                "Take the TC String from a TCF v2 CMP.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![param.target()],
            ));
            return;
        }
        Some(version) => {
            violations.push(violation(
                "core.privacy.tc_string_version",
                format!(
                    "`gdpr_consent` is `{}`, whose first six bits decode to version {version}. The TC String spec fixes the version to 2, so this is not a TC String.",
                    truncate(&param.value)
                ),
                Severity::Error,
                "param.gdpr_consent",
                "Pass the TC String from the CMP. A placeholder such as `1` is base64-shaped but carries no consent.",
                source("IAB Tech Lab TCF v2", TCF_SPEC),
                vec![param.target()],
            ));
            return;
        }
        None => return,
    }

    if core.len() < TC_CORE_MIN_CHARS {
        violations.push(violation(
            "core.privacy.tc_string_truncated",
            format!(
                "`gdpr_consent` has a {}-character core segment. The mandatory fields through PublisherCC need {TC_CORE_MANDATORY_BITS} bits, so a TC String cannot be shorter than {TC_CORE_MIN_CHARS} characters.",
                core.len()
            ),
            Severity::Error,
            "param.gdpr_consent",
            "Pass the whole TC String. Truncation usually comes from a length limit on a macro or a database column.",
            source("IAB Tech Lab TCF v2", TCF_SPEC),
            vec![param.target()],
        ));
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

    // D1 stores `us_privacy=REDACTED`. The param was present (deprecated still
    // applies); the sentinel is not a USP string to format-check.
    if is_redacted_sentinel(&param.value) {
        return;
    }

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
    } else if !param.value.starts_with('1') {
        // The shape is right, so the leading character is a version claim rather
        // than noise. Only version 1 was ever published.
        violations.push(violation(
            "core.privacy.us_privacy_version",
            format!(
                "`us_privacy` claims specification version `{}`. Only version 1 was published before the signal was deprecated.",
                &param.value[..1]
            ),
            Severity::Warning,
            "param.us_privacy",
            "Send `1` as the first character, or move to `gpp` and `gpp_sid`.",
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
        && !skip_signal_value(&param.value)
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
        } else {
            check_gpp_header(param, violations);
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
        && !skip_signal_value(&param.value)
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

/// Reads the GPP header, which the spec pins to a fixed type and version.
///
/// The header is the first section, and its first six bits are "fixed to 3 as
/// GPP Header field". That makes a value in `gpp` that is not a GPP string
/// cheap to spot: a TC String pasted into the wrong parameter decodes to type 2
/// and stops here.
fn check_gpp_header(param: &RawParam, violations: &mut Vec<Violation>) {
    let header = param.value.split('~').next().unwrap_or_default();

    match read_bits(header, 0, 6) {
        Some(3) => {}
        Some(kind) => {
            violations.push(violation(
                "core.privacy.gpp_header_type",
                format!(
                    "`gpp` is `{}`, whose header decodes to type {kind}. The GPP spec fixes the header type to 3, so this is not a GPP string.",
                    truncate(&param.value)
                ),
                Severity::Error,
                "param.gpp",
                "Send the GPP string from the CMP. A TC String belongs in `gdpr_consent`, not here.",
                source("IAB Tech Lab GPP", GPP_SPEC),
                vec![param.target()],
            ));
            return;
        }
        None => return,
    }

    if let Some(version) = read_bits(header, 6, 6)
        && version != 1
    {
        violations.push(violation(
            "core.privacy.gpp_header_version",
            format!(
                "`gpp` declares GPP specification version {version}. Version 1 is the published one, and a callee reading the header will stop at an unknown version."
            ),
            Severity::Warning,
            "param.gpp",
            "Send a version 1 GPP string, which starts `DB`.",
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

/// The value of one URL-safe base64 character, as the 6 bits it stands for.
fn base64url_bits(character: u8) -> Option<u32> {
    Some(match character {
        b'A'..=b'Z' => u32::from(character - b'A'),
        b'a'..=b'z' => u32::from(character - b'a') + 26,
        b'0'..=b'9' => u32::from(character - b'0') + 52,
        b'-' => 62,
        b'_' => 63,
        _ => return None,
    })
}

/// Reads a big-endian bit field out of a URL-safe base64 segment.
///
/// The consent specs describe their strings as bit fields that happen to be
/// base64 for transport, so reading a field means going back to the bits rather
/// than decoding to bytes.
fn read_bits(segment: &str, start: usize, length: usize) -> Option<u64> {
    if length == 0 || length > 64 {
        return None;
    }

    let bytes = segment.as_bytes();
    let mut value: u64 = 0;

    for offset in start..start + length {
        let character = *bytes.get(offset / 6)?;
        let bits = base64url_bits(character)?;
        let bit = (bits >> (5 - (offset % 6))) & 1;
        value = (value << 1) | u64::from(bit);
    }

    Some(value)
}

/// Bits the core segment needs for the fields the spec lists as mandatory, from
/// Version through PublisherCC. The segment carries more after that, so this is
/// a floor rather than a size.
const TC_CORE_MANDATORY_BITS: usize = 213;

/// Characters those bits take once encoded.
const TC_CORE_MIN_CHARS: usize = TC_CORE_MANDATORY_BITS.div_ceil(6);

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
            codes("https://example.com/px?gdpr=1&gdpr_consent=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAAAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA").is_empty()
        );
        assert!(codes("https://example.com/px?gdpr=0").is_empty());
    }

    #[test]
    fn gdpr_flag_values_are_constrained() {
        assert_eq!(
            codes(
                "https://example.com/px?gdpr=true&gdpr_consent=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA"
            ),
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
            codes(
                "https://example.com/px?gdpr_consent=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA"
            ),
            vec!["core.privacy.gdpr_consent_without_flag"]
        );
    }

    #[test]
    fn a_tc_string_outside_gdpr_is_informational() {
        assert_eq!(
            codes(
                "https://example.com/px?gdpr=0&gdpr_consent=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA"
            ),
            vec!["core.privacy.gdpr_consent_ignored"]
        );
    }

    #[test]
    fn a_placeholder_in_the_consent_slot_is_not_a_tc_string() {
        // Base64 characters, so the alphabet check passes it. The version field
        // is what gives it away.
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=1"),
            ["core.privacy.tc_string_version"]
        );
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=true"),
            ["core.privacy.tc_string_version"]
        );
    }

    #[test]
    fn a_tcf_v1_string_is_reported_as_sunset() {
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=BOxxRfAOxxRfAAfKABENAAAAAAAAoAAA"),
            ["core.privacy.tc_string_version"]
        );
    }

    #[test]
    fn a_truncated_tc_string_is_flagged() {
        // Right version, too short to hold the fields the spec makes mandatory.
        assert_eq!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=CPXxRfAPXxRf"),
            ["core.privacy.tc_string_truncated"]
        );
    }

    #[test]
    fn a_us_privacy_version_other_than_one_is_flagged() {
        assert_eq!(
            codes("https://example.com/px?us_privacy=2YNN"),
            [
                "core.privacy.us_privacy_deprecated",
                "core.privacy.us_privacy_version"
            ]
        );
    }

    #[test]
    fn a_tc_string_in_the_gpp_slot_is_flagged() {
        // The commonest way to get this wrong is to put the right string in the
        // wrong parameter. The header type says so.
        assert_eq!(
            codes(
                "https://example.com/px?gpp=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA&gpp_sid=2"
            ),
            ["core.privacy.gpp_header_type"]
        );
    }

    #[test]
    fn an_unknown_gpp_version_is_flagged() {
        assert_eq!(
            codes("https://example.com/px?gpp=DCABMA&gpp_sid=2"),
            ["core.privacy.gpp_header_version"]
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
    fn redacted_storage_sentinels_are_not_decoded_as_consent_strings() {
        assert!(
            codes("https://example.com/px?gdpr=1&gdpr_consent=REDACTED").is_empty(),
            "REDACTED is not a TC String"
        );
        assert_eq!(
            codes("https://example.com/px?us_privacy=REDACTED"),
            vec!["core.privacy.us_privacy_deprecated"]
        );
        assert!(
            codes("https://example.com/px?gpp=REDACTED").is_empty(),
            "REDACTED is not a GPP string"
        );
        assert!(
            codes(
                "https://beacons.extremereach.io/duration?gdpr_consent=REDACTED&us_privacy=REDACTED&gpp=REDACTED"
            )
            .iter()
            .all(|code| code == "core.privacy.us_privacy_deprecated")
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
            codes(
                "https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA~1YNN"
            ),
            vec!["core.privacy.gpp_sid_missing"]
        );
        assert!(codes("https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA~1YNN&gpp_sid=2").is_empty());
        assert!(codes("https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA&gpp_sid=2,6").is_empty());
    }

    #[test]
    fn gpp_values_are_format_checked() {
        assert_eq!(
            codes("https://example.com/px?gpp=not a gpp string&gpp_sid=2"),
            vec!["core.privacy.gpp_malformed"]
        );
        assert_eq!(
            codes(
                "https://example.com/px?gpp=DBACNYA~CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA&gpp_sid=usnat"
            ),
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
            codes(
                "https://example.com/px?gdpr=1&gdpr=0&gdpr_consent=CPXxRfAPXxRfAAfKABENB-CgAAAAAAAAAAYgAAAAAAAA"
            ),
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
