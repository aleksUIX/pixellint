//! Declarative rulepack manifests.
//!
//! A manifest describes a vendor endpoint family as data: which hosts and paths
//! it covers, which parameters it contracts, and what each violation should say.
//! [`ManifestRulePack`] interprets a compiled manifest and implements the same
//! [`ValidatorPlugin`] trait as the hand-written `core` pack, so first-party
//! vendor packs and user-supplied packs run through one code path.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactKind, RulePackMetadata, RuleSource, RuleSourceLevel, Severity, ValidationReport,
    ValidationRequest, ValidatorPlugin, Violation, ViolationTarget, ViolationTargetComponent,
    detect_macro_spans, sanitize_macro_spans,
};

/// How a pack's parameters are carried on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamStyle {
    /// Standard `?name=value&name=value` query parameters.
    #[default]
    Query,
    /// Semicolon-delimited `name=value` pairs inside the path, as used by
    /// Floodlight activity tags.
    Matrix,
}

/// Whether a contracted parameter has to be present, must be absent, or is on
/// its way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    /// Absence is a violation at error severity by default.
    Required,
    /// Absence is a violation at warning severity by default.
    Recommended,
    /// Presence and absence are both fine; only the value format is checked.
    #[default]
    Optional,
    /// Presence is a violation at error severity by default.
    Forbidden,
    /// Presence is a violation at warning severity by default.
    Deprecated,
}

/// Value-level contract applied to a parameter when it is present and fully
/// expanded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValueFormat {
    /// Any non-empty value.
    NonEmpty,
    /// Decimal digits only, with optional length bounds.
    Integer {
        #[serde(default)]
        min_digits: Option<usize>,
        #[serde(default)]
        max_digits: Option<usize>,
    },
    /// One of a fixed set of values.
    Enum {
        values: Vec<String>,
        #[serde(default)]
        case_insensitive: bool,
    },
    /// Matches a regular expression.
    Regex { pattern: String },
    /// Parses as an absolute URL, optionally HTTPS-only. The value is
    /// percent-decoded before parsing.
    Url {
        #[serde(default)]
        require_https: bool,
    },
    /// Lowercase hexadecimal of an exact length, for hashed identifiers.
    Hex { length: usize },
}

/// Which artifacts a pack claims.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSpec {
    /// Artifact kinds the pack applies to. Empty means all URL-like kinds.
    #[serde(default)]
    pub artifact_kinds: Vec<ArtifactKind>,
    /// Exact host matches, compared case-insensitively.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Domain suffix matches. `example.com` matches `a.example.com` and
    /// `example.com`, but not `notexample.com`.
    #[serde(default)]
    pub host_suffixes: Vec<String>,
    /// Exact path matches.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Path prefix matches.
    #[serde(default)]
    pub path_prefixes: Vec<String>,
    /// Path substring matches.
    #[serde(default)]
    pub path_contains: Vec<String>,
}

/// A contracted parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamContract {
    pub name: String,
    /// Alternate spellings that satisfy the same contract.
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub requirement: Requirement,
    #[serde(default)]
    pub format: Option<ValueFormat>,
    /// Overrides the severity derived from `requirement`.
    #[serde(default)]
    pub severity: Option<Severity>,
    /// Human-readable explanation appended to generated messages.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fix_hint: Option<String>,
    /// Official documentation URL for this parameter. Falls back to the pack's
    /// `docs` value.
    #[serde(default)]
    pub doc: Option<String>,
    /// Overrides the pack-level evidence level for this parameter.
    #[serde(default)]
    pub source_level: Option<RuleSourceLevel>,
}

/// The assertion a pack-level rule makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Assertion {
    /// At least one of `params` must be present.
    RequireOneOf { params: Vec<String> },
    /// At most one of `params` may be present.
    MutuallyExclusive { params: Vec<String> },
    /// When `when` is present, every parameter in `requires` must be present.
    RequiredWith { when: String, requires: Vec<String> },
    /// No parameter value may match `pattern`. Empty `params` means every
    /// parameter is checked.
    ForbidValuePattern {
        pattern: String,
        #[serde(default)]
        params: Vec<String>,
    },
}

/// A pack-level rule that spans more than one parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRule {
    /// Stable rule id. Must start with the pack's code prefix.
    pub code: String,
    #[serde(flatten)]
    pub assertion: Assertion,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub fix_hint: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default)]
    pub source_level: Option<RuleSourceLevel>,
}

/// A rulepack expressed as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulePackManifest {
    /// Pack id such as `vendor/meta`. Becomes the code prefix `vendor.meta`.
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// Pack version. Defaults to the crate version when omitted.
    #[serde(default)]
    pub version: Option<String>,
    /// Vendor slug reported as `detected_vendor` when the pack matches.
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default = "default_source_level")]
    pub source_level: RuleSourceLevel,
    /// Pack-wide documentation URL, used when a rule omits its own.
    #[serde(default)]
    pub docs: Option<String>,
    #[serde(default)]
    pub param_style: ParamStyle,
    #[serde(rename = "match")]
    pub matcher: MatchSpec,
    #[serde(default)]
    pub params: Vec<ParamContract>,
    #[serde(default)]
    pub rules: Vec<PackRule>,
}

