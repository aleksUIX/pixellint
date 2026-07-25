use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use url::Url;

pub mod manifest;

pub use manifest::{
    Assertion, ManifestError, ManifestRulePack, MatchSpec, PackRule, ParamContract, ParamStyle,
    Requirement, RulePackManifest, ValueFormat,
};

/// First-party vendor rulepacks compiled into the crate, as (id, manifest JSON).
///
/// Every pack here is a declarative manifest, the same format users write for
/// their own packs. Nothing about a first-party pack is privileged.
pub const BUILTIN_VENDOR_MANIFESTS: &[(&str, &str)] = &[
    (
        "vendor/floodlight",
        include_str!("../rulepacks/vendor/floodlight.json"),
    ),
    (
        "vendor/google-analytics",
        include_str!("../rulepacks/vendor/google-analytics.json"),
    ),
    (
        "vendor/linkedin",
        include_str!("../rulepacks/vendor/linkedin.json"),
    ),
    ("vendor/meta", include_str!("../rulepacks/vendor/meta.json")),
    (
        "vendor/tiktok",
        include_str!("../rulepacks/vendor/tiktok.json"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Url,
    #[serde(rename = "html")]
    HtmlSnippet,
    #[serde(rename = "js")]
    JavaScriptSnippet,
    #[serde(rename = "gtm")]
    GtmTemplate,
    #[serde(rename = "request")]
    NetworkRequest,
    #[serde(rename = "vast")]
    VastTracker,
    #[serde(rename = "postback")]
    ServerPostback,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionState {
    #[default]
    Unknown,
    Template,
    Fired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSourceLevel {
    Normative,
    OfficialVendor,
    OfficialTemplate,
    EcosystemReference,
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleSource {
    pub level: RuleSourceLevel,
    pub name: String,
    pub reference: Option<String>,
}

impl RuleSource {
    pub fn normative(name: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            level: RuleSourceLevel::Normative,
            name: name.into(),
            reference: Some(reference.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationTargetComponent {
    WholeUrl,
    Scheme,
    Authority,
    UserInfo,
    Host,
    Port,
    Path,
    QueryParam,
    Fragment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViolationTarget {
    pub component: ViolationTargetComponent,
    pub name: Option<String>,
    pub value: Option<String>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    pub code: String,
    pub message: String,
    pub severity: Severity,
    pub field: Option<String>,
    pub fix_hint: Option<String>,
    pub source: RuleSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<ViolationTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulePackMetadata {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub source_level: RuleSourceLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationRequest {
    pub artifact_kind: ArtifactKind,
    pub artifact: String,
    pub claimed_vendor: Option<String>,
    pub expansion_state: ExpansionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ValidationOptions {
    pub only_rulepacks: Vec<String>,
    pub except_rulepacks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub plugin_id: String,
    pub detected_vendor: Option<String>,
    pub violations: Vec<Violation>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.violations
            .iter()
            .all(|violation| violation.severity != Severity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationSummary {
    pub reports: Vec<ValidationReport>,
}

impl ValidationSummary {
    pub fn is_ok(&self) -> bool {
        self.reports.iter().all(ValidationReport::is_ok)
    }
}

pub trait ValidatorPlugin: Send + Sync {
    fn metadata(&self) -> &RulePackMetadata;
    fn supports(&self, request: &ValidationRequest) -> bool;
    fn validate(&self, request: &ValidationRequest) -> ValidationReport;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    PluginNotFound(String),
    NoMatchingPlugin,
    NoRulepacksSelected,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PluginNotFound(plugin_id) => {
                write!(f, "rulepack not found: {plugin_id}")
            }
            Self::NoMatchingPlugin => write!(f, "no matching rulepack found"),
            Self::NoRulepacksSelected => write!(f, "no rulepacks remain after applying toggles"),
        }
    }
}

impl Error for EngineError {}

pub struct Engine {
    plugins: BTreeMap<String, Arc<dyn ValidatorPlugin>>,
}

impl Default for Engine {
    /// An engine with the `core` pack and every first-party vendor pack
    /// registered. Built-in manifests are compiled in tests, so a malformed one
    /// is a build-time bug rather than a runtime surprise.
    fn default() -> Self {
        let mut engine = Self::new();
        engine.register(CoreRulePack::default());

        for (id, json) in BUILTIN_VENDOR_MANIFESTS {
            engine.register_manifest_json(json).unwrap_or_else(|error| {
                panic!("built-in rulepack `{id}` failed to compile: {error}")
            });
        }

        engine
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            plugins: BTreeMap::new(),
        }
    }

    pub fn register<P>(&mut self, plugin: P)
    where
        P: ValidatorPlugin + 'static,
    {
        self.plugins
            .insert(plugin.metadata().id.clone(), Arc::new(plugin));
    }

    /// Compiles a declarative rulepack manifest and registers it.
    pub fn register_manifest_json(&mut self, json: &str) -> Result<(), ManifestError> {
        self.register(ManifestRulePack::from_json(json)?);
        Ok(())
    }

    /// Compiles a declarative rulepack manifest from disk and registers it.
    pub fn register_manifest_path(&mut self, path: impl AsRef<Path>) -> Result<(), ManifestError> {
        self.register(ManifestRulePack::from_path(path)?);
        Ok(())
    }

    pub fn list_rulepacks(&self) -> Vec<RulePackMetadata> {
        self.plugins
            .values()
            .map(|plugin| plugin.metadata().clone())
            .collect()
    }

    pub fn validate(
        &self,
        request: &ValidationRequest,
        options: &ValidationOptions,
    ) -> Result<ValidationSummary, EngineError> {
        let plugins = self.select_plugins(request, options)?;
        let reports = plugins
            .into_iter()
            .map(|plugin| plugin.validate(request))
            .collect();

        Ok(ValidationSummary { reports })
    }

    fn select_plugins(
        &self,
        request: &ValidationRequest,
        options: &ValidationOptions,
    ) -> Result<Vec<Arc<dyn ValidatorPlugin>>, EngineError> {
        self.ensure_known_rulepacks(&options.only_rulepacks)?;
        self.ensure_known_rulepacks(&options.except_rulepacks)?;

        let excluded: BTreeSet<&str> = options
            .except_rulepacks
            .iter()
            .map(String::as_str)
            .collect();

        if !options.only_rulepacks.is_empty() {
            let selected = options
                .only_rulepacks
                .iter()
                .filter(|rulepack_id| !excluded.contains(rulepack_id.as_str()))
                .filter_map(|rulepack_id| self.plugins.get(rulepack_id).map(Arc::clone))
                .collect::<Vec<_>>();

            if selected.is_empty() {
                return Err(EngineError::NoRulepacksSelected);
            }

            return Ok(selected);
        }

        let selected = self
            .plugins
            .values()
            .filter(|plugin| !excluded.contains(plugin.metadata().id.as_str()))
            .filter(|plugin| plugin.supports(request))
            .map(Arc::clone)
            .collect::<Vec<_>>();

        if selected.is_empty() {
            if excluded.is_empty() {
                Err(EngineError::NoMatchingPlugin)
            } else {
                Err(EngineError::NoRulepacksSelected)
            }
        } else {
            Ok(selected)
        }
    }

    fn ensure_known_rulepacks(&self, rulepack_ids: &[String]) -> Result<(), EngineError> {
        for rulepack_id in rulepack_ids {
            if !self.plugins.contains_key(rulepack_id) {
                return Err(EngineError::PluginNotFound(rulepack_id.clone()));
            }
        }

        Ok(())
    }
}

pub struct CoreRulePack {
    metadata: RulePackMetadata,
}

impl Default for CoreRulePack {
    fn default() -> Self {
        Self {
            metadata: RulePackMetadata {
                id: "core".to_string(),
                display_name: "Core Cardinal Rules".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description:
                    "Shared, spec-backed baseline checks for URL-like measurement artifacts."
                        .to_string(),
                source_level: RuleSourceLevel::Normative,
            },
        }
    }
}

impl ValidatorPlugin for CoreRulePack {
    fn metadata(&self) -> &RulePackMetadata {
        &self.metadata
    }

    fn supports(&self, _request: &ValidationRequest) -> bool {
        true
    }

    fn validate(&self, request: &ValidationRequest) -> ValidationReport {
        let mut violations = Vec::new();
        let artifact = request.artifact.trim();

        if artifact.is_empty() {
            violations.push(Violation {
                code: "core.input.empty".to_string(),
                message: "Artifact is empty.".to_string(),
                severity: Severity::Error,
                field: None,
                fix_hint: Some(
                    "Provide a pixel URL, snippet, template, or request to validate.".to_string(),
                ),
                source: RuleSource {
                    level: RuleSourceLevel::Heuristic,
                    name: "Pixellint input baseline".to_string(),
                    reference: None,
                },
                targets: Vec::new(),
            });
        }

        if !artifact.is_empty() {
            match request.artifact_kind {
                ArtifactKind::Url | ArtifactKind::VastTracker | ArtifactKind::ServerPostback => {
                    validate_url_like_artifact(artifact, request.expansion_state, &mut violations);
                }
                _ => {}
            }
        }

        ValidationReport {
            plugin_id: self.metadata.id.clone(),
            detected_vendor: request.claimed_vendor.clone(),
            violations,
        }
    }
}

fn validate_url_like_artifact(
    artifact: &str,
    expansion_state: ExpansionState,
    violations: &mut Vec<Violation>,
) {
    let macro_spans = detect_macro_spans(artifact);
    let has_unsafe_macro_positions =
        apply_macro_rules(artifact, expansion_state, &macro_spans, violations);

    if has_unsafe_macro_positions {
        return;
    }

    let parse_artifact = if macro_spans.is_empty() {
        artifact.to_string()
    } else {
        sanitize_macro_spans(artifact, &macro_spans)
    };

    if has_missing_network_host(&parse_artifact) {
        violations.push(Violation {
            code: "core.url.host_missing".to_string(),
            message: "Network-delivered tracking URLs must include a host component.".to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Provide a fully qualified endpoint such as https://example.com/pixel.".to_string(),
            ),
            source: RuleSource::normative("URL Standard", "https://url.spec.whatwg.org/"),
            targets: Vec::new(),
        });
        return;
    }

    match Url::parse(&parse_artifact) {
        Ok(url) => {
            if url.scheme() == "http" {
                violations.push(Violation {
                    code: "core.url.insecure_transport".to_string(),
                    message:
                        "Plain http tracking endpoints are discouraged; use https for measurement artifacts."
                            .to_string(),
                    severity: Severity::Warning,
                    field: Some("url".to_string()),
                    fix_hint: Some(
                        "Upgrade the endpoint to https so trackers remain compatible with secure playback and delivery environments."
                            .to_string(),
                    ),
                    source: RuleSource {
                        level: RuleSourceLevel::EcosystemReference,
                        name: "Secure tracking transport baseline".to_string(),
                        reference: None,
                    },
                    targets: Vec::new(),
                });
            }

            if !matches!(url.scheme(), "http" | "https") {
                violations.push(Violation {
                    code: "core.url.unsupported_scheme".to_string(),
                    message:
                        "Only http and https URL artifacts are supported by the core rulepack."
                            .to_string(),
                    severity: Severity::Error,
                    field: Some("url".to_string()),
                    fix_hint: Some(
                        "Use an http or https endpoint for network-delivered tracking artifacts."
                            .to_string(),
                    ),
                    source: RuleSource::normative(
                        "W3C Beacon / URL transport baseline",
                        "https://www.w3.org/TR/beacon/",
                    ),
                    targets: Vec::new(),
                });
            }

            if url.host_str().is_none() {
                violations.push(Violation {
                    code: "core.url.host_missing".to_string(),
                    message: "Network-delivered tracking URLs must include a host component."
                        .to_string(),
                    severity: Severity::Error,
                    field: Some("url".to_string()),
                    fix_hint: Some(
                        "Provide a fully qualified endpoint such as https://example.com/pixel."
                            .to_string(),
                    ),
                    source: RuleSource::normative("URL Standard", "https://url.spec.whatwg.org/"),
                    targets: Vec::new(),
                });
            }

            if !url.username().is_empty() || url.password().is_some() {
                violations.push(Violation {
                    code: "core.url.userinfo_deprecated".to_string(),
                    message:
                        "Credentials embedded in tracking URLs are deprecated and should not be used."
                            .to_string(),
                    severity: Severity::Warning,
                    field: Some("url".to_string()),
                    fix_hint: Some(
                        "Move credentials to a safer transport or server-side configuration."
                            .to_string(),
                    ),
                    source: RuleSource::normative(
                        "RFC 3986 URI generic syntax",
                        "https://www.rfc-editor.org/rfc/rfc3986",
                    ),
                    targets: Vec::new(),
                });
            }

            if url.fragment().is_some() {
                violations.push(Violation {
                    code: "core.url.fragment_ignored".to_string(),
                    message:
                        "URL fragments are not transmitted to the server and cannot carry measurement parameters."
                            .to_string(),
                    severity: Severity::Warning,
                    field: Some("url".to_string()),
                    fix_hint: Some(
                        "Move tracking data into the query string or request body."
                            .to_string(),
                    ),
                    source: RuleSource::normative(
                        "RFC 3986 URI generic syntax",
                        "https://www.rfc-editor.org/rfc/rfc3986",
                    ),
                    targets: Vec::new(),
                });
            }
        }
        Err(_) => violations.push(Violation {
            code: "core.url.invalid".to_string(),
            message: "Artifact is not a valid URL.".to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Provide a fully qualified URL such as https://example.com/pixel?x=1".to_string(),
            ),
            source: RuleSource::normative("URL Standard", "https://url.spec.whatwg.org/"),
            targets: Vec::new(),
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MacroSyntax {
    Bracket,
    DollarBraces,
    DoubleBraces,
}

impl MacroSyntax {
    fn example(self) -> &'static str {
        match self {
            Self::Bracket => "[NAME]",
            Self::DollarBraces => "${NAME}",
            Self::DoubleBraces => "{{NAME}}",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MacroPosition {
    Scheme,
    Authority,
    UserInfo,
    Host,
    Port,
    Path,
    Query,
    Fragment,
    WholeUrl,
}

impl MacroPosition {
    fn field(self) -> &'static str {
        match self {
            Self::Scheme => "url.scheme",
            Self::Authority => "url.authority",
            Self::UserInfo => "url.userinfo",
            Self::Host => "url.host",
            Self::Port => "url.port",
            Self::Path => "url.path",
            Self::Query => "url.query",
            Self::Fragment => "url.fragment",
            Self::WholeUrl => "url",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Scheme => "scheme",
            Self::Authority => "authority",
            Self::UserInfo => "userinfo",
            Self::Host => "host",
            Self::Port => "port",
            Self::Path => "path",
            Self::Query => "query",
            Self::Fragment => "fragment",
            Self::WholeUrl => "url",
        }
    }

    fn target_component(self) -> ViolationTargetComponent {
        match self {
            Self::Scheme => ViolationTargetComponent::Scheme,
            Self::Authority => ViolationTargetComponent::Authority,
            Self::UserInfo => ViolationTargetComponent::UserInfo,
            Self::Host => ViolationTargetComponent::Host,
            Self::Port => ViolationTargetComponent::Port,
            Self::Path => ViolationTargetComponent::Path,
            Self::Query => ViolationTargetComponent::QueryParam,
            Self::Fragment => ViolationTargetComponent::Fragment,
            Self::WholeUrl => ViolationTargetComponent::WholeUrl,
        }
    }

    fn is_unsafe(self) -> bool {
        matches!(
            self,
            Self::Scheme | Self::Authority | Self::UserInfo | Self::Host | Self::Port
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacroSpan {
    start: usize,
    end: usize,
    syntax: MacroSyntax,
}

fn apply_macro_rules(
    artifact: &str,
    expansion_state: ExpansionState,
    macro_spans: &[MacroSpan],
    violations: &mut Vec<Violation>,
) -> bool {
    if macro_spans.is_empty() {
        return false;
    }

    let syntaxes = macro_spans
        .iter()
        .map(|span| span.syntax)
        .collect::<BTreeSet<_>>();

    if syntaxes.len() > 1 {
        let targets = macro_spans
            .iter()
            .map(|span| target_for_macro_span(artifact, span))
            .collect();
        let syntax_list = syntaxes
            .iter()
            .map(|syntax| syntax.example())
            .collect::<Vec<_>>()
            .join(", ");
        violations.push(Violation {
            code: "core.macro.mixed_syntax".to_string(),
            message: format!(
                "Multiple macro syntaxes were detected in the same artifact: {syntax_list}."
            ),
            severity: Severity::Warning,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Standardize on one macro syntax per artifact so trafficking and expansion behavior stays predictable."
                    .to_string(),
            ),
            source: macro_rule_source(),
            targets,
        });
    }

    if expansion_state == ExpansionState::Fired {
        let targets = macro_spans
            .iter()
            .map(|span| target_for_macro_span(artifact, span))
            .collect();
        violations.push(Violation {
            code: "core.macro.unexpanded_in_fired_url".to_string(),
            message: "Observed fired URLs should not contain unresolved macro tokens."
                .to_string(),
            severity: Severity::Error,
            field: Some("url".to_string()),
            fix_hint: Some(
                "Expand macros before the request is fired, or validate the artifact in template mode instead."
                    .to_string(),
            ),
            source: macro_rule_source(),
            targets,
        });
    }

    let unsafe_spans = macro_spans
        .iter()
        .map(|span| (*span, classify_macro_position(artifact, span)))
        .filter(|(_, position)| position.is_unsafe())
        .collect::<Vec<_>>();

    let unsafe_positions = unsafe_spans
        .iter()
        .map(|(_, position)| *position)
        .collect::<BTreeSet<_>>();

    if unsafe_positions.is_empty() {
        return false;
    }

    let position_list = unsafe_positions
        .iter()
        .map(|position| position.label())
        .collect::<Vec<_>>()
        .join(", ");
    let first_position = *unsafe_positions
        .iter()
        .next()
        .unwrap_or(&MacroPosition::WholeUrl);
    let targets = unsafe_spans
        .iter()
        .map(|(span, _)| target_for_macro_span(artifact, span))
        .collect();

    violations.push(Violation {
        code: "core.macro.unsafe_position".to_string(),
        message: format!(
            "Macros in URL {position_list} components are unsafe because they can prevent reliable endpoint resolution."
        ),
        severity: Severity::Error,
        field: Some(first_position.field().to_string()),
        fix_hint: Some(
            "Keep macros in path or query values, or expand scheme and authority components before validation."
                .to_string(),
        ),
        source: macro_rule_source(),
        targets,
    });

    true
}

fn macro_rule_source() -> RuleSource {
    RuleSource {
        level: RuleSourceLevel::EcosystemReference,
        name: "Ad-tech macro handling baseline".to_string(),
        reference: None,
    }
}

pub(crate) fn detect_macro_spans(artifact: &str) -> Vec<MacroSpan> {
    let mut spans = Vec::new();
    let mut index = 0;

    while index < artifact.len() {
        let remainder = &artifact[index..];

        if let Some(span) = match_macro_span(artifact, index, remainder) {
            index = span.end;
            spans.push(span);
            continue;
        }

        index += 1;
    }

    spans
}

/// Macro delimiters recognized by the generic ad-tech macro scanner, in match order.
const MACRO_DELIMITERS: [(&str, &str, MacroSyntax); 3] = [
    ("${", "}", MacroSyntax::DollarBraces),
    ("{{", "}}", MacroSyntax::DoubleBraces),
    ("[", "]", MacroSyntax::Bracket),
];

fn match_macro_span(artifact: &str, index: usize, remainder: &str) -> Option<MacroSpan> {
    for (open, close, syntax) in MACRO_DELIMITERS {
        let Some(after_open) = remainder.strip_prefix(open) else {
            continue;
        };
        let Some(close_offset) = after_open.find(close) else {
            continue;
        };

        let body_start = index + open.len();
        let body_end = body_start + close_offset;

        if is_macro_body(&artifact[body_start..body_end]) {
            return Some(MacroSpan {
                start: index,
                end: body_end + close.len(),
                syntax,
            });
        }
    }

    None
}

fn is_macro_body(body: &str) -> bool {
    if body.is_empty() || body.trim() != body {
        return false;
    }

    let mut has_identifier_character = false;

    for character in body.chars() {
        match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.' | '-' => {
                if character.is_ascii_alphabetic() || character == '_' {
                    has_identifier_character = true;
                }
            }
            _ => return false,
        }
    }

    has_identifier_character
}

pub(crate) fn sanitize_macro_spans(artifact: &str, macro_spans: &[MacroSpan]) -> String {
    let mut sanitized = String::with_capacity(artifact.len());
    let mut cursor = 0;

    for span in macro_spans {
        sanitized.push_str(&artifact[cursor..span.start]);
        sanitized.push_str("macro");
        cursor = span.end;
    }

    sanitized.push_str(&artifact[cursor..]);
    sanitized
}

fn target_for_macro_span(artifact: &str, span: &MacroSpan) -> ViolationTarget {
    let position = classify_macro_position(artifact, span);
    let value = &artifact[span.start..span.end];

    if position == MacroPosition::Query {
        let (name, param_value) = query_param_context(artifact, span);
        return ViolationTarget {
            component: ViolationTargetComponent::QueryParam,
            name,
            value: param_value,
            start: span.start,
            end: span.end,
        };
    }

    ViolationTarget {
        component: position.target_component(),
        name: None,
        value: Some(value.to_string()),
        start: span.start,
        end: span.end,
    }
}

fn query_param_context(artifact: &str, span: &MacroSpan) -> (Option<String>, Option<String>) {
    let length = artifact.len();
    let fragment_start = artifact.find('#').unwrap_or(length);
    let Some(query_start) = artifact[..fragment_start].find('?') else {
        return (None, Some(artifact[span.start..span.end].to_string()));
    };

    let query_value_start = query_start + 1;
    if !overlaps(span, query_value_start, fragment_start) {
        return (None, Some(artifact[span.start..span.end].to_string()));
    }

    let segment_start = artifact[..span.start]
        .rfind(['?', '&'])
        .map(|index| index + 1)
        .unwrap_or(query_value_start);
    let segment_end = artifact[span.end..fragment_start]
        .find('&')
        .map(|offset| span.end + offset)
        .unwrap_or(fragment_start);
    let segment = &artifact[segment_start..segment_end];

    if let Some(separator) = segment.find('=') {
        let name = &segment[..separator];
        let value = &segment[separator + 1..];
        (
            (!name.is_empty()).then(|| name.to_string()),
            Some(value.to_string()),
        )
    } else {
        (
            (!segment.is_empty()).then(|| segment.to_string()),
            Some(segment.to_string()),
        )
    }
}

fn classify_macro_position(artifact: &str, span: &MacroSpan) -> MacroPosition {
    let length = artifact.len();
    let Some(scheme_end) = artifact.find("://") else {
        return MacroPosition::WholeUrl;
    };

    if overlaps(span, 0, scheme_end) {
        return MacroPosition::Scheme;
    }

    let authority_start = scheme_end + 3;
    let authority_end = artifact[authority_start..]
        .find(['/', '?', '#'])
        .map(|offset| authority_start + offset)
        .unwrap_or(length);

    let fragment_start = artifact.find('#');
    let query_search_end = fragment_start.unwrap_or(length);
    let query_start = artifact[..query_search_end].find('?');
    let path_end = query_start.or(fragment_start).unwrap_or(length);

    if overlaps(span, authority_start, authority_end) {
        let authority = &artifact[authority_start..authority_end];
        let (userinfo_end, host_start) = if let Some(at_offset) = authority.find('@') {
            let userinfo_end = authority_start + at_offset;
            if overlaps(span, authority_start, userinfo_end) {
                return MacroPosition::UserInfo;
            }
            (Some(userinfo_end), userinfo_end + 1)
        } else {
            (None, authority_start)
        };

        if host_start < authority_end {
            let host_port = &artifact[host_start..authority_end];
            if host_port.starts_with('[') {
                if let Some(close_offset) = host_port.find(']') {
                    let host_end = host_start + close_offset + 1;
                    if overlaps(span, host_start, host_end) {
                        return MacroPosition::Host;
                    }

                    if host_end < authority_end
                        && artifact.as_bytes().get(host_end) == Some(&b':')
                        && overlaps(span, host_end + 1, authority_end)
                    {
                        return MacroPosition::Port;
                    }
                }
            } else if let Some(port_separator) = host_port.rfind(':') {
                let port_start = host_start + port_separator + 1;
                let host_end = host_start + port_separator;

                if overlaps(span, host_start, host_end) {
                    return MacroPosition::Host;
                }

                if overlaps(span, port_start, authority_end) {
                    return MacroPosition::Port;
                }
            } else if overlaps(span, host_start, authority_end) {
                return MacroPosition::Host;
            }
        }

        if userinfo_end.is_some() || overlaps(span, authority_start, authority_end) {
            return MacroPosition::Authority;
        }
    }

    if authority_end < path_end
        && artifact.as_bytes().get(authority_end) == Some(&b'/')
        && overlaps(span, authority_end, path_end)
    {
        return MacroPosition::Path;
    }

    if let Some(query_start) = query_start {
        let query_value_start = query_start + 1;
        let query_end = fragment_start.unwrap_or(length);
        if overlaps(span, query_value_start, query_end) {
            return MacroPosition::Query;
        }
    }

    if let Some(fragment_start) = fragment_start {
        let fragment_value_start = fragment_start + 1;
        if overlaps(span, fragment_value_start, length) {
            return MacroPosition::Fragment;
        }
    }

    MacroPosition::WholeUrl
}

fn overlaps(span: &MacroSpan, start: usize, end: usize) -> bool {
    start < end && span.start < end && span.end > start
}

fn has_missing_network_host(artifact: &str) -> bool {
    let lowered = artifact.to_ascii_lowercase();
    let Some(remainder) = lowered
        .strip_prefix("http://")
        .or_else(|| lowered.strip_prefix("https://"))
    else {
        return false;
    };

    matches!(
        remainder.chars().next(),
        None | Some('/') | Some('?') | Some('#') | Some(':') | Some('@')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixtureRulePack {
        metadata: RulePackMetadata,
    }

    impl Default for FixtureRulePack {
        fn default() -> Self {
            Self {
                metadata: RulePackMetadata {
                    id: "fixture".to_string(),
                    display_name: "Fixture RulePack".to_string(),
                    version: "0.0.0".to_string(),
                    description: "Test helper rulepack".to_string(),
                    source_level: RuleSourceLevel::Heuristic,
                },
            }
        }
    }

    impl ValidatorPlugin for FixtureRulePack {
        fn metadata(&self) -> &RulePackMetadata {
            &self.metadata
        }

        fn supports(&self, _request: &ValidationRequest) -> bool {
            true
        }

        fn validate(&self, request: &ValidationRequest) -> ValidationReport {
            ValidationReport {
                plugin_id: self.metadata.id.clone(),
                detected_vendor: request.claimed_vendor.clone(),
                violations: vec![Violation {
                    code: "fixture.info".to_string(),
                    message: "fixture ran".to_string(),
                    severity: Severity::Info,
                    field: None,
                    fix_hint: None,
                    source: RuleSource {
                        level: RuleSourceLevel::Heuristic,
                        name: "Fixture".to_string(),
                        reference: None,
                    },
                    targets: Vec::new(),
                }],
            }
        }
    }

    fn sample_request(artifact: &str) -> ValidationRequest {
        sample_request_with_kind(ArtifactKind::Url, artifact)
    }

    fn sample_request_with_kind(artifact_kind: ArtifactKind, artifact: &str) -> ValidationRequest {
        sample_request_with_kind_and_state(artifact_kind, ExpansionState::Unknown, artifact)
    }

    fn sample_request_with_kind_and_state(
        artifact_kind: ArtifactKind,
        expansion_state: ExpansionState,
        artifact: &str,
    ) -> ValidationRequest {
        ValidationRequest {
            artifact_kind,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state,
        }
    }

    fn violation_codes(summary: &ValidationSummary) -> Vec<String> {
        summary
            .reports
            .iter()
            .flat_map(|report| report.violations.iter())
            .map(|violation| violation.code.clone())
            .collect()
    }

    #[test]
    fn core_rulepack_runs_in_auto_mode() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1#ignored"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(summary.reports.len(), 1);
        assert_eq!(summary.reports[0].plugin_id, "core");
        assert_eq!(
            summary.reports[0].violations[0].code,
            "core.url.fragment_ignored"
        );
    }

    #[test]
    fn explicit_rulepack_selection_is_toggleable() {
        let mut engine = Engine::default();
        engine.register(FixtureRulePack::default());

        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1"),
                &ValidationOptions {
                    only_rulepacks: vec!["core".to_string(), "fixture".to_string()],
                    except_rulepacks: Vec::new(),
                },
            )
            .unwrap();

        assert_eq!(summary.reports.len(), 2);
        assert_eq!(summary.reports[0].plugin_id, "core");
        assert_eq!(summary.reports[1].plugin_id, "fixture");
    }

    #[test]
    fn excluding_all_selected_rulepacks_returns_an_error() {
        let engine = Engine::default();
        let error = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1"),
                &ValidationOptions {
                    only_rulepacks: vec!["core".to_string()],
                    except_rulepacks: vec!["core".to_string()],
                },
            )
            .unwrap_err();

        assert_eq!(error, EngineError::NoRulepacksSelected);
    }

    #[test]
    fn core_rulepack_warns_on_deprecated_userinfo() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://user:pass@example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            summary.reports[0].violations[0].code,
            "core.url.userinfo_deprecated"
        );
    }

    #[test]
    fn core_rulepack_reports_empty_input_without_cascading_url_noise() {
        let engine = Engine::default();
        let summary = engine
            .validate(&sample_request("   \n\t"), &ValidationOptions::default())
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.input.empty"]);
    }

    #[test]
    fn core_rulepack_errors_on_invalid_url() {
        let engine = Engine::default();
        let summary = engine
            .validate(&sample_request("not a url"), &ValidationOptions::default())
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.url.invalid"]);
    }

    #[test]
    fn core_rulepack_errors_on_unsupported_scheme() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("ftp://example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.unsupported_scheme"]
        );
    }

    #[test]
    fn core_rulepack_warns_on_insecure_http_transport() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("http://example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.insecure_transport"]
        );
    }

    #[test]
    fn core_rulepack_errors_on_missing_host() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https:///pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.url.host_missing"]);
    }

    #[test]
    fn core_rulepack_accepts_clean_https_pixel() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_accepts_trimmed_https_pixel() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request(" \n https://example.com/pixel?id=1 \t "),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_accepts_localhost_with_port() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://localhost:8443/pixel?id=1&source=qa"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_accepts_ipv6_hosts() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://[2001:db8::1]/pixel?id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_combines_insecure_transport_userinfo_and_fragment_warnings() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("http://user:pass@example.com/pixel?id=1#frag"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec![
                "core.url.insecure_transport",
                "core.url.userinfo_deprecated",
                "core.url.fragment_ignored"
            ]
        );
    }

    #[test]
    fn core_rulepack_validates_vast_tracker_urls() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::VastTracker,
                    "https://example.com/vast/track?event=start#ignored",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.url.fragment_ignored"]);
    }

    #[test]
    fn core_rulepack_warns_on_insecure_vast_tracker_transport() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::VastTracker,
                    "http://tracker.example.com/vast/track?event=start#ignored",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.insecure_transport", "core.url.fragment_ignored"]
        );
    }

    #[test]
    fn core_rulepack_validates_server_postback_urls() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::ServerPostback,
                    "ftp://example.com/postback?tx=abc123",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.unsupported_scheme"]
        );
    }

    #[test]
    fn core_rulepack_warns_on_insecure_server_postback_transport() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind(
                    ArtifactKind::ServerPostback,
                    "http://collector.example.com/postback?tx=abc123",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.url.insecure_transport"]
        );
    }

    #[test]
    fn core_rulepack_accepts_query_macros_in_unknown_state() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request("https://example.com/pixel?cb=[CACHEBUSTING]&id=1"),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert!(summary.is_ok());
        assert!(summary.reports[0].violations.is_empty());
    }

    #[test]
    fn core_rulepack_errors_on_unexpanded_macro_in_fired_url() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Fired,
                    "https://example.com/pixel?cb=[CACHEBUSTING]&id=1",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.macro.unexpanded_in_fired_url"]
        );

        let targets = &summary.reports[0].violations[0].targets;
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].component, ViolationTargetComponent::QueryParam);
        assert_eq!(targets[0].name.as_deref(), Some("cb"));
        assert_eq!(targets[0].value.as_deref(), Some("[CACHEBUSTING]"));
    }

    #[test]
    fn core_rulepack_warns_on_mixed_macro_syntax() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Template,
                    "https://example.com/pixel?cb=[CACHEBUSTING]&price=${AUCTION_PRICE}",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(violation_codes(&summary), vec!["core.macro.mixed_syntax"]);

        let targets = &summary.reports[0].violations[0].targets;
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].name.as_deref(), Some("cb"));
        assert_eq!(targets[1].name.as_deref(), Some("price"));
    }

    #[test]
    fn core_rulepack_errors_on_macro_in_host_position() {
        let engine = Engine::default();
        let summary = engine
            .validate(
                &sample_request_with_kind_and_state(
                    ArtifactKind::Url,
                    ExpansionState::Template,
                    "https://${HOST}/pixel?id=1",
                ),
                &ValidationOptions::default(),
            )
            .unwrap();

        assert_eq!(
            violation_codes(&summary),
            vec!["core.macro.unsafe_position"]
        );
        assert_eq!(
            summary.reports[0].violations[0].field.as_deref(),
            Some("url.host")
        );
        assert_eq!(summary.reports[0].violations[0].targets.len(), 1);
        assert_eq!(
            summary.reports[0].violations[0].targets[0].component,
            ViolationTargetComponent::Host
        );
        assert_eq!(
            summary.reports[0].violations[0].targets[0].value.as_deref(),
            Some("${HOST}")
        );
    }
}
