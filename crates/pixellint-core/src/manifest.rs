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

use crate::json::{self, JsonDocument, JsonValueKind};
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
    /// JSON body shapes the pack claims. A body artifact belongs to this pack
    /// when every entry here holds, so the shape of the payload stands in for
    /// the host that a bare body does not carry.
    #[serde(default)]
    pub json_paths: Vec<ShapeMatch>,
}

/// One condition on the shape of a JSON body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShapeMatch {
    /// The path resolves to a field that is present.
    Present(String),
    /// The path carries no value that belongs to a different vendor.
    ///
    /// Conversion APIs have converged on the same `{"data": [...]}` envelope,
    /// so presence alone cannot tell a Meta payload from a Snap one. The values
    /// can: Meta writes `action_source: "website"` where Snap writes `"WEB"`.
    ///
    /// This is written as an exclusion rather than as "my values match" on
    /// purpose. The discriminating field is usually one the pack also contracts,
    /// and a payload with a typo in it is the one that most needs validating, so
    /// an unfamiliar value must leave the payload claimable. Only a value that
    /// positively belongs to someone else rules the pack out.
    Excludes { path: String, excludes: String },
    /// At least one of the nested conditions holds. Endpoints that accept both
    /// a single event and a batch envelope need it: the two shapes have no path
    /// in common, but either one identifies the payload.
    Any { any_of: Vec<ShapeMatch> },
}

impl ShapeMatch {
    /// Every path this condition names, so they can all be checked at load.
    fn paths(&self) -> Vec<&str> {
        match self {
            Self::Present(path) => vec![path],
            Self::Excludes { path, .. } => vec![path],
            Self::Any { any_of } => any_of.iter().flat_map(Self::paths).collect(),
        }
    }
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
    /// Overrides the severity of value-format violations only. Lets a pack say
    /// "this parameter is mandatory, but an unrecognized value is only worth a
    /// warning", which is the common shape for event-name parameters that also
    /// accept custom values.
    #[serde(default)]
    pub format_severity: Option<Severity>,
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
    /// When `when` carries one of `equals`, every parameter in `requires` must
    /// be present. This is the shape consent signals take: a flag value decides
    /// whether the rest of the signal is mandatory.
    RequiredWhenValue {
        when: String,
        equals: Vec<String>,
        requires: Vec<String>,
    },
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
    /// Regular expression with named capture groups, run against the artifact's
    /// path. Each named group becomes a parameter, which is how endpoints that
    /// carry an identifier in the path rather than the query get contracted.
    #[serde(default)]
    pub path_pattern: Option<String>,
    #[serde(rename = "match")]
    pub matcher: MatchSpec,
    #[serde(default)]
    pub params: Vec<ParamContract>,
    #[serde(default)]
    pub rules: Vec<PackRule>,
    /// Contracts on the JSON request body, for endpoints that carry their
    /// payload there rather than in the query string.
    #[serde(default)]
    pub body: Option<BodySpec>,
}

/// Which part of a body the contracts are written against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScopeSpec {
    /// One batch array, such as `data[]`.
    One(String),
    /// Alternative envelopes, tried in order, first one that resolves wins. An
    /// empty string means the document itself. LinkedIn takes either a single
    /// event at the root or a batch under `elements`, so its pack declares
    /// `["elements[]", ""]`.
    Any(Vec<String>),
}

impl ScopeSpec {
    fn patterns(&self) -> Vec<&str> {
        match self {
            Self::One(pattern) => vec![pattern],
            Self::Any(patterns) => patterns.iter().map(String::as_str).collect(),
        }
    }
}

/// Contracts applied to a JSON request body.
///
/// Conversion APIs batch events into an array, and every element of that array
/// has to satisfy the same contract. `scope` names that array, and the packs
/// underneath it are written relative to one element, so a manifest reads
/// `event_name` rather than repeating `data[].event_name` on every line. Each
/// element is then evaluated on its own: three events with no `event_name`
/// produce three findings, each pointing at its own bytes.
///
/// Omitting `scope` evaluates the document once as a single scope, which is the
/// shape of an API that posts one event per request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodySpec {
    #[serde(default)]
    pub scope: Option<ScopeSpec>,
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
    PathPatternWithoutCaptures {
        pack_id: String,
        pattern: String,
    },
    BodyWithoutShape(String),
    ShapeWithoutBody(String),
    InvalidJsonPath {
        pack_id: String,
        path: String,
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
            Self::PathPatternWithoutCaptures { pack_id, pattern } => write!(
                f,
                "rulepack `{pack_id}` path pattern `{pattern}` has no named capture groups, so it contributes no parameters"
            ),
            Self::BodyWithoutShape(pack_id) => write!(
                f,
                "rulepack `{pack_id}` contracts a JSON body but declares no `match.json_paths`, so it would claim every payload it is shown"
            ),
            Self::ShapeWithoutBody(pack_id) => write!(
                f,
                "rulepack `{pack_id}` declares `match.json_paths` but contracts no body, so the shape match would check nothing"
            ),
            Self::InvalidJsonPath { pack_id, path } => write!(
                f,
                "rulepack `{pack_id}` has the malformed JSON path `{path}`: use dotted keys with `[]` for arrays, as in `data[].user_data.em[]`"
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

/// Where a set of contracts is being evaluated, so one checker can serve both
/// the query string and a JSON body without either borrowing the other's
/// vocabulary.
struct Scope<'a> {
    /// Segment used in violation codes: `param` for the URL, `body` for a
    /// payload. Keeping them apart matters because an endpoint may accept the
    /// same field in both places under different rules.
    code_segment: &'static str,
    /// Prefix for the reported `field`, such as `param` or `body.data[1]`.
    field_prefix: String,
    /// Where to point when there is nothing more specific to point at, such as
    /// a field that is missing entirely.
    fallback: ViolationTarget,
    /// The payload being read, when this scope is a body rather than a URL.
    body: Option<BodyScope<'a>>,
}

/// The document and the element a body scope is evaluating.
struct BodyScope<'a> {
    document: &'a JsonDocument,
    path: &'a str,
}