fn default_source_level() -> RuleSourceLevel {
    RuleSourceLevel::OfficialVendor
}

/// Why a manifest could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Parse(String),
    Io(String),
    InvalidId(String),
    EmptyField {
        pack_id: String,
        field: &'static str,
    },
    MatcherTooBroad(String),
    DuplicateParam {
        pack_id: String,
        name: String,
    },
    InvalidRegex {
        pack_id: String,
        pattern: String,
        error: String,
    },
    MissingCitation {
        pack_id: String,
        rule: String,
    },
    CodePrefix {
        pack_id: String,
        code: String,
        expected_prefix: String,
    },
    UnknownParam {
        pack_id: String,
        code: String,
        name: String,
    },
    EmptyFormatValues {
        pack_id: String,
        name: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "invalid rulepack manifest JSON: {error}"),
            Self::Io(error) => write!(f, "could not read rulepack manifest: {error}"),
            Self::InvalidId(id) => write!(
                f,
                "invalid rulepack id `{id}`: use lowercase segments such as `vendor/meta`"
            ),
            Self::EmptyField { pack_id, field } => {
                write!(f, "rulepack `{pack_id}` has an empty `{field}`")
            }
            Self::MatcherTooBroad(pack_id) => write!(
                f,
                "rulepack `{pack_id}` must declare at least one host or host suffix so it only runs on its own endpoints"
            ),
            Self::DuplicateParam { pack_id, name } => write!(
                f,
                "rulepack `{pack_id}` contracts the parameter `{name}` more than once"
            ),
            Self::InvalidRegex {
                pack_id,
                pattern,
                error,
            } => write!(
                f,
                "rulepack `{pack_id}` has an invalid regex `{pattern}`: {error}"
            ),
            Self::MissingCitation { pack_id, rule } => write!(
                f,
                "rulepack `{pack_id}` rule `{rule}` claims vendor-documented evidence but cites no documentation URL"
            ),
            Self::CodePrefix {
                pack_id,
                code,
                expected_prefix,
            } => write!(
                f,
                "rulepack `{pack_id}` rule code `{code}` must start with `{expected_prefix}`"
            ),
            Self::UnknownParam {
                pack_id,
                code,
                name,
            } => write!(
                f,
                "rulepack `{pack_id}` rule `{code}` references the uncontracted parameter `{name}`"
            ),
            Self::EmptyFormatValues { pack_id, name } => write!(
                f,
                "rulepack `{pack_id}` parameter `{name}` declares an enum format with no values"
            ),
        }
    }
}

impl Error for ManifestError {}

#[derive(Debug)]
struct CompiledParam {
    contract: ParamContract,
    names: Vec<String>,
    regex: Option<Regex>,
    doc: Option<String>,
    source_level: RuleSourceLevel,
}

#[derive(Debug)]
struct CompiledRule {
    rule: PackRule,
    regex: Option<Regex>,
    doc: Option<String>,
    source_level: RuleSourceLevel,
}

/// A rulepack compiled from a [`RulePackManifest`].
#[derive(Debug)]
pub struct ManifestRulePack {
    metadata: RulePackMetadata,
    code_prefix: String,
    vendor: Option<String>,
    docs: Option<String>,
    param_style: ParamStyle,
    matcher: MatchSpec,
    params: Vec<CompiledParam>,
    rules: Vec<CompiledRule>,
}

impl RulePackManifest {
    /// Parses a manifest from JSON without compiling it.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|error| ManifestError::Parse(error.to_string()))
    }
}

impl ManifestRulePack {
    /// Compiles a manifest, validating everything that can be checked without
    /// an artifact: ids, citations, regexes, and cross-references.
    pub fn compile(manifest: RulePackManifest) -> Result<Self, ManifestError> {
        let pack_id = manifest.id.clone();
        validate_pack_id(&pack_id)?;

        for (field, value) in [
            ("display_name", &manifest.display_name),
            ("description", &manifest.description),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::EmptyField { pack_id, field });
            }
        }

        if manifest.matcher.hosts.is_empty() && manifest.matcher.host_suffixes.is_empty() {
            return Err(ManifestError::MatcherTooBroad(pack_id));
        }

        let code_prefix = pack_id.replace('/', ".");
        let mut seen_names = BTreeSet::new();
        let mut params = Vec::with_capacity(manifest.params.len());

        for contract in &manifest.params {
            let mut names = vec![contract.name.clone()];
            names.extend(contract.aliases.iter().cloned());

            for name in &names {
                if !seen_names.insert(name.clone()) {
                    return Err(ManifestError::DuplicateParam {
                        pack_id,
                        name: name.clone(),
                    });
                }
            }

            let source_level = contract.source_level.unwrap_or(manifest.source_level);
            let doc = contract.doc.clone().or_else(|| manifest.docs.clone());
            require_citation(&pack_id, &contract.name, source_level, doc.as_deref())?;

            let regex = match &contract.format {
                Some(ValueFormat::Regex { pattern }) => Some(compile_regex(&pack_id, pattern)?),
                Some(ValueFormat::Enum { values, .. }) if values.is_empty() => {
                    return Err(ManifestError::EmptyFormatValues {
                        pack_id,
                        name: contract.name.clone(),
                    });
                }
                _ => None,
            };

            params.push(CompiledParam {
                contract: contract.clone(),
                names,
                regex,
                doc,
                source_level,
            });
        }

        let mut rules = Vec::with_capacity(manifest.rules.len());

        for rule in &manifest.rules {
            if !rule.code.starts_with(&format!("{code_prefix}.")) {
                return Err(ManifestError::CodePrefix {
                    pack_id,
                    code: rule.code.clone(),
                    expected_prefix: format!("{code_prefix}."),
                });
            }

            for name in assertion_params(&rule.assertion) {
                if !seen_names.contains(name) {
                    return Err(ManifestError::UnknownParam {
                        pack_id,
                        code: rule.code.clone(),
                        name: name.clone(),
                    });
                }
            }

            let source_level = rule.source_level.unwrap_or(manifest.source_level);
            let doc = rule.doc.clone().or_else(|| manifest.docs.clone());
            require_citation(&pack_id, &rule.code, source_level, doc.as_deref())?;

            let regex = match &rule.assertion {
                Assertion::ForbidValuePattern { pattern, .. } => {
                    Some(compile_regex(&pack_id, pattern)?)
                }
                _ => None,
            };

            rules.push(CompiledRule {
                rule: rule.clone(),
                regex,
                doc,
                source_level,
            });
        }

        Ok(Self {
            metadata: RulePackMetadata {
                id: manifest.id.clone(),
                display_name: manifest.display_name.clone(),
                version: manifest
                    .version
                    .clone()
                    .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
                description: manifest.description.clone(),
                source_level: manifest.source_level,
            },
            code_prefix,
            vendor: manifest.vendor.clone(),
            docs: manifest.docs.clone(),
            param_style: manifest.param_style,
            matcher: manifest.matcher.clone(),
            params,
            rules,
        })
    }

    /// Compiles a manifest straight from JSON.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        Self::compile(RulePackManifest::from_json(json)?)
    }

    /// Compiles a manifest from a JSON file on disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let path = path.as_ref();
        let json = fs::read_to_string(path)
            .map_err(|error| ManifestError::Io(format!("{}: {error}", path.display())))?;
        Self::from_json(&json)
    }

    /// The vendor slug this pack reports when it matches.
    pub fn vendor(&self) -> Option<&str> {
        self.vendor.as_deref()
    }

    /// The pack-wide documentation URL, if any.
    pub fn docs(&self) -> Option<&str> {
        self.docs.as_deref()
    }

    fn matches_artifact_kind(&self, kind: ArtifactKind) -> bool {
        if self.matcher.artifact_kinds.is_empty() {
            return matches!(
                kind,
                ArtifactKind::Url
                    | ArtifactKind::VastTracker
                    | ArtifactKind::ServerPostback
                    | ArtifactKind::NetworkRequest
                    | ArtifactKind::Unknown
            );
        }

        self.matcher.artifact_kinds.contains(&kind)
    }

    fn matches_endpoint(&self, artifact: &str) -> bool {
        let Some(parsed) = parse_artifact_url(artifact) else {
            return false;
        };
        let Some(host) = parsed.host else {
            return false;
        };
        let host = host.to_ascii_lowercase();

        let host_matches = self
            .matcher
            .hosts
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&host))
            || self.matcher.host_suffixes.iter().any(|suffix| {
                let suffix = suffix.to_ascii_lowercase();
                host == suffix || host.ends_with(&format!(".{suffix}"))
            });

        if !host_matches {
            return false;
        }

        let path = parsed.path;
        let path_constrained = !self.matcher.paths.is_empty()
            || !self.matcher.path_prefixes.is_empty()
            || !self.matcher.path_contains.is_empty();

        if !path_constrained {
            return true;
        }

        self.matcher
            .paths
            .iter()
            .any(|candidate| candidate == &path)
            || self
                .matcher
                .path_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
            || self
                .matcher
                .path_contains
                .iter()
                .any(|needle| path.contains(needle))
    }
}

impl ValidatorPlugin for ManifestRulePack {
    fn metadata(&self) -> &RulePackMetadata {
        &self.metadata
    }

    fn supports(&self, request: &ValidationRequest) -> bool {
        self.matches_artifact_kind(request.artifact_kind)
            && self.matches_endpoint(request.artifact.trim())
    }