impl Scope<'_> {
    /// Where to point for a finding about a field that carries no value of its
    /// own. A missing `user_data.em` is most useful pointed at the `user_data`
    /// that does exist, rather than at the whole event.
    fn fallback_for(&self, name: &str) -> ViolationTarget {
        let Some(body) = &self.body else {
            return self.fallback.clone();
        };

        let path = match body.path.is_empty() {
            true => name.to_string(),
            false => format!("{}.{name}", body.path),
        };

        match body.document.nearest_present_ancestor(&path) {
            Some(field) => ViolationTarget {
                component: ViolationTargetComponent::BodyField,
                name: Some(field.path.clone()),
                value: None,
                start: field.start,
                end: field.end,
            },
            None => self.fallback.clone(),
        }
    }
}

/// A shape condition with its patterns compiled.
#[derive(Debug)]
enum CompiledShape {
    Present(String),
    Excludes { path: String, regex: Regex },
    Any(Vec<CompiledShape>),
}

impl CompiledShape {
    fn holds(&self, document: &JsonDocument) -> bool {
        match self {
            Self::Present(path) => document.matches_pattern(path),
            Self::Excludes { path, regex } => !document
                .expand(path)
                .iter()
                .filter_map(|concrete| document.get(concrete))
                .any(|field| regex.is_match(&field.text)),
            Self::Any(alternatives) => alternatives.iter().any(|shape| shape.holds(document)),
        }
    }
}

#[derive(Debug)]
struct CompiledBody {
    scope: Option<ScopeSpec>,
    params: Vec<CompiledParam>,
    rules: Vec<CompiledRule>,
}

/// A rulepack compiled from a [`RulePackManifest`].
#[derive(Debug)]
pub struct ManifestRulePack {
    metadata: RulePackMetadata,
    code_prefix: String,
    vendor: Option<String>,
    docs: Option<String>,
    param_style: ParamStyle,
    path_pattern: Option<Regex>,
    matcher: MatchSpec,
    params: Vec<CompiledParam>,
    rules: Vec<CompiledRule>,
    body: Option<CompiledBody>,
    shapes: Vec<CompiledShape>,
}

impl RulePackManifest {
    /// Parses a manifest from JSON without compiling it.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|error| ManifestError::Parse(error.to_string()))
    }
}

/// Compiles the shape conditions a pack claims its payloads by.
fn compile_shapes(
    pack_id: &str,
    shapes: &[ShapeMatch],
) -> Result<Vec<CompiledShape>, ManifestError> {
    shapes
        .iter()
        .map(|shape| compile_shape(pack_id, shape))
        .collect()
}

fn compile_shape(pack_id: &str, shape: &ShapeMatch) -> Result<CompiledShape, ManifestError> {
    Ok(match shape {
        ShapeMatch::Present(path) => CompiledShape::Present(path.clone()),
        ShapeMatch::Excludes { path, excludes } => CompiledShape::Excludes {
            path: path.clone(),
            regex: compile_regex(pack_id, excludes)?,
        },
        ShapeMatch::Any { any_of } => CompiledShape::Any(compile_shapes(pack_id, any_of)?),
    })
}