    fn validate(&self, request: &ValidationRequest) -> ValidationReport {
        let artifact = request.artifact.trim();
        let mut violations = Vec::new();

        if !self.matches_endpoint(artifact) {
            violations.push(Violation {
                code: format!("{}.endpoint_mismatch", self.code_prefix),
                message: format!(
                    "{} was requested explicitly, but this artifact does not target one of its endpoints.",
                    self.metadata.display_name
                ),
                severity: Severity::Info,
                field: None,
                fix_hint: Some(
                    "Drop the rulepack selection to let Pixellint pick packs by endpoint.".to_string(),
                ),
                source: RuleSource {
                    level: RuleSourceLevel::Heuristic,
                    name: format!("{} endpoint matcher", self.metadata.display_name),
                    reference: self.docs.clone(),
                },
                targets: Vec::new(),
            });

            return ValidationReport {
                plugin_id: self.metadata.id.clone(),
                detected_vendor: None,
                violations,
            };
        }

        let params = extract_params(artifact, self.param_style);

        for compiled in &self.params {
            self.check_param(artifact, compiled, &params, &mut violations);
        }

        for compiled in &self.rules {
            self.check_rule(artifact, compiled, &params, &mut violations);
        }

        if let (Some(claimed), Some(vendor)) = (request.claimed_vendor.as_deref(), self.vendor())
            && !claimed.eq_ignore_ascii_case(vendor)
        {
            violations.push(Violation {
                code: format!("{}.claimed_vendor_mismatch", self.code_prefix),
                message: format!(
                    "Artifact was submitted as `{claimed}` but targets a {vendor} endpoint."
                ),
                severity: Severity::Info,
                field: None,
                fix_hint: Some(format!(
                    "Set claimed_vendor to `{vendor}` or check that the endpoint is the intended one."
                )),
                source: RuleSource {
                    level: RuleSourceLevel::Heuristic,
                    name: format!("{} endpoint matcher", self.metadata.display_name),
                    reference: self.docs.clone(),
                },
                targets: Vec::new(),
            });
        }

        ValidationReport {
            plugin_id: self.metadata.id.clone(),
            detected_vendor: self.vendor.clone(),
            violations,
        }
    }
}

impl ManifestRulePack {
    fn source_for(&self, level: RuleSourceLevel, doc: Option<&str>) -> RuleSource {
        RuleSource {
            level,
            name: self.metadata.display_name.clone(),
            reference: doc.map(str::to_string),
        }
    }

    fn check_param(
        &self,
        artifact: &str,
        compiled: &CompiledParam,
        params: &[RawParam],
        violations: &mut Vec<Violation>,
    ) {
        let contract = &compiled.contract;
        let present: Vec<&RawParam> = params
            .iter()
            .filter(|param| compiled.names.iter().any(|name| name == &param.name))
            .collect();
        let field = format!("param.{}", contract.name);
        let source = self.source_for(compiled.source_level, compiled.doc.as_deref());

        match contract.requirement {
            Requirement::Required | Requirement::Recommended => {
                if present.is_empty() {
                    let severity = contract.severity.unwrap_or(
                        if contract.requirement == Requirement::Required {
                            Severity::Error
                        } else {
                            Severity::Warning
                        },
                    );

                    violations.push(Violation {
                        code: format!("{}.param.{}.missing", self.code_prefix, contract.name),
                        message: describe(
                            format!(
                                "`{}` is {} on {} requests but is not present.",
                                contract.name,
                                if contract.requirement == Requirement::Required {
                                    "required"
                                } else {
                                    "expected"
                                },
                                self.metadata.display_name
                            ),
                            contract.description.as_deref(),
                        ),
                        severity,
                        field: Some(field.clone()),
                        fix_hint: contract
                            .fix_hint
                            .clone()
                            .or_else(|| Some(format!("Add the `{}` parameter.", contract.name))),
                        source: source.clone(),
                        targets: vec![whole_url_target(artifact)],
                    });
                    return;
                }
            }
            Requirement::Forbidden => {
                for param in &present {
                    violations.push(Violation {
                        code: format!("{}.param.{}.forbidden", self.code_prefix, contract.name),
                        message: describe(
                            format!(
                                "`{}` must not be sent to {}.",
                                param.name, self.metadata.display_name
                            ),
                            contract.description.as_deref(),
                        ),
                        severity: contract.severity.unwrap_or(Severity::Error),
                        field: Some(field.clone()),
                        fix_hint: contract
                            .fix_hint
                            .clone()
                            .or_else(|| Some(format!("Remove the `{}` parameter.", param.name))),
                        source: source.clone(),
                        targets: vec![param.target()],
                    });
                }
                return;
            }
            Requirement::Deprecated => {
                for param in &present {
                    violations.push(Violation {
                        code: format!("{}.param.{}.deprecated", self.code_prefix, contract.name),
                        message: describe(
                            format!(
                                "`{}` is deprecated by {}.",
                                param.name, self.metadata.display_name
                            ),
                            contract.description.as_deref(),
                        ),
                        severity: contract.severity.unwrap_or(Severity::Warning),
                        field: Some(field.clone()),
                        fix_hint: contract.fix_hint.clone(),
                        source: source.clone(),
                        targets: vec![param.target()],
                    });
                }
            }
            Requirement::Optional => {}
        }

        for param in present {
            if param.value.is_empty() {
                violations.push(Violation {
                    code: format!("{}.param.{}.empty", self.code_prefix, contract.name),
                    message: format!("`{}` is present but has an empty value.", param.name),
                    severity: contract.severity.unwrap_or(match contract.requirement {
                        Requirement::Required => Severity::Error,
                        _ => Severity::Warning,
                    }),
                    field: Some(field.clone()),
                    fix_hint: contract
                        .fix_hint
                        .clone()
                        .or_else(|| Some(format!("Populate `{}` before firing.", param.name))),
                    source: source.clone(),
                    targets: vec![param.target()],
                });
                continue;
            }

            // Unexpanded macros are the core pack's business. Checking the
            // literal macro text against a value format would double-report the
            // same defect with a worse message.
            if contains_macro(&param.value) {
                continue;
            }

            let Some(format) = &contract.format else {
                continue;
            };

            if let Some(reason) = format_violation(format, compiled.regex.as_ref(), &param.value) {
                violations.push(Violation {
                    code: format!("{}.param.{}.invalid", self.code_prefix, contract.name),
                    message: describe(
                        format!("`{}` {reason}", param.name),
                        contract.description.as_deref(),
                    ),
                    severity: contract.severity.unwrap_or(Severity::Error),
                    field: Some(field.clone()),
                    fix_hint: contract.fix_hint.clone(),
                    source: source.clone(),
                    targets: vec![param.target()],
                });
            }
        }
    }

    fn check_rule(
        &self,
        artifact: &str,
        compiled: &CompiledRule,
        params: &[RawParam],
        violations: &mut Vec<Violation>,
    ) {
        let rule = &compiled.rule;
        let source = self.source_for(compiled.source_level, compiled.doc.as_deref());
        let present = |name: &str| params.iter().any(|param| param.name == name);

        let (triggered, targets) = match &rule.assertion {
            Assertion::RequireOneOf { params: names } => {
                if names.iter().any(|name| present(name)) {
                    (false, Vec::new())
                } else {
                    (true, vec![whole_url_target(artifact)])
                }
            }
            Assertion::MutuallyExclusive { params: names } => {
                let hits: Vec<&RawParam> = params
                    .iter()
                    .filter(|param| names.contains(&param.name))
                    .collect();

                if hits.len() > 1 {
                    (true, hits.iter().map(|param| param.target()).collect())
                } else {
                    (false, Vec::new())
                }
            }
            Assertion::RequiredWith { when, requires } => {
                if present(when) && !requires.iter().all(|name| present(name)) {
                    let target = params
                        .iter()
                        .find(|param| &param.name == when)
                        .map(RawParam::target)
                        .unwrap_or_else(|| whole_url_target(artifact));
                    (true, vec![target])
                } else {
                    (false, Vec::new())
                }
            }
            Assertion::ForbidValuePattern {
                params: names,
                pattern: _,
            } => {
                let Some(regex) = compiled.regex.as_ref() else {
                    return;
                };

                let hits: Vec<&RawParam> = params
                    .iter()
                    .filter(|param| names.is_empty() || names.contains(&param.name))
                    .filter(|param| !contains_macro(&param.value))
                    .filter(|param| regex.is_match(&param.value))
                    .collect();

                if hits.is_empty() {
                    (false, Vec::new())
                } else {
                    (true, hits.iter().map(|param| param.target()).collect())
                }
            }
        };

        if triggered {
            violations.push(Violation {
                code: rule.code.clone(),
                message: rule.message.clone(),
                severity: rule.severity,
                field: None,
                fix_hint: rule.fix_hint.clone(),
                source,
                targets,
            });
        }
    }
}

fn describe(message: String, description: Option<&str>) -> String {
    match description {
        Some(description) if !description.trim().is_empty() => {
            format!("{message} {description}")
        }
        _ => message,
    }
}

fn assertion_params(assertion: &Assertion) -> Vec<&String> {
    match assertion {
        Assertion::RequireOneOf { params } | Assertion::MutuallyExclusive { params } => {
            params.iter().collect()
        }
        Assertion::RequiredWith { when, requires } => {
            let mut names = vec![when];
            names.extend(requires.iter());
            names
        }
        Assertion::ForbidValuePattern { params, .. } => params.iter().collect(),
    }
}

fn require_citation(
    pack_id: &str,
    rule: &str,
    level: RuleSourceLevel,
    doc: Option<&str>,
) -> Result<(), ManifestError> {
    let cited = doc.is_some_and(|doc| doc.starts_with("http://") || doc.starts_with("https://"));

    if level == RuleSourceLevel::OfficialVendor && !cited {
        return Err(ManifestError::MissingCitation {
            pack_id: pack_id.to_string(),
            rule: rule.to_string(),
        });
    }

    Ok(())
}

fn compile_regex(pack_id: &str, pattern: &str) -> Result<Regex, ManifestError> {
    Regex::new(pattern).map_err(|error| ManifestError::InvalidRegex {
        pack_id: pack_id.to_string(),
        pattern: pattern.to_string(),
        error: error.to_string(),
    })
}

fn validate_pack_id(id: &str) -> Result<(), ManifestError> {
    let valid = !id.is_empty()
        && id.split('/').all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        });

    if valid {
        Ok(())
    } else {
        Err(ManifestError::InvalidId(id.to_string()))
    }
}