/// Compiles one list of parameter contracts, returning the names it defines so
/// rules can be checked against them.
fn compile_params(
    pack_id: &str,
    manifest: &RulePackManifest,
    contracts: &[ParamContract],
) -> Result<(Vec<CompiledParam>, BTreeSet<String>), ManifestError> {
    let mut seen_names = BTreeSet::new();
    let mut params = Vec::with_capacity(contracts.len());

    for contract in contracts {
        let mut names = vec![contract.name.clone()];
        names.extend(contract.aliases.iter().cloned());

        for name in &names {
            if !seen_names.insert(name.clone()) {
                return Err(ManifestError::DuplicateParam {
                    pack_id: pack_id.to_string(),
                    name: name.clone(),
                });
            }
        }

        let source_level = contract.source_level.unwrap_or(manifest.source_level);
        let doc = contract.doc.clone().or_else(|| manifest.docs.clone());
        require_citation(pack_id, &contract.name, source_level, doc.as_deref())?;

        let regex = match &contract.format {
            Some(ValueFormat::Regex { pattern }) => Some(compile_regex(pack_id, pattern)?),
            Some(ValueFormat::Enum { values, .. }) if values.is_empty() => {
                return Err(ManifestError::EmptyFormatValues {
                    pack_id: pack_id.to_string(),
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

    Ok((params, seen_names))
}

/// Compiles one list of cross-parameter rules against the names available to
/// them, so a rule can never reference a parameter the pack does not contract.
fn compile_rules(
    pack_id: &str,
    code_prefix: &str,
    manifest: &RulePackManifest,
    pack_rules: &[PackRule],
    available: &BTreeSet<String>,
) -> Result<Vec<CompiledRule>, ManifestError> {
    let mut rules = Vec::with_capacity(pack_rules.len());

    for rule in pack_rules {
        if !rule.code.starts_with(&format!("{code_prefix}.")) {
            return Err(ManifestError::CodePrefix {
                pack_id: pack_id.to_string(),
                code: rule.code.clone(),
                expected_prefix: format!("{code_prefix}."),
            });
        }

        for name in assertion_params(&rule.assertion) {
            if !available.contains(name) {
                return Err(ManifestError::UnknownParam {
                    pack_id: pack_id.to_string(),
                    code: rule.code.clone(),
                    name: name.clone(),
                });
            }
        }

        let source_level = rule.source_level.unwrap_or(manifest.source_level);
        let doc = rule.doc.clone().or_else(|| manifest.docs.clone());
        require_citation(pack_id, &rule.code, source_level, doc.as_deref())?;

        let regex = match &rule.assertion {
            Assertion::ForbidValuePattern { pattern, .. } => Some(compile_regex(pack_id, pattern)?),
            _ => None,
        };

        rules.push(CompiledRule {
            rule: rule.clone(),
            regex,
            doc,
            source_level,
        });
    }

    Ok(rules)
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

        let path_pattern = match &manifest.path_pattern {
            Some(pattern) => {
                let regex = compile_regex(&pack_id, pattern)?;

                if regex.capture_names().flatten().count() == 0 {
                    return Err(ManifestError::PathPatternWithoutCaptures {
                        pack_id,
                        pattern: pattern.clone(),
                    });
                }

                Some(regex)
            }
            None => None,
        };

        let mut shapes = Vec::new();
        let (params, url_names) = compile_params(&pack_id, &manifest, &manifest.params)?;
        let rules = compile_rules(
            &pack_id,
            &code_prefix,
            &manifest,
            &manifest.rules,
            &url_names,
        )?;

        // Body contracts get their own name space, because an endpoint is free
        // to accept the same field in the query string and in the payload, and
        // the two are different contracts with different findings.
        let body = match &manifest.body {
            Some(spec) => {
                if manifest.matcher.json_paths.is_empty() {
                    return Err(ManifestError::BodyWithoutShape(pack_id));
                }

                let (params, names) = compile_params(&pack_id, &manifest, &spec.params)?;
                let rules = compile_rules(&pack_id, &code_prefix, &manifest, &spec.rules, &names)?;

                let paths = manifest
                    .matcher
                    .json_paths
                    .iter()
                    .flat_map(ShapeMatch::paths)
                    .map(str::to_string)
                    .chain(
                        spec.scope
                            .iter()
                            .flat_map(ScopeSpec::patterns)
                            // The empty pattern names the document itself.
                            .filter(|pattern| !pattern.is_empty())
                            .map(str::to_string),
                    )
                    .chain(names.iter().cloned())
                    .collect::<Vec<_>>();

                for path in &paths {
                    if !json::is_valid_pattern(path) {
                        return Err(ManifestError::InvalidJsonPath {
                            pack_id,
                            path: path.clone(),
                        });
                    }
                }

                shapes = compile_shapes(&pack_id, &manifest.matcher.json_paths)?;

                Some(CompiledBody {
                    scope: spec.scope.clone(),
                    params,
                    rules,
                })
            }
            None => {
                if !manifest.matcher.json_paths.is_empty() {
                    return Err(ManifestError::ShapeWithoutBody(pack_id));
                }
                None
            }
        };

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
                vendor: manifest.vendor.clone(),
            },
            code_prefix,
            vendor: manifest.vendor.clone(),
            docs: manifest.docs.clone(),
            param_style: manifest.param_style,
            path_pattern,
            matcher: manifest.matcher.clone(),
            params,
            rules,
            body,
            shapes,
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
            ) || (kind == ArtifactKind::JsonPayload && self.body.is_some());
        }

        self.matcher.artifact_kinds.contains(&kind)
    }

    /// Whether this artifact should be read as a JSON body rather than a URL.
    /// An explicit `json` kind says so outright; an unstated kind is treated as
    /// a body when it opens like one and the pack has body contracts to apply.
    fn reads_as_body(&self, kind: ArtifactKind, artifact: &str) -> bool {
        if self.body.is_none() {
            return false;
        }

        kind == ArtifactKind::JsonPayload
            || (kind == ArtifactKind::Unknown && JsonDocument::looks_like_json(artifact))
    }

    /// Whether the payload has the shape this pack claims. A bare body carries
    /// no host, so its shape is the only thing that can identify it.
    fn matches_shape(&self, document: &JsonDocument) -> bool {
        !self.shapes.is_empty() && self.shapes.iter().all(|shape| shape.holds(document))
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
        let artifact = request.artifact.trim();

        if !self.matches_artifact_kind(request.artifact_kind) {
            return false;
        }

        if self.reads_as_body(request.artifact_kind, artifact) {
            return JsonDocument::parse(artifact)
                .map(|document| self.matches_shape(&document))
                .unwrap_or(false);
        }

        self.matches_endpoint(artifact)
    }

    fn validate(&self, request: &ValidationRequest) -> ValidationReport {
        let artifact = request.artifact.trim();
        let mut violations = Vec::new();

        if self.reads_as_body(request.artifact_kind, artifact) {
            return self.validate_body(request, artifact);
        }

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

        let mut params = extract_params(artifact, self.param_style);
        params.extend(self.extract_path_params(artifact));

        let scope = Scope {
            code_segment: "param",
            field_prefix: "param".to_string(),
            fallback: whole_url_target(artifact),
            body: None,
        };

        for compiled in &self.params {
            self.check_param(&scope, compiled, &params, &mut violations);
        }

        for compiled in &self.rules {
            self.check_rule(&scope, compiled, &params, &mut violations);
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
    /// Validates a JSON request body.
    ///
    /// The contracts are written against one element of the batch, so the body
    /// is evaluated once per element: an array of three events that all omit
    /// `event_name` reports three findings, each pointing at its own event. A
    /// payload the pack does not recognize is reported the same way a URL that
    /// misses the endpoint is, since both mean the caller aimed the pack at the
    /// wrong artifact.
    fn validate_body(&self, request: &ValidationRequest, artifact: &str) -> ValidationReport {
        let Some(body) = &self.body else {
            return ValidationReport {
                plugin_id: self.metadata.id.clone(),
                detected_vendor: None,
                violations: Vec::new(),
            };
        };

        // A body that does not parse is the core pack's finding to report, and
        // it has nothing this pack can contract.
        let Ok(document) = JsonDocument::parse(artifact) else {
            return ValidationReport {
                plugin_id: self.metadata.id.clone(),
                detected_vendor: None,
                violations: Vec::new(),
            };
        };

        let mut violations = Vec::new();

        if !self.matches_shape(&document) {
            violations.push(Violation {
                code: format!("{}.payload_mismatch", self.code_prefix),
                message: format!(
                    "{} was requested explicitly, but this payload does not have the shape its endpoint accepts.",
                    self.metadata.display_name
                ),
                severity: Severity::Info,
                field: None,
                fix_hint: Some(
                    "Drop the rulepack selection to let Pixellint pick packs by payload shape."
                        .to_string(),
                ),
                source: RuleSource {
                    level: RuleSourceLevel::Heuristic,
                    name: format!("{} payload matcher", self.metadata.display_name),
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

        for scope_path in body_scopes(&document, body.scope.as_ref()) {
            let params = collect_body_params(&document, &scope_path, &body.params);
            let scope = Scope {
                code_segment: "body",
                field_prefix: match scope_path.is_empty() {
                    true => "body".to_string(),
                    false => format!("body.{scope_path}"),
                },
                fallback: body_target(&document, &scope_path, artifact),
                body: Some(BodyScope {
                    document: &document,
                    path: &scope_path,
                }),
            };

            for compiled in &body.params {
                self.check_param(&scope, compiled, &params, &mut violations);
            }

            for compiled in &body.rules {
                self.check_rule(&scope, compiled, &params, &mut violations);
            }
        }

        if let (Some(claimed), Some(vendor)) = (request.claimed_vendor.as_deref(), self.vendor())
            && !claimed.eq_ignore_ascii_case(vendor)
        {
            violations.push(Violation {
                code: format!("{}.claimed_vendor_mismatch", self.code_prefix),
                message: format!(
                    "Artifact was submitted as `{claimed}` but is a {vendor} payload."
                ),
                severity: Severity::Info,
                field: None,
                fix_hint: Some(format!(
                    "Set claimed_vendor to `{vendor}` or check that the payload is the intended one."
                )),
                source: RuleSource {
                    level: RuleSourceLevel::Heuristic,
                    name: format!("{} payload matcher", self.metadata.display_name),
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

    /// Pulls named captures out of the path so an identifier carried there can
    /// be contracted like any query parameter.
    fn extract_path_params(&self, artifact: &str) -> Vec<RawParam> {
        let Some(pattern) = &self.path_pattern else {
            return Vec::new();
        };

        let (path_start, path_end) = path_span(artifact);
        let path = &artifact[path_start..path_end];
        let Some(captures) = pattern.captures(path) else {
            return Vec::new();
        };

        pattern
            .capture_names()
            .flatten()
            .filter_map(|name| {
                let capture = captures.name(name)?;

                Some(RawParam::query(
                    name.to_string(),
                    percent_decode(capture.as_str()),
                    path_start + capture.start(),
                    path_start + capture.end(),
                ))
            })
            .collect()
    }

    fn source_for(&self, level: RuleSourceLevel, doc: Option<&str>) -> RuleSource {
        RuleSource {
            level,
            name: self.metadata.display_name.clone(),
            reference: doc.map(str::to_string),
        }
    }

    fn check_param(
        &self,
        scope: &Scope,
        compiled: &CompiledParam,
        params: &[RawParam],
        violations: &mut Vec<Violation>,
    ) {
        let contract = &compiled.contract;
        let addressed: Vec<&RawParam> = params
            .iter()
            .filter(|param| compiled.names.iter().any(|name| name == &param.name))
            .collect();
        let present: Vec<&RawParam> = addressed
            .iter()
            .copied()
            .filter(|param| !param.missing)
            .collect();
        let empty_slots: Vec<&RawParam> = addressed
            .iter()
            .copied()
            .filter(|param| param.missing)
            .collect();
        let field = format!("{}.{}", scope.field_prefix, contract.name);
        let source = self.source_for(compiled.source_level, compiled.doc.as_deref());

        match contract.requirement {
            Requirement::Required | Requirement::Recommended => {
                let severity =
                    contract
                        .severity
                        .unwrap_or(if contract.requirement == Requirement::Required {
                            Severity::Error
                        } else {
                            Severity::Warning
                        });

                let report_missing =
                    |targets: Vec<ViolationTarget>, violations: &mut Vec<Violation>| {
                        for target in targets {
                            violations.push(Violation {
                                code: format!(
                                    "{}.{}.{}.missing",
                                    self.code_prefix, scope.code_segment, contract.name
                                ),
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
                                fix_hint: contract.fix_hint.clone().or_else(|| {
                                    Some(format!("Add the `{}` parameter.", contract.name))
                                }),
                                source: source.clone(),
                                targets: vec![target],
                            });
                        }
                    };

                if scope.body.is_some() {
                    // Nothing addressed the contract at all: something above it
                    // is missing and has already been reported.
                    if addressed.is_empty() {
                        return;
                    }

                    // Every place the value belongs is checked on its own. One
                    // identifier carrying `idType` does not excuse the next one
                    // for leaving it out.
                    report_missing(
                        empty_slots.iter().map(|slot| slot.target()).collect(),
                        violations,
                    );
                } else if present.is_empty() {
                    report_missing(vec![scope.fallback_for(&contract.name)], violations);
                    return;
                }
            }
            Requirement::Forbidden => {
                for param in &present {
                    violations.push(Violation {
                        code: format!(
                            "{}.{}.{}.forbidden",
                            self.code_prefix, scope.code_segment, contract.name
                        ),
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
                        code: format!(
                            "{}.{}.{}.deprecated",
                            self.code_prefix, scope.code_segment, contract.name
                        ),
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
                    code: format!(
                        "{}.{}.{}.empty",
                        self.code_prefix, scope.code_segment, contract.name
                    ),
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

            // A value format describes a scalar. When the same field is also
            // accepted as a list, the list itself is checked element by element
            // by the contract written for it.
            if param.container {
                continue;
            }

            let Some(format) = &contract.format else {
                continue;
            };

            if let Some(reason) = format_violation(format, compiled.regex.as_ref(), &param.value) {
                violations.push(Violation {
                    code: format!(
                        "{}.{}.{}.invalid",
                        self.code_prefix, scope.code_segment, contract.name
                    ),
                    message: describe(
                        format!("`{}` {reason}", param.name),
                        contract.description.as_deref(),
                    ),
                    severity: contract
                        .format_severity
                        .or(contract.severity)
                        .unwrap_or(Severity::Error),
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
        scope: &Scope,
        compiled: &CompiledRule,
        params: &[RawParam],
        violations: &mut Vec<Violation>,
    ) {
        let rule = &compiled.rule;
        let source = self.source_for(compiled.source_level, compiled.doc.as_deref());
        // An empty slot is not a value, so a rule must not read it as one.
        let live: Vec<RawParam> = params
            .iter()
            .filter(|param| !param.missing)
            .cloned()
            .collect();
        let params: &[RawParam] = &live;
        let present = |name: &str| params.iter().any(|param| param.name == name);

        let (triggered, targets) = match &rule.assertion {
            Assertion::RequireOneOf { params: names } => {
                if names.iter().any(|name| present(name)) {
                    (false, Vec::new())
                } else {
                    (true, vec![scope.fallback.clone()])
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
                        .unwrap_or_else(|| scope.fallback_for(when));
                    (true, vec![target])
                } else {
                    (false, Vec::new())
                }
            }
            Assertion::RequiredWhenValue {
                when,
                equals,
                requires,
            } => {
                let triggered = params
                    .iter()
                    .filter(|param| &param.name == when)
                    .filter(|param| !contains_macro(&param.value))
                    .any(|param| equals.contains(&param.value));

                if triggered && !requires.iter().all(|name| present(name)) {
                    let target = params
                        .iter()
                        .find(|param| &param.name == when)
                        .map(RawParam::target)
                        .unwrap_or_else(|| scope.fallback_for(when));
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
        Assertion::RequiredWith { when, requires }
        | Assertion::RequiredWhenValue { when, requires, .. } => {
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

pub(crate) fn contains_macro(value: &str) -> bool {
    !detect_macro_spans(value).is_empty()
}

/// A parameter as it appears in the raw artifact, with byte offsets preserved so
/// findings can point at the exact span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawParam {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// Where the value was carried, so a finding about a body field is not
    /// reported as if it were a query parameter.
    pub(crate) component: ViolationTargetComponent,
    /// The concrete location for a body field, such as `data[1].event_name`.
    /// Query parameters have no need for it and leave it empty.
    pub(crate) location: Option<String>,
    /// Whether the value is an object or an array. Vendors accept several of
    /// these fields as either a scalar or a list of them, so a value contract
    /// written for the scalar form must not fire on the list.
    pub(crate) container: bool,
    /// A slot the contract addresses where no value was found.
    ///
    /// Body contracts need to tell two absences apart. A field missing from an
    /// event that exists is worth reporting, once per event. A field under a
    /// container that is itself missing is not: the container has already been
    /// reported, and the fields beneath it were never addressable.
    pub(crate) missing: bool,
}

impl RawParam {
    /// A query parameter, the common case.
    pub(crate) fn query(name: String, value: String, start: usize, end: usize) -> Self {
        Self {
            name,
            value,
            start,
            end,
            component: ViolationTargetComponent::QueryParam,
            location: None,
            container: false,
            missing: false,
        }
    }

    pub(crate) fn target(&self) -> ViolationTarget {
        ViolationTarget {
            component: self.component,
            name: Some(self.location.clone().unwrap_or_else(|| self.name.clone())),
            value: Some(self.value.clone()),
            start: self.start,
            end: self.end,
        }
    }
}

/// The concrete scopes a body spec evaluates over: one per element of the batch
/// array, or a single empty scope covering the whole document when the endpoint
/// takes one event per request.
fn body_scopes(document: &JsonDocument, scope: Option<&ScopeSpec>) -> Vec<String> {
    let Some(scope) = scope else {
        return vec![String::new()];
    };

    for pattern in scope.patterns() {
        if pattern.is_empty() {
            return vec![String::new()];
        }

        let scopes = document.expand(pattern);
        if !scopes.is_empty() {
            return scopes;
        }
    }

    Vec::new()
}

/// Reads one scope's contracted fields out of the document.
///
/// Each contract name is a path relative to the scope, so `user_data.em[]`
/// under `data[1]` reads `data[1].user_data.em[0]` and up. Every hit keeps the
/// contract's own name so the existing checkers match it, and carries its
/// concrete path along for the finding.
fn collect_body_params(
    document: &JsonDocument,
    scope: &str,
    contracts: &[CompiledParam],
) -> Vec<RawParam> {
    let mut params = Vec::new();

    for compiled in contracts {
        for name in &compiled.names {
            let pattern = match scope.is_empty() {
                true => name.clone(),
                false => format!("{scope}.{name}"),
            };

            for path in document.expand(&pattern) {
                let Some(field) = document.get(&path) else {
                    // The slot is addressable but empty. Recording it lets the
                    // contract report once per place the value belongs.
                    let anchor = document.nearest_present_ancestor(&path);

                    params.push(RawParam {
                        name: name.clone(),
                        value: String::new(),
                        start: anchor.map(|field| field.start).unwrap_or(0),
                        end: anchor.map(|field| field.end).unwrap_or(0),
                        component: ViolationTargetComponent::BodyField,
                        location: Some(path),
                        container: false,
                        missing: true,
                    });
                    continue;
                };

                // Containers have no text of their own. Standing in the label
                // keeps a populated object from being reported as empty, while
                // an empty one still is.
                let value = match (field.kind, field.is_blank()) {
                    (_, true) => String::new(),
                    (JsonValueKind::Object | JsonValueKind::Array, false) => {
                        field.kind.label().to_string()
                    }
                    _ => field.text.clone(),
                };

                params.push(RawParam {
                    name: name.clone(),
                    value,
                    start: field.start,
                    end: field.end,
                    component: ViolationTargetComponent::BodyField,
                    location: Some(path),
                    container: matches!(field.kind, JsonValueKind::Object | JsonValueKind::Array),
                    missing: false,
                });
            }
        }
    }

    params
}

/// Where to point when a body finding has no field of its own to blame: the
/// enclosing event if there is one, otherwise the whole payload.
fn body_target(document: &JsonDocument, scope: &str, artifact: &str) -> ViolationTarget {
    match document.get(scope) {
        Some(field) => ViolationTarget {
            component: ViolationTargetComponent::BodyField,
            name: Some(scope.to_string()),
            value: None,
            start: field.start,
            end: field.end,
        },
        None => ViolationTarget {
            component: ViolationTargetComponent::WholeBody,
            name: None,
            value: None,
            start: 0,
            end: artifact.len(),
        },
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

/// The host of an artifact, with macros neutralized first.
pub(crate) fn artifact_host(artifact: &str) -> Option<String> {
    parse_artifact_url(artifact)?.host
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
pub(crate) fn extract_params(artifact: &str, style: ParamStyle) -> Vec<RawParam> {
    let length = artifact.len();
    let fragment_start = artifact.find('#').unwrap_or(length);
    let query_start = artifact[..fragment_start].find('?');

    let (region_start, region_end) = match style {
        ParamStyle::Query => match query_start {
            Some(start) => (start + 1, fragment_start),
            None => return Vec::new(),
        },
        ParamStyle::Matrix => path_span(artifact),
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
            // Matrix parameters ride on the path, so the first pair carries the
            // path prefix with it: `/ddm/activity/src=123`. Trim everything up
            // to the last slash in the name so the parameter is `src`, and move
            // the reported span with it.
            let (name, name_offset) = match name.rfind('/') {
                Some(index) if style == ParamStyle::Matrix => (&name[index + 1..], index + 1),
                _ => (name, 0),
            };

            params.push(RawParam::query(
                percent_decode(name),
                percent_decode(value),
                cursor + name_offset,
                segment_end,
            ));
        }

        if segment_end >= region_end {
            break;
        }

        cursor = segment_end + 1;
    }

    params
}

/// Byte range of the path within a raw artifact, excluding query and fragment.
pub(crate) fn path_span(artifact: &str) -> (usize, usize) {
    let length = artifact.len();
    let fragment_start = artifact.find('#').unwrap_or(length);
    let path_end = artifact[..fragment_start]
        .find('?')
        .unwrap_or(fragment_start);
    let scheme_end = artifact.find("://").map(|index| index + 3).unwrap_or(0);
    let path_start = artifact[scheme_end..path_end]
        .find('/')
        .map(|offset| scheme_end + offset)
        .unwrap_or(path_end);

    (path_start, path_end)
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
    fn matrix_params_survive_a_path_prefix() {
        let manifest = TEST_MANIFEST
            .replace(r#""match": {"#, r#""param_style": "matrix", "match": {"#)
            .replace(
                r#""path_prefixes": ["/collect"]"#,
                r#""path_prefixes": ["/ddm/activity"]"#,
            );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile matrix manifest");
        let artifact = "https://px.example.com/ddm/activity/id=12345;ev=Purchase;legacy=1?";
        let report = pack.validate(&request(artifact));

        assert_eq!(codes(&report), vec!["vendor.test.param.legacy.deprecated"]);
        let target = &report.violations[0].targets[0];
        assert_eq!(&artifact[target.start..target.end], "legacy=1");
    }

    #[test]
    fn format_severity_overrides_only_value_findings() {
        let manifest = TEST_MANIFEST.replace(
            r#""format": { "kind": "enum", "values": ["PageView", "Purchase"] }"#,
            r#""format": { "kind": "enum", "values": ["PageView", "Purchase"] }, "format_severity": "warning""#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile manifest");

        let report = pack.validate(&request("https://px.example.com/collect?id=123&ev=Custom"));
        assert_eq!(report.violations[0].severity, Severity::Warning);
        assert!(report.is_ok());

        let report = pack.validate(&request("https://px.example.com/collect?id=123"));
        assert_eq!(report.violations[0].severity, Severity::Error);
    }

    #[test]
    fn conditional_rules_fire_on_the_triggering_value_only() {
        let manifest = TEST_MANIFEST.replace(
            r#"        "rules": ["#,
            r#"        "rules": [
            {
                "code": "vendor.test.consent.state_requires_country",
                "kind": "required_when_value",
                "when": "ev",
                "equals": ["Purchase"],
                "requires": ["legacy"],
                "severity": "error",
                "message": "Purchase events must carry the legacy identifier."
            },"#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile manifest");

        // The trigger value is present and the required parameter is not.
        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase",
        ));
        assert_eq!(
            codes(&report),
            vec!["vendor.test.consent.state_requires_country"]
        );

        // Trigger value present, requirement satisfied.
        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase&legacy=1",
        ));
        assert_eq!(codes(&report), vec!["vendor.test.param.legacy.deprecated"]);

        // A different value does not trigger the rule.
        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        // An unexpanded macro is not a value claim, so it cannot trigger.
        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=[EVENT_NAME]",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn path_captures_become_contracted_parameters() {
        let manifest = TEST_MANIFEST
            .replace(
                r#""match": {"#,
                r#""path_pattern": "^/conversion/(?<id>[^/]*)/", "match": {"#,
            )
            .replace(
                r#""path_prefixes": ["/collect"]"#,
                r#""path_prefixes": ["/conversion"]"#,
            );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile manifest");

        // The path capture satisfies the `id` contract.
        let artifact = "https://px.example.com/conversion/12345/?ev=PageView";
        let report = pack.validate(&request(artifact));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        // It is checked like any other parameter, and points at the path span.
        let artifact = "https://px.example.com/conversion/not-numeric/?ev=PageView";
        let report = pack.validate(&request(artifact));
        assert_eq!(codes(&report), vec!["vendor.test.param.id.invalid"]);
        let target = &report.violations[0].targets[0];
        assert_eq!(&artifact[target.start..target.end], "not-numeric");

        // An empty capture reads as an empty value, not a missing parameter.
        let report = pack.validate(&request("https://px.example.com/conversion//?ev=PageView"));
        assert_eq!(codes(&report), vec!["vendor.test.param.id.empty"]);

        // A path that does not match the pattern leaves the parameter missing.
        let report = pack.validate(&request("https://px.example.com/conversion?ev=PageView"));
        assert_eq!(codes(&report), vec!["vendor.test.param.id.missing"]);
    }

    #[test]
    fn path_patterns_must_capture_something() {
        let manifest = TEST_MANIFEST.replace(
            r#""match": {"#,
            r#""path_pattern": "^/conversion/[0-9]+", "match": {"#,
        );
        let error = ManifestRulePack::from_json(&manifest).expect_err("captures are required");
        assert!(
            matches!(error, ManifestError::PathPatternWithoutCaptures { .. }),
            "{error}"
        );
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

#[cfg(test)]
mod body_tests {
    use super::*;
    use crate::ExpansionState;

    const BODY_MANIFEST: &str = r#"{
        "id": "vendor/test-api",
        "display_name": "Test Conversions API",
        "description": "Fixture pack used by the body loader tests.",
        "vendor": "test",
        "source_level": "official_vendor",
        "docs": "https://example.com/docs",
        "match": {
            "hosts": ["api.example.com"],
            "json_paths": ["data[].event_name"]
        },
        "body": {
            "scope": "data[]",
            "params": [
                { "name": "event_name", "requirement": "required" },
                {
                    "name": "event_time",
                    "requirement": "required",
                    "format": { "kind": "integer", "max_digits": 10 }
                },
                { "name": "user_data", "requirement": "required" },
                {
                    "name": "user_data.em[]",
                    "format": { "kind": "regex", "pattern": "^[a-f0-9]{4}$" }
                },
                { "name": "custom_data.value" }
            ],
            "rules": [
                {
                    "code": "vendor.test-api.body.purchase_needs_value",
                    "kind": "required_when_value",
                    "when": "event_name",
                    "equals": ["Purchase"],
                    "requires": ["custom_data.value"],
                    "severity": "error",
                    "message": "A purchase needs a value."
                }
            ]
        }
    }"#;

    fn body_pack() -> ManifestRulePack {
        ManifestRulePack::from_json(BODY_MANIFEST).expect("compile body manifest")
    }

    fn body_request(artifact: &str) -> ValidationRequest {
        ValidationRequest {
            artifact_kind: ArtifactKind::JsonPayload,
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
    fn a_body_pack_claims_a_payload_by_shape() {
        let pack = body_pack();

        assert!(pack.supports(&body_request(r#"{"data":[{"event_name":"Purchase"}]}"#)));
        // Same kind, different API: the shape is what keeps packs apart when
        // there is no host to go by.
        assert!(!pack.supports(&body_request(r#"{"events":[{"name":"Purchase"}]}"#)));
        assert!(!pack.supports(&body_request("not json at all")));
    }

    #[test]
    fn an_unstated_kind_is_read_as_a_body_when_it_opens_like_one() {
        let mut request = body_request(r#"{"data":[{"event_name":"Purchase"}]}"#);
        request.artifact_kind = ArtifactKind::Unknown;

        assert!(body_pack().supports(&request));
    }

    #[test]
    fn every_element_of_the_batch_is_checked_on_its_own() {
        let report = body_pack().validate(&body_request(
            r#"{"data":[{"event_name":"A"},{"event_name":"B"}]}"#,
        ));

        // Two events, each missing the same two fields: four findings, not two.
        assert_eq!(
            codes(&report),
            vec![
                "vendor.test-api.body.event_time.missing",
                "vendor.test-api.body.user_data.missing",
                "vendor.test-api.body.event_time.missing",
                "vendor.test-api.body.user_data.missing",
            ]
        );
    }

    #[test]
    fn a_finding_points_at_the_bytes_it_is_about() {
        let artifact =
            r#"{"data":[{"event_name":"A","event_time":17700000000000,"user_data":{}}]}"#;
        let report = body_pack().validate(&body_request(artifact));

        let violation = report
            .violations
            .iter()
            .find(|violation| violation.code.ends_with("event_time.invalid"))
            .expect("the timestamp is reported");
        let target = &violation.targets[0];

        assert_eq!(target.component, ViolationTargetComponent::BodyField);
        assert_eq!(target.name.as_deref(), Some("data[0].event_time"));
        assert_eq!(&artifact[target.start..target.end], "17700000000000");
    }

    #[test]
    fn a_missing_field_names_its_path_and_points_at_its_container() {
        let artifact = r#"{"data":[{"event_name":"A","user_data":{"em":["ffff"]}}]}"#;
        let report = body_pack().validate(&body_request(artifact));

        let violation = report
            .violations
            .iter()
            .find(|violation| violation.code.ends_with("event_time.missing"))
            .expect("the missing timestamp is reported");
        let target = &violation.targets[0];

        // The name says exactly which field is absent; the span is the event it
        // belongs in, since the field itself has no bytes to point at.
        assert_eq!(target.name.as_deref(), Some("data[0].event_time"));
        assert_eq!(
            &artifact[target.start..target.end],
            r#"{"event_name":"A","user_data":{"em":["ffff"]}}"#
        );
    }

    #[test]
    fn a_field_missing_from_several_places_is_reported_from_each() {
        let manifest = r#"{
            "id": "vendor/multi",
            "display_name": "Multi",
            "description": "Contracts a field that repeats inside one event.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"], "json_paths": ["ids[].kind"] },
            "body": {
                "params": [{ "name": "ids[].kind", "requirement": "required" }]
            }
        }"#;
        let pack = ManifestRulePack::from_json(manifest).expect("compiles");
        let report = pack.validate(&body_request(r#"{"ids":[{"kind":"a"},{},{}]}"#));

        // Two of the three entries omit it, so it is reported twice.
        assert_eq!(
            codes(&report),
            vec![
                "vendor.multi.body.ids[].kind.missing",
                "vendor.multi.body.ids[].kind.missing",
            ]
        );
    }

    #[test]
    fn a_field_under_a_missing_container_is_not_reported() {
        let manifest = r#"{
            "id": "vendor/nested",
            "display_name": "Nested",
            "description": "Contracts a container and the fields inside it.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"], "json_paths": ["event"] },
            "body": {
                "params": [
                    { "name": "user.ids", "requirement": "required" },
                    { "name": "user.ids[].kind", "requirement": "required" }
                ]
            }
        }"#;
        let pack = ManifestRulePack::from_json(manifest).expect("compiles");
        let report = pack.validate(&body_request(r#"{"event":"x","user":{"name":"a"}}"#));

        // `user.ids` is gone, so the contract on what lives inside it has
        // nothing to say. Reporting it too would be one defect told twice.
        assert_eq!(codes(&report), vec!["vendor.nested.body.user.ids.missing"]);
    }

    #[test]
    fn a_missing_container_does_not_cascade() {
        let report = body_pack().validate(&body_request(
            r#"{"data":[{"event_name":"A","event_time":1}]}"#,
        ));

        // `user_data` is absent, so it is reported once. The contract on
        // `user_data.em[]` underneath it stays quiet.
        assert_eq!(
            codes(&report),
            vec!["vendor.test-api.body.user_data.missing"]
        );
    }

    #[test]
    fn a_scalar_format_does_not_fire_on_the_list_form() {
        let report = body_pack().validate(&body_request(
            r#"{"data":[{"event_name":"A","event_time":1,"user_data":{"em":["ffff","zzzz"]}}]}"#,
        ));

        // The good element passes and the bad one is reported: the array itself
        // is never measured against a contract written for one value.
        assert_eq!(
            codes(&report),
            vec!["vendor.test-api.body.user_data.em[].invalid"]
        );
    }

    #[test]
    fn cross_field_rules_run_per_element() {
        let report = body_pack().validate(&body_request(
            r#"{"data":[
                {"event_name":"Purchase","event_time":1,"user_data":{"em":["ffff"]},"custom_data":{"value":1}},
                {"event_name":"Purchase","event_time":1,"user_data":{"em":["ffff"]}}
            ]}"#,
        ));

        assert_eq!(
            codes(&report),
            vec!["vendor.test-api.body.purchase_needs_value"]
        );
    }

    #[test]
    fn an_empty_batch_claims_nothing() {
        let pack = body_pack();
        let request = body_request(r#"{"data":[]}"#);

        // An empty array satisfies no shape, so the pack does not claim the
        // payload. Forcing it on anyway says so rather than inventing findings
        // about events that are not there.
        assert!(!pack.supports(&request));
        assert_eq!(
            codes(&pack.validate(&request)),
            vec!["vendor.test-api.payload_mismatch"]
        );
    }

    #[test]
    fn a_payload_of_the_wrong_shape_is_reported_when_the_pack_is_forced() {
        let report = body_pack().validate(&body_request(r#"{"events":[{"name":"A"}]}"#));

        assert_eq!(codes(&report), vec!["vendor.test-api.payload_mismatch"]);
        assert_eq!(report.detected_vendor, None);
    }

    #[test]
    fn a_body_that_does_not_parse_is_left_to_the_core_pack() {
        let report = body_pack().validate(&body_request(r#"{"data":[{"event_name":}]}"#));

        assert!(report.violations.is_empty());
    }

    #[test]
    fn a_url_artifact_still_takes_the_url_path() {
        let pack = body_pack();
        let request = ValidationRequest {
            artifact_kind: ArtifactKind::Url,
            artifact: "https://api.example.com/v1/events".to_string(),
            claimed_vendor: None,
            expansion_state: ExpansionState::Unknown,
        };

        assert!(pack.supports(&request));
        assert!(pack.validate(&request).violations.is_empty());
    }

    #[test]
    fn a_body_without_a_shape_to_match_is_rejected_at_load() {
        let manifest = r#"{
            "id": "vendor/loose",
            "display_name": "Loose",
            "description": "Claims every payload it is shown.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"] },
            "body": { "params": [{ "name": "a" }] }
        }"#;

        assert!(matches!(
            ManifestRulePack::from_json(manifest),
            Err(ManifestError::BodyWithoutShape(_))
        ));
    }

    #[test]
    fn a_shape_with_nothing_to_check_is_rejected_at_load() {
        let manifest = r#"{
            "id": "vendor/idle",
            "display_name": "Idle",
            "description": "Matches a shape and checks nothing.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"], "json_paths": ["data[].a"] }
        }"#;

        assert!(matches!(
            ManifestRulePack::from_json(manifest),
            Err(ManifestError::ShapeWithoutBody(_))
        ));
    }

    #[test]
    fn a_malformed_path_is_rejected_at_load() {
        let manifest = r#"{
            "id": "vendor/typo",
            "display_name": "Typo",
            "description": "Has a path that addresses nothing.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"], "json_paths": ["data[]."] },
            "body": { "params": [{ "name": "a" }] }
        }"#;

        assert!(matches!(
            ManifestRulePack::from_json(manifest),
            Err(ManifestError::InvalidJsonPath { .. })
        ));
    }

    #[test]
    fn url_and_body_may_contract_the_same_name() {
        let manifest = r#"{
            "id": "vendor/both",
            "display_name": "Both",
            "description": "Accepts the token in either place.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"], "json_paths": ["data[].a"] },
            "params": [{ "name": "access_token", "requirement": "required" }],
            "body": {
                "scope": "data[]",
                "params": [{ "name": "access_token", "requirement": "required" }]
            }
        }"#;

        let pack = ManifestRulePack::from_json(manifest).expect("compiles");
        let report = pack.validate(&body_request(r#"{"data":[{"a":1}]}"#));

        // Same field name, different carrier, so the codes have to differ.
        assert_eq!(
            codes(&report),
            vec!["vendor.both.body.access_token.missing"]
        );
    }
}