fn format_violation(format: &ValueFormat, regex: Option<&Regex>, value: &str) -> Option<String> {
    match format {
        ValueFormat::NonEmpty => None,
        ValueFormat::Integer {
            min_digits,
            max_digits,
        } => {
            if !value.chars().all(|character| character.is_ascii_digit()) {
                return Some(format!("must be numeric, but is `{value}`."));
            }
            if let Some(min) = min_digits
                && value.len() < *min
            {
                return Some(format!(
                    "must be at least {min} digits, but `{value}` has {}.",
                    value.len()
                ));
            }
            if let Some(max) = max_digits
                && value.len() > *max
            {
                return Some(format!(
                    "must be at most {max} digits, but `{value}` has {}.",
                    value.len()
                ));
            }
            None
        }
        ValueFormat::Enum {
            values,
            case_insensitive,
        } => {
            let matched = values.iter().any(|candidate| {
                if *case_insensitive {
                    candidate.eq_ignore_ascii_case(value)
                } else {
                    candidate == value
                }
            });

            if matched {
                None
            } else {
                Some(format!(
                    "is `{value}`, which is not one of the documented values: {}.",
                    values.join(", ")
                ))
            }
        }
        ValueFormat::Regex { pattern } => match regex {
            Some(regex) if regex.is_match(value) => None,
            Some(_) => Some(format!(
                "is `{value}`, which does not match the documented format `{pattern}`."
            )),
            None => None,
        },
        ValueFormat::Url { require_https } => {
            let decoded = percent_decode(value);
            match url::Url::parse(&decoded) {
                Ok(parsed) => {
                    if *require_https && parsed.scheme() != "https" {
                        Some(format!("must be an https URL, but is `{decoded}`."))
                    } else {
                        None
                    }
                }
                Err(_) => Some(format!("must be an absolute URL, but is `{decoded}`.")),
            }
        }
        ValueFormat::Hex { length } => {
            let valid = value.len() == *length
                && value.chars().all(|character| {
                    character.is_ascii_hexdigit() && !character.is_ascii_uppercase()
                });

            if valid {
                None
            } else {
                Some(format!(
                    "must be {length} lowercase hex characters, which usually means a hashed value, but is `{value}`."
                ))
            }
        }
    }
}

fn contains_macro(value: &str) -> bool {
    !detect_macro_spans(value).is_empty()
}

/// A parameter as it appears in the raw artifact, with byte offsets preserved so
/// findings can point at the exact span.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawParam {
    name: String,
    value: String,
    start: usize,
    end: usize,
}

impl RawParam {
    fn target(&self) -> ViolationTarget {
        ViolationTarget {
            component: ViolationTargetComponent::QueryParam,
            name: Some(self.name.clone()),
            value: Some(self.value.clone()),
            start: self.start,
            end: self.end,
        }
    }
}

fn whole_url_target(artifact: &str) -> ViolationTarget {
    ViolationTarget {
        component: ViolationTargetComponent::WholeUrl,
        name: None,
        value: None,
        start: 0,
        end: artifact.len(),
    }
}

struct ArtifactUrl {
    host: Option<String>,
    path: String,
}

/// Parses an artifact URL with macros neutralized, so a templated URL still
/// resolves to a host and path.
fn parse_artifact_url(artifact: &str) -> Option<ArtifactUrl> {
    let spans = detect_macro_spans(artifact);
    let sanitized = if spans.is_empty() {
        artifact.to_string()
    } else {
        sanitize_macro_spans(artifact, &spans)
    };

    let parsed = url::Url::parse(&sanitized).ok()?;

    Some(ArtifactUrl {
        host: parsed.host_str().map(str::to_string),
        path: parsed.path().to_string(),
    })
}

/// Splits the raw artifact into parameters, keeping byte offsets into the
/// original string. Query style reads `?a=1&b=2`; matrix style reads the
/// semicolon-delimited pairs Floodlight puts in the path.
fn extract_params(artifact: &str, style: ParamStyle) -> Vec<RawParam> {
    let length = artifact.len();
    let fragment_start = artifact.find('#').unwrap_or(length);
    let query_start = artifact[..fragment_start].find('?');

    let (region_start, region_end) = match style {
        ParamStyle::Query => match query_start {
            Some(start) => (start + 1, fragment_start),
            None => return Vec::new(),
        },
        ParamStyle::Matrix => {
            let path_end = query_start.unwrap_or(fragment_start);
            let scheme_end = artifact.find("://").map(|index| index + 3).unwrap_or(0);
            let path_start = artifact[scheme_end..path_end]
                .find('/')
                .map(|offset| scheme_end + offset)
                .unwrap_or(path_end);
            (path_start, path_end)
        }
    };

    if region_start >= region_end {
        return Vec::new();
    }

    let separator = match style {
        ParamStyle::Query => '&',
        ParamStyle::Matrix => ';',
    };

    let mut params = Vec::new();
    let mut cursor = region_start;

    while cursor <= region_end {
        let segment_end = artifact[cursor..region_end]
            .find(separator)
            .map(|offset| cursor + offset)
            .unwrap_or(region_end);
        let segment = &artifact[cursor..segment_end];

        if !segment.is_empty()
            && let Some((name, value)) = segment.split_once('=')
        {
            params.push(RawParam {
                name: percent_decode(name),
                value: percent_decode(value),
                start: cursor,
                end: segment_end,
            });
        }

        if segment_end >= region_end {
            break;
        }

        cursor = segment_end + 1;
    }

    params
}

/// Minimal percent-decoding for parameter names and values. `+` is decoded as a
/// space because form-encoded pixel payloads are common.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = (bytes[index + 1] as char).to_digit(16);
                let low = (bytes[index + 2] as char).to_digit(16);

                match (high, low) {
                    (Some(high), Some(low)) => {
                        decoded.push((high * 16 + low) as u8);
                        index += 3;
                    }
                    _ => {
                        decoded.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ExpansionState, ValidationOptions};

    const TEST_MANIFEST: &str = r#"{
        "id": "vendor/test",
        "display_name": "Test Vendor Pixel",
        "description": "Fixture pack used by the manifest loader tests.",
        "vendor": "test",
        "source_level": "official_vendor",
        "docs": "https://example.com/docs",
        "match": {
            "hosts": ["px.example.com"],
            "path_prefixes": ["/collect"]
        },
        "params": [
            {
                "name": "id",
                "requirement": "required",
                "format": { "kind": "integer", "min_digits": 3 }
            },
            {
                "name": "ev",
                "requirement": "required",
                "format": { "kind": "enum", "values": ["PageView", "Purchase"] }
            },
            {
                "name": "legacy",
                "requirement": "deprecated"
            },
            {
                "name": "debug",
                "requirement": "forbidden"
            },
            {
                "name": "url",
                "format": { "kind": "url", "require_https": true }
            }
        ],
        "rules": [
            {
                "code": "vendor.test.pii.raw_email",
                "kind": "forbid_value_pattern",
                "pattern": "@",
                "severity": "error",
                "message": "Raw email addresses must be hashed before they are sent."
            }
        ]
    }"#;

    fn pack() -> ManifestRulePack {
        ManifestRulePack::from_json(TEST_MANIFEST).expect("compile test manifest")
    }

    fn request(artifact: &str) -> ValidationRequest {
        ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: artifact.to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        }
    }

    fn codes(report: &ValidationReport) -> Vec<String> {
        report
            .violations
            .iter()
            .map(|violation| violation.code.clone())
            .collect()
    }

    #[test]
    fn clean_artifact_produces_no_findings() {
        let report = pack().validate(&request(
            "https://px.example.com/collect?id=12345&ev=PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));
        assert_eq!(report.detected_vendor.as_deref(), Some("test"));
    }

    #[test]
    fn missing_required_params_are_reported() {
        let report = pack().validate(&request("https://px.example.com/collect?other=1"));
        assert_eq!(
            codes(&report),
            vec![
                "vendor.test.param.id.missing".to_string(),
                "vendor.test.param.ev.missing".to_string(),
            ]
        );
    }

    #[test]
    fn value_formats_are_enforced() {
        let report = pack().validate(&request(
            "https://px.example.com/collect?id=ab&ev=Signup&url=http%3A%2F%2Fexample.com",
        ));
        assert_eq!(
            codes(&report),
            vec![
                "vendor.test.param.id.invalid".to_string(),
                "vendor.test.param.ev.invalid".to_string(),
                "vendor.test.param.url.invalid".to_string(),
            ]
        );
    }

    #[test]
    fn integer_digit_bounds_are_enforced() {
        let report = pack().validate(&request("https://px.example.com/collect?id=12&ev=Purchase"));
        assert_eq!(codes(&report), vec!["vendor.test.param.id.invalid"]);
    }

    #[test]
    fn deprecated_and_forbidden_params_are_reported() {
        let report = pack().validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase&legacy=1&debug=1",
        ));
        assert_eq!(
            codes(&report),
            vec![
                "vendor.test.param.legacy.deprecated".to_string(),
                "vendor.test.param.debug.forbidden".to_string(),
            ]
        );
    }

    #[test]
    fn empty_values_are_reported_once() {
        let report = pack().validate(&request("https://px.example.com/collect?id=&ev=PageView"));
        assert_eq!(codes(&report), vec!["vendor.test.param.id.empty"]);
    }

    #[test]
    fn macro_values_defer_to_the_core_pack() {
        let report = pack().validate(&request(
            "https://px.example.com/collect?id=[ADVERTISER_ID]&ev=PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn pack_rules_run_over_every_param_when_unscoped() {
        let report = pack().validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase&em=buyer%40example.com",
        ));
        assert_eq!(codes(&report), vec!["vendor.test.pii.raw_email"]);
    }

    #[test]
    fn targets_point_at_the_offending_parameter() {
        let artifact = "https://px.example.com/collect?id=ab&ev=PageView";
        let report = pack().validate(&request(artifact));
        let target = &report.violations[0].targets[0];
        assert_eq!(&artifact[target.start..target.end], "id=ab");
        assert_eq!(target.name.as_deref(), Some("id"));
    }

    #[test]
    fn packs_only_match_their_own_endpoints() {
        let pack = pack();
        assert!(pack.supports(&request("https://px.example.com/collect?id=1")));
        assert!(!pack.supports(&request("https://px.example.com/other?id=1")));
        assert!(!pack.supports(&request("https://other.example.com/collect?id=1")));
        assert!(!pack.supports(&request("https://notpx.example.com/collect?id=1")));
    }

    #[test]
    fn host_suffix_matching_respects_label_boundaries() {
        let manifest = TEST_MANIFEST.replace(
            r#""hosts": ["px.example.com"]"#,
            r#""host_suffixes": ["example.com"]"#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile suffix manifest");
        assert!(pack.supports(&request("https://px.example.com/collect?id=1")));
        assert!(pack.supports(&request("https://example.com/collect?id=1")));
        assert!(!pack.supports(&request("https://notexample.com/collect?id=1")));
    }

    #[test]
    fn forcing_a_pack_off_endpoint_reports_the_mismatch() {
        let report = pack().validate(&request("https://other.example.com/collect?id=1"));
        assert_eq!(codes(&report), vec!["vendor.test.endpoint_mismatch"]);
        assert_eq!(report.detected_vendor, None);
    }

    #[test]
    fn claimed_vendor_mismatch_is_informational() {
        let mut request = request("https://px.example.com/collect?id=123&ev=PageView");
        request.claimed_vendor = Some("other".to_string());
        let report = pack().validate(&request);
        assert_eq!(codes(&report), vec!["vendor.test.claimed_vendor_mismatch"]);
    }

    #[test]
    fn matrix_params_are_read_from_the_path() {
        let manifest = TEST_MANIFEST
            .replace(r#""match": {"#, r#""param_style": "matrix", "match": {"#)
            .replace(
                r#""path_prefixes": ["/collect"]"#,
                r#""path_prefixes": ["/activity"]"#,
            );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile matrix manifest");
        let report = pack.validate(&request(
            "https://px.example.com/activity;id=12345;ev=PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let report = pack.validate(&request("https://px.example.com/activity;id=12345"));
        assert_eq!(codes(&report), vec!["vendor.test.param.ev.missing"]);
    }

    #[test]
    fn engine_registers_manifests_and_runs_them_with_core() {
        let mut engine = Engine::default();
        engine
            .register_manifest_json(TEST_MANIFEST)
            .expect("register manifest");

        let summary = engine
            .validate(
                &request("http://px.example.com/collect?id=1"),
                &ValidationOptions::default(),
            )
            .expect("validate");

        let plugin_ids: Vec<&str> = summary
            .reports
            .iter()
            .map(|report| report.plugin_id.as_str())
            .collect();
        assert!(plugin_ids.contains(&"core"));
        assert!(plugin_ids.contains(&"vendor/test"));
    }

    #[test]
    fn manifests_without_a_host_matcher_are_rejected() {
        let manifest = TEST_MANIFEST.replace(r#""hosts": ["px.example.com"],"#, "");
        let error = ManifestRulePack::from_json(&manifest).expect_err("matcher must be scoped");
        assert!(
            matches!(error, ManifestError::MatcherTooBroad(_)),
            "{error}"
        );
    }

    #[test]
    fn vendor_rules_must_cite_documentation() {
        let manifest = TEST_MANIFEST.replace(r#""docs": "https://example.com/docs","#, "");
        let error = ManifestRulePack::from_json(&manifest).expect_err("citation is required");
        assert!(
            matches!(error, ManifestError::MissingCitation { .. }),
            "{error}"
        );
    }

    #[test]
    fn rule_codes_must_use_the_pack_prefix() {
        let manifest = TEST_MANIFEST.replace("vendor.test.pii.raw_email", "custom.pii.raw_email");
        let error = ManifestRulePack::from_json(&manifest).expect_err("prefix is enforced");
        assert!(matches!(error, ManifestError::CodePrefix { .. }), "{error}");
    }

    #[test]
    fn rules_cannot_reference_uncontracted_params() {
        let manifest = TEST_MANIFEST.replace(
            r#""kind": "forbid_value_pattern",
                "pattern": "@","#,
            r#""kind": "require_one_of",
                "params": ["not_contracted"],"#,
        );
        let error = ManifestRulePack::from_json(&manifest).expect_err("cross-reference is checked");
        assert!(
            matches!(error, ManifestError::UnknownParam { .. }),
            "{error}"
        );
    }

    #[test]
    fn duplicate_param_names_are_rejected() {
        let manifest = TEST_MANIFEST.replace(
            r#"{
                "name": "legacy",
                "requirement": "deprecated"
            },"#,
            r#"{
                "name": "id",
                "requirement": "optional"
            },"#,
        );
        let error = ManifestRulePack::from_json(&manifest).expect_err("duplicates are rejected");
        assert!(
            matches!(error, ManifestError::DuplicateParam { .. }),
            "{error}"
        );
    }

    #[test]
    fn invalid_regexes_are_rejected_at_load_time() {
        let manifest = TEST_MANIFEST.replace(r#""pattern": "@""#, r#""pattern": "([""#);
        let error = ManifestRulePack::from_json(&manifest).expect_err("regex is compiled at load");
        assert!(
            matches!(error, ManifestError::InvalidRegex { .. }),
            "{error}"
        );
    }

    #[test]
    fn unknown_manifest_fields_are_rejected() {
        let manifest = TEST_MANIFEST.replace(
            r#""id": "vendor/test","#,
            r#""id": "vendor/test", "typo_field": true,"#,
        );
        let error = ManifestRulePack::from_json(&manifest).expect_err("unknown fields are caught");
        assert!(matches!(error, ManifestError::Parse(_)), "{error}");
    }

    #[test]
    fn invalid_pack_ids_are_rejected() {
        let manifest = TEST_MANIFEST.replace(r#""id": "vendor/test""#, r#""id": "Vendor Test""#);
        let error = ManifestRulePack::from_json(&manifest).expect_err("ids are validated");
        assert!(matches!(error, ManifestError::InvalidId(_)), "{error}");
    }

    #[test]
    fn percent_encoded_values_are_decoded_before_checks() {
        assert_eq!(percent_decode("buyer%40example.com"), "buyer@example.com");
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
