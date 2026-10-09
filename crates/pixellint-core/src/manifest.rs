//! Declarative rulepack manifests.
//!
//! A manifest describes a vendor endpoint family as data: which hosts and paths
//! it covers, which parameters it contracts, and what each violation should say.
//! [`ManifestRulePack`] interprets a compiled manifest and implements the same
//! [`ValidatorPlugin`] trait as the hand-written `core` pack, so first-party
//! vendor packs and user-supplied packs run through one code path.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::TimestampUnit;
use crate::json::{self, JsonDocument, JsonField, JsonValueKind};
use crate::prepare::{self, ArtifactUrl, PreparedArtifact};
use crate::{
    ArtifactKind, PluginRouting, RulePackMetadata, RuleSource, RuleSourceLevel, Severity,
    ValidationReport, ValidationRequest, ValidatorPlugin, Violation, ViolationTarget,
    ViolationTargetComponent, detect_macro_spans,
};

/// How a pack's parameters are carried on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamStyle {
    /// Standard `?name=value&name=value` query parameters.
    #[default]
    Query,
    /// Query parameters that split on both `&` and `;`, the shape Adform
    /// impression and click tags use: `?bn=123;C=1` next to `?bn=123&v=3`.
    QuerySemicolon,
    /// Semicolon-delimited `name=value` pairs inside the path, as used by
    /// Floodlight activity tags.
    Matrix,
    /// Slash-delimited `key:value` path segments, as used by Partnerize
    /// conversion URLs. Basket containers wrap a group in `[...]`; the
    /// brackets are stripped so `category` and `quantity` still match.
    ColonPath,
}

impl ParamStyle {
    pub(crate) fn cache_index(self) -> usize {
        match self {
            Self::Query => 0,
            Self::QuerySemicolon => 1,
            Self::Matrix => 2,
            Self::ColonPath => 3,
        }
    }
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
    /// An IP address literal, optionally restricted to one address family.
    Ip {
        #[serde(default)]
        version: Option<IpVersion>,
        /// Vendor-specific address ranges that are not accepted.
        #[serde(default)]
        exclude_ranges: Vec<String>,
    },
    /// A calendar-valid RFC 3339 timestamp, optionally allowing local time.
    #[serde(rename = "datetime")]
    DateTime {
        #[serde(default)]
        require_timezone: bool,
        #[serde(default)]
        allow_date_only: bool,
        #[serde(default)]
        allow_basic: bool,
        #[serde(default)]
        allow_space_separator: bool,
        #[serde(default)]
        allow_javascript_date: bool,
        #[serde(default)]
        allow_unpadded_date: bool,
    },
    /// A calendar-valid ISO 8601 date in YYYY-MM-DD form.
    Date,
    /// JSON syntax inside a string, accepting any JSON value.
    Json,
    /// Adobe AppMeasurement semicolon product rows and merchandising cells.
    AdobeProducts,
    /// AppMeasurement event lists with source-defined names and assignments.
    AdobeEvents,
    /// Google's versioned Additional Consent provider lists.
    AdditionalConsent,
    /// Braze's explicit $time custom attribute date representations.
    BrazeTime,
    /// A currency assigned by the dated ISO 4217 registry.
    Currency {
        #[serde(default)]
        allow_historical: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    /// TCF v2 structural encoding, version, and mandatory-prefix checks.
    Tcf,
    /// GPP structural encoding and header type/version checks.
    Gpp,
    /// Published four-character US Privacy encoding.
    UsPrivacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpVersion {
    V4,
    V6,
}

/// Which artifacts a pack claims.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSpec {
    /// Self-hosted vendor endpoints, constrained by an explicit path selector.
    #[serde(default)]
    pub any_host: bool,
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
    /// At least one named query key must be submitted, including an empty value.
    #[serde(default)]
    pub query_params_any: Vec<String>,
    /// None of these query keys may be submitted.
    #[serde(default)]
    pub query_params_none: Vec<String>,
    /// JSON body shapes the pack claims. A body artifact belongs to this pack
    /// when every entry here holds, so the shape of the payload stands in for
    /// the host that a bare body does not carry.
    #[serde(default)]
    pub json_paths: Vec<ShapeMatch>,
}

/// One condition on the shape of a JSON body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
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
    /// At least one present value matches the supplied regular expression.
    Matches { path: String, pattern: String },
    /// A recognizable field with an actual JSON type.
    Type { path: String, json_type: JsonTypes },
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
            Self::Matches { path, .. } => vec![path],
            Self::Type { path, .. } => vec![path],
            Self::Any { any_of } => any_of.iter().flat_map(Self::paths).collect(),
        }
    }
}

/// The JSON type required by a body field, independently of its scalar format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
}

/// A single JSON type or an explicit union of allowed types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonTypes {
    One(JsonType),
    Any(Vec<JsonType>),
}

impl JsonTypes {
    fn types(&self) -> &[JsonType] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Any(values) => values,
        }
    }
}

/// A contracted parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamContract {
    pub name: String,
    /// Read this document-root JSON path into the scope-local logical name.
    #[serde(default)]
    pub root_path: Option<String>,
    /// Read a path relative to an ancestor of the selected body scope.
    #[serde(default)]
    pub ancestor_path: Option<AncestorPath>,
    /// When set, this contract applies to every present parameter whose name
    /// matches, instead of to `name` alone. Findings use the matched name.
    /// Required and recommended are rejected at load: absence of a family is
    /// not a missing parameter.
    #[serde(default)]
    pub name_pattern: Option<String>,
    /// On a body, match immediate member names under this relative object path.
    /// The empty path names the current scope.
    #[serde(default)]
    pub name_pattern_parent: Option<String>,
    /// Alternate spellings that satisfy the same contract.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// A scope-local guard for this field's presence and value checks.
    #[serde(default)]
    pub condition: Option<RuleCondition>,
    /// Vendor-documented normalization applied before string value checks.
    #[serde(default)]
    pub normalization: Option<StringNormalization>,
    #[serde(default)]
    pub requirement: Requirement,
    #[serde(default)]
    pub format: Option<ValueFormat>,
    /// Actual JSON type, rather than the text representation of the value.
    /// Only body contracts can declare this field.
    #[serde(default)]
    pub json_type: Option<JsonTypes>,
    #[serde(default)]
    pub min_items: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub min_properties: Option<usize>,
    #[serde(default)]
    pub max_properties: Option<usize>,
    /// Object keys excluded from the property count, for reserved vendor fields.
    #[serde(default)]
    pub property_exclusions: Vec<String>,
    /// Nested object/array depth, counting this container as level one.
    #[serde(default)]
    pub max_depth: Option<usize>,
    /// Total scalar values and array elements across immediate object members.
    #[serde(default)]
    pub max_member_values: Option<usize>,
    /// Limit a string array after joining its decoded elements.
    #[serde(default)]
    pub max_joined_length: Option<usize>,
    #[serde(default)]
    pub join_separator: Option<String>,
    /// Compact UTF-8 JSON bytes of this selected value, excluding whitespace.
    #[serde(default)]
    pub max_compact_bytes: Option<usize>,
    /// Constraints on the decoded object member name at the selected location.
    #[serde(default)]
    pub max_key_length: Option<usize>,
    #[serde(default)]
    pub key_pattern: Option<String>,
    /// String length in Unicode scalar values after JSON or URL decoding.
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    /// Decoded UTF-16 code units, matching JavaScript String.length.
    #[serde(default)]
    pub max_utf16_length: Option<usize>,
    /// String length in decoded UTF-8 bytes, distinct from character count.
    #[serde(default)]
    pub min_byte_length: Option<usize>,
    #[serde(default)]
    pub max_byte_length: Option<usize>,
    /// Inclusive numeric bounds. URL values are parsed as JSON numbers.
    #[serde(default)]
    pub minimum: Option<serde_json::Number>,
    #[serde(default)]
    pub maximum: Option<serde_json::Number>,
    /// The numeric value must be an exact multiple of this positive number.
    #[serde(default)]
    pub multiple_of: Option<serde_json::Number>,
    /// Apply numeric bounds to string members of a number/string type union.
    #[serde(default)]
    pub numeric_strings: bool,
    /// Protobuf JSON treats null fields as unset, without changing array members.
    #[serde(default)]
    pub null_as_missing: bool,
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
    /// When true, a blank value is treated as an unfilled template slot and is
    /// not reported. Format checks still run on a populated value. Floodlight
    /// `npa` and `tfua` ship this way on exported tags.
    #[serde(default)]
    pub allow_empty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringNormalization {
    Trim,
    TrimLowercase,
}

fn normalize_string(value: &str, normalization: Option<StringNormalization>) -> Cow<'_, str> {
    match normalization {
        None => Cow::Borrowed(value),
        Some(StringNormalization::Trim) => Cow::Borrowed(value.trim()),
        Some(StringNormalization::TrimLowercase) => Cow::Owned(value.trim().to_lowercase()),
    }
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
    /// When `when` carries one of `equals` and `param` is present, `param` must
    /// equal `value`. Missing `param` is left to a presence contract. Macro
    /// values never trigger it.
    ValueWhen {
        when: String,
        equals: Vec<String>,
        param: String,
        value: String,
    },
    /// When `when` carries one of `equals`, none of `params` may be present.
    ForbiddenWhenValue {
        when: String,
        equals: Vec<String>,
        params: Vec<String>,
    },
    /// At least one complete group must be present.
    RequireAnyOf {
        groups: Vec<Vec<String>>,
        /// Count present empty values when the vendor requires keys alone.
        #[serde(default)]
        allow_empty: bool,
    },
    /// Apply a scalar format only for the specified discriminator values.
    FormatWhen {
        when: String,
        equals: Vec<String>,
        param: String,
        format: ValueFormat,
        #[serde(default)]
        pair_occurrences: bool,
    },
    /// Apply a scalar format independently of a discriminator.
    Format { param: String, format: ValueFormat },
    /// This URL parameter must be the final parameter carried on the wire.
    LastParam { param: String },
    /// Limit the raw query text after the question mark, before any fragment.
    MaxQueryLength { max_length: usize },
    /// Require a literal HTTPS scheme on this URL endpoint.
    RequireHttps,
    /// Limit the entire trimmed URL's Unicode characters.
    MaxUrlLength { max_length: usize },
    /// Limit the complete JSON request's UTF-8 bytes, including whitespace.
    MaxBodyBytes { max_bytes: usize },
    /// When a parameter is present, another present parameter must equal a value.
    ValueWith {
        when: String,
        param: String,
        value: String,
    },
    /// The minimum string length is supplied by another field, or a default.
    MinLengthFrom {
        params: Vec<String>,
        length_param: String,
        default_length: usize,
    },
    /// Both fields must be numeric, and the left cannot exceed the right.
    LessEqual {
        left: String,
        right: String,
        #[serde(default)]
        strict: bool,
        #[serde(default)]
        unit: Option<TimestampUnit>,
    },
    /// Both populated literal scalar fields must carry the same value.
    EqualValues { left: String, right: String },
    /// Limit a URL field's path, excluding its origin, query and fragment.
    UrlPathLength { param: String, max_length: usize },
    /// Present delimited lists must describe the same number of records.
    EqualSplitLengths {
        params: Vec<String>,
        separator: String,
    },
    /// Exact sum of amounts in a delimited key/value list.
    DelimitedSum {
        param: String,
        total: String,
        separator: String,
        value_separator: String,
    },
    /// Published Awin product index or commission-group relationships.
    AwinBasket {
        parts: String,
        check: AwinBasketCheck,
    },
    /// Applicable GPP IDs must occur in the encoded header's section list.
    GppSections { param: String, sections: String },
    /// Destination-specific positive TCF consent for a vendor and purposes.
    TcfConsent {
        param: String,
        vendor_id: u16,
        purpose_ids: Vec<u8>,
    },
    /// Destination-specific choices in applicable, published US GPP fields.
    GppFieldValues {
        param: String,
        section_param: String,
        field: String,
        values: Vec<u8>,
    },
    /// Present native JSON arrays must describe the same number of records.
    EqualArrayLengths { params: Vec<String> },
    /// Literal scalar values of this object member must be unique in an array.
    UniqueArrayBy { param: String, field: String },
    /// Repeated parameter families must have the same occurrence count.
    EqualOccurrences { params: Vec<String> },
    /// A timestamp must fall within the permitted window around validation time.
    TimeWindow {
        param: String,
        /// Use this field only when the primary field is absent in the scope.
        #[serde(default)]
        fallback_param: Option<String>,
        unit: TimestampUnit,
        #[serde(default)]
        max_age_seconds: Option<u64>,
        #[serde(default)]
        max_future_seconds: Option<u64>,
    },
}

/// An optional guard for a cross-field assertion, evaluated within its scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleCondition {
    Exists { param: String },
    Present { param: String },
    ValueIn { param: String, values: Vec<String> },
    ValuePattern { param: String, pattern: String },
    JsonType { param: String, json_type: JsonTypes },
    All { conditions: Vec<RuleCondition> },
    Any { conditions: Vec<RuleCondition> },
    Not { condition: Box<RuleCondition> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AwinBasketCheck {
    IndexSequence,
    GroupMembership,
}

impl RuleCondition {
    fn params(&self) -> Vec<&String> {
        match self {
            Self::Exists { param }
            | Self::Present { param }
            | Self::ValueIn { param, .. }
            | Self::ValuePattern { param, .. }
            | Self::JsonType { param, .. } => vec![param],
            Self::All { conditions } | Self::Any { conditions } => {
                conditions.iter().flat_map(Self::params).collect()
            }
            Self::Not { condition } => condition.params(),
        }
    }

    // A macro has an unknown value. Negation must not turn it into a positive
    // match and accidentally apply a vendor requirement to a template.
    fn evaluate(&self, params: &[RawParam<'_>]) -> Option<bool> {
        match self {
            Self::Exists { param } => Some(params.iter().any(|field| {
                (!field.missing || field.json_kind.is_some()) && field.name.as_ref() == param
            })),
            Self::ValuePattern { param, pattern } => {
                let regex = Regex::new(pattern).expect("condition regex validated at compilation");
                let mut unknown = false;
                for field in params
                    .iter()
                    .filter(|field| !field.missing && field.name.as_ref() == param)
                {
                    if contains_macro(field.value.as_ref()) {
                        unknown = true;
                    } else if !field.container && regex.is_match(field.value.as_ref()) {
                        return Some(true);
                    }
                }
                if unknown { None } else { Some(false) }
            }
            Self::JsonType { param, json_type } => {
                let mut unknown = false;
                for field in params
                    .iter()
                    .filter(|field| !field.missing && field.name.as_ref() == param)
                {
                    if !field.container && contains_macro(field.value.as_ref()) {
                        unknown = true;
                        continue;
                    }
                    let matched = json_type.types().iter().any(|kind| match kind {
                        JsonType::String => field.json_kind == Some(JsonValueKind::String),
                        JsonType::Number => field.json_kind == Some(JsonValueKind::Number),
                        JsonType::Integer => {
                            field.json_kind == Some(JsonValueKind::Number)
                                && decimal_parts(field.value.as_ref())
                                    .is_some_and(|(_, _, exponent)| exponent >= 0)
                        }
                        JsonType::Boolean => field.json_kind == Some(JsonValueKind::Bool),
                        JsonType::Object => field.json_kind == Some(JsonValueKind::Object),
                        JsonType::Array => field.json_kind == Some(JsonValueKind::Array),
                        JsonType::Null => field.json_kind == Some(JsonValueKind::Null),
                    });
                    if matched {
                        return Some(true);
                    }
                }
                if unknown { None } else { Some(false) }
            }
            Self::Present { param } => Some(params.iter().any(|field| {
                !field.missing
                    && field.name.as_ref() == param
                    && (field.container || !field.value.is_empty())
            })),
            Self::ValueIn { param, values } => {
                let mut unknown = false;
                for field in params
                    .iter()
                    .filter(|field| !field.missing && field.name.as_ref() == param)
                {
                    if contains_macro(field.value.as_ref()) {
                        unknown = true;
                    } else if values.iter().any(|value| {
                        value == field.value.as_ref()
                            || field.json_kind == Some(JsonValueKind::Number)
                                && compare_numeric_text(value, field.value.as_ref())
                                    == Some(std::cmp::Ordering::Equal)
                    }) {
                        return Some(true);
                    }
                }
                if unknown { None } else { Some(false) }
            }
            Self::All { conditions } => {
                let values: Vec<_> = conditions
                    .iter()
                    .map(|condition| condition.evaluate(params))
                    .collect();
                if values.contains(&Some(false)) {
                    Some(false)
                } else if values.contains(&None) {
                    None
                } else {
                    Some(true)
                }
            }
            Self::Any { conditions } => {
                let values: Vec<_> = conditions
                    .iter()
                    .map(|condition| condition.evaluate(params))
                    .collect();
                if values.contains(&Some(true)) {
                    Some(true)
                } else if values.contains(&None) {
                    None
                } else {
                    Some(false)
                }
            }
            Self::Not { condition } => condition.evaluate(params).map(|value| !value),
        }
    }
}

/// An ancestor-relative field, retaining its actual document location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AncestorPath {
    pub levels: usize,
    pub path: String,
}

fn validate_condition(
    pack_id: &str,
    name: &str,
    condition: &RuleCondition,
) -> Result<(), ManifestError> {
    let invalid = |reason: &str| ManifestError::InvalidConstraint {
        pack_id: pack_id.to_string(),
        name: name.to_string(),
        reason: reason.to_string(),
    };
    match condition {
        RuleCondition::ValuePattern { pattern, .. } => {
            compile_regex(pack_id, pattern)?;
        }
        RuleCondition::JsonType { json_type, .. } if json_type.types().is_empty() => {
            return Err(invalid("json_type condition needs at least one type"));
        }
        RuleCondition::All { conditions } | RuleCondition::Any { conditions } => {
            if conditions.is_empty() {
                return Err(invalid("condition group must not be empty"));
            }
            for condition in conditions {
                validate_condition(pack_id, name, condition)?;
            }
        }
        RuleCondition::Not { condition } => validate_condition(pack_id, name, condition)?,
        _ => {}
    }
    Ok(())
}

fn validate_format(pack_id: &str, name: &str, format: &ValueFormat) -> Result<(), ManifestError> {
    if let ValueFormat::Enum { values, .. } = format
        && values.is_empty()
    {
        return Err(ManifestError::EmptyFormatValues {
            pack_id: pack_id.to_string(),
            name: name.to_string(),
        });
    }
    if let ValueFormat::Ip { exclude_ranges, .. } = format {
        for range in exclude_ranges {
            if parse_cidr(range).is_none() {
                return Err(ManifestError::InvalidConstraint {
                    pack_id: pack_id.to_string(),
                    name: name.to_string(),
                    reason: format!("invalid excluded CIDR range `{range}`"),
                });
            }
        }
    }
    Ok(())
}

/// A pack-level rule that spans more than one parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRule {
    /// Stable rule id. Must start with the pack's code prefix.
    pub code: String,
    #[serde(flatten)]
    pub assertion: Assertion,
    #[serde(default)]
    pub condition: Option<RuleCondition>,
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
    /// Destination-documented alternate values meaning GDPR does not apply.
    #[serde(default)]
    pub gdpr_non_applicable_values: Vec<String>,
    /// Destination-documented alternative query names carrying a TC String.
    #[serde(default)]
    pub gdpr_consent_aliases: Vec<String>,
    /// URL presence checked by declared HTTP alternatives on complete captures.
    #[serde(default)]
    pub http_url_presence_overrides: Vec<String>,
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
    /// Fragment keys read by a documented browser loader. This declaration
    /// changes only core's server-delivery warning, not value contracts.
    #[serde(default)]
    pub client_fragment_params: Vec<String>,
    #[serde(rename = "match")]
    pub matcher: MatchSpec,
    #[serde(default)]
    pub params: Vec<ParamContract>,
    #[serde(default)]
    pub rules: Vec<PackRule>,
    /// Contracts evaluated separately in raw semicolon-separated query groups.
    #[serde(default)]
    pub query_scopes: Vec<QuerySpec>,
    /// Contracts on the JSON request body, for endpoints that carry their
    /// payload there rather than in the query string. A list declares more than
    /// one, which is how a pack contracts both the envelope and the events
    /// inside it.
    #[serde(default)]
    pub body: Option<BodySpecs>,
    /// Complete-request contracts. The namespace includes method, headers,
    /// URL query fields, and a decoded body. Bare artifacts do not run these.
    #[serde(default)]
    pub http: Option<BodySpecs>,
    /// Query-only event strings inside a captured bulk request. Each item uses
    /// this pack's existing URL contracts, bound to the captured endpoint.
    #[serde(default)]
    pub http_queries: Vec<RequestQuerySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestQuerySpec {
    pub source_field: String,
    #[serde(default)]
    pub encoding: RequestQueryEncoding,
    #[serde(default)]
    pub inherited_params: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub inherited_overrides: bool,
    /// Opt-in native map reader with explicitly sourced server semantics.
    #[serde(default)]
    pub native_map: Option<RequestQueryMapCoercion>,
    /// Advisory oldest-first order for explicit Matomo cdt values.
    #[serde(default)]
    pub check_chronological_order: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestQueryMapCoercion {
    MatomoPhp8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestQueryEncoding {
    #[default]
    Query,
    UrlQuery,
}

/// A namespace evaluated once per selected semicolon-separated query group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuerySpec {
    /// Query semicolon groups by default, or independent bracketed path items.
    #[serde(default)]
    pub source: QuerySource,
    /// Read these declared logical parameters from outside all path items.
    #[serde(default)]
    pub global_params: Vec<String>,
    /// Skip initial global or targeting groups without renumbering later groups.
    #[serde(default)]
    pub first_index: usize,
    /// Include groups through this absolute index. Omitted means no upper bound.
    #[serde(default)]
    pub last_index: Option<usize>,
    /// Select groups using values within that group only.
    #[serde(default)]
    pub condition: Option<RuleCondition>,
    #[serde(default)]
    pub params: Vec<ParamContract>,
    #[serde(default)]
    pub rules: Vec<PackRule>,
    /// These fields must have distinct literal values across selected groups.
    /// The field contract supplies the finding's citation and severity.
    #[serde(default)]
    pub unique_by: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuerySource {
    #[default]
    QuerySemicolon,
    PathItems,
}

/// One body contract, or several at different levels of the same payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum BodySpecs {
    One(Box<BodySpec>),
    Many(Vec<BodySpec>),
}

impl<'de> Deserialize<'de> for BodySpecs {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_array() {
            serde_json::from_value(value)
                .map(Self::Many)
                .map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Self::One)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl BodySpecs {
    fn specs(&self) -> &[BodySpec] {
        match self {
            Self::One(spec) => std::slice::from_ref(spec.as_ref()),
            Self::Many(specs) => specs,
        }
    }
}

/// Which part of a body the contracts are written against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScopeSpec {
    /// One batch array, such as `data[]`.
    One(String),
    /// Alternative envelopes, tried in order, first one that is present wins. An
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
    /// Decode this URL parameter and validate its JSON value as the body.
    #[serde(default)]
    pub source_param: Option<String>,
    /// Decode a JSON string field in the request body, or inside source_param.
    #[serde(default)]
    pub source_field: Option<String>,
    /// Decode this second string field inside the first decoded field.
    #[serde(default)]
    pub decoded_source_field: Option<String>,
    /// Select the nested representation using the first decoded document.
    #[serde(default)]
    pub decoded_source_condition: Option<ShapeMatch>,
    /// Maximum Unicode characters in the original source_field string.
    #[serde(default)]
    pub source_max_length: Option<usize>,
    /// Apply source_max_length when this first-decoded-document shape holds.
    #[serde(default)]
    pub source_max_length_when: Option<ShapeMatch>,
    /// A URL-scope guard for a body decoded from a query parameter.
    #[serde(default)]
    pub condition: Option<RuleCondition>,
    #[serde(default)]
    pub encoding: ParamEncoding,
    /// Severity of encoded-source syntax failures. Omission preserves errors.
    #[serde(default)]
    pub encoding_severity: Option<Severity>,
    /// Inner field encoding when source_param and source_field are combined.
    #[serde(default)]
    pub field_encoding: ParamEncoding,
    #[serde(default)]
    pub scope: Option<ScopeSpec>,
    /// Absolute JSON paths excluded along with their descendants from this scope.
    #[serde(default)]
    pub scope_exclusions: Vec<String>,
    #[serde(default)]
    pub params: Vec<ParamContract>,
    #[serde(default)]
    pub rules: Vec<PackRule>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamEncoding {
    #[default]
    Json,
    Base64Json,
    /// Browser btoa JSON, with every decoded byte mapped to its Latin-1 code point.
    Base64Latin1Json,
    /// Pipe-separated tuple components, preserved as JSON strings.
    PipeDelimitedJson,
    /// One strict encodeURIComponent layer over UTF-8 JSON, preserving plus.
    PercentEncodedJson,
    /// Form-style query fields represented as JSON strings or repeated arrays.
    QueryParamsJson,
}

impl ParamEncoding {
    fn decoded_byte_length(self, source: &str, text: &str) -> usize {
        match self {
            Self::Base64Latin1Json => text.chars().count(),
            Self::PipeDelimitedJson | Self::QueryParamsJson => source.len(),
            Self::Json | Self::Base64Json | Self::PercentEncodedJson => text.len(),
        }
    }

    fn decode(self, source: &str) -> Result<String, String> {
        match self {
            Self::Json => Ok(source.to_string()),
            Self::Base64Json => decode_base64_json(source),
            Self::Base64Latin1Json => decode_base64_latin1_json(source),
            Self::PercentEncodedJson => decode_percent_encoded_json(source),
            Self::QueryParamsJson => decode_query_params_json(source),
            Self::PipeDelimitedJson => {
                serde_json::to_string(&source.split('|').collect::<Vec<_>>())
                    .map_err(|error| error.to_string())
            }
        }
    }
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
    NamePatternNotOptional {
        pack_id: String,
        name: String,
    },
    NamePatternWithAliases {
        pack_id: String,
        name: String,
    },
    NamePatternOnBody {
        pack_id: String,
        name: String,
    },
    InvalidConstraint {
        pack_id: String,
        name: String,
        reason: String,
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
            Self::NamePatternNotOptional { pack_id, name } => write!(
                f,
                "rulepack `{pack_id}` parameter `{name}` uses `name_pattern`, which can only check values that are already present, so it cannot be required or recommended"
            ),
            Self::NamePatternWithAliases { pack_id, name } => write!(
                f,
                "rulepack `{pack_id}` parameter `{name}` uses `name_pattern` and cannot also declare aliases"
            ),
            Self::NamePatternOnBody { pack_id, name } => write!(
                f,
                "rulepack `{pack_id}` parameter `{name}` uses `name_pattern` on a JSON body, which only matches exact field paths"
            ),
            Self::InvalidConstraint {
                pack_id,
                name,
                reason,
            } => write!(
                f,
                "rulepack `{pack_id}` parameter `{name}` has invalid constraints: {reason}"
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
    name_regex: Option<Regex>,
    key_regex: Option<Regex>,
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
    /// The artifact being checked. Fallback targets borrow spans from it and
    /// are built only when a contract fires.
    artifact: &'a str,
    raw_artifact_len: usize,
    reference_time_unix_seconds: i64,
    /// The payload being read, when this scope is a body rather than a URL.
    body: Option<BodyScope<'a>>,
    query: Option<QuerySpan>,
}

#[derive(Clone, Copy)]
struct QuerySpan {
    index: usize,
    start: usize,
    end: usize,
}

/// The document and the element a body scope is evaluating.
struct BodyScope<'a> {
    document: &'a JsonDocument<'a>,
    path: &'a str,
}

impl Scope<'_> {
    fn field(&self, name: &str) -> String {
        if let Some(query) = self.query {
            return format!("{}[{}].{name}", self.code_segment, query.index);
        }
        match &self.body {
            None => format!("param.{name}"),
            Some(body) if body.path.is_empty() => format!("{}.{name}", self.code_segment),
            Some(body) => format!("{}.{}.{name}", self.code_segment, body.path),
        }
    }

    fn fallback(&self) -> ViolationTarget {
        if let Some(query) = self.query {
            return ViolationTarget {
                component: if self.code_segment == "path_items" {
                    ViolationTargetComponent::Path
                } else {
                    ViolationTargetComponent::QueryParam
                },
                name: Some(format!("{}[{}]", self.code_segment, query.index)),
                value: None,
                start: query.start,
                end: query.end,
            };
        }
        match &self.body {
            Some(body) => body_target(body.document, body.path, self.artifact),
            None => whole_url_target(self.artifact),
        }
    }

    /// Where to point for a finding about a field that carries no value of its
    /// own. A missing `user_data.em` is most useful pointed at the `user_data`
    /// that does exist, rather than at the whole event.
    fn fallback_for(&self, name: &str) -> ViolationTarget {
        let Some(body) = &self.body else {
            return self.fallback();
        };

        let path = match body.path.is_empty() {
            true => name.to_string(),
            false => format!("{}.{name}", body.path),
        };

        match body.document.nearest_present_ancestor(&path) {
            Some((ancestor, field)) => ViolationTarget {
                component: ViolationTargetComponent::BodyField,
                name: Some(ancestor.to_string()),
                value: None,
                start: field.start,
                end: field.end,
            },
            None => self.fallback(),
        }
    }
}

/// A shape condition with its patterns compiled.
#[derive(Debug)]
enum CompiledShape {
    Present(String),
    Excludes { path: String, regex: Regex },
    Matches { path: String, regex: Regex },
    Type { path: String, json_type: JsonTypes },
    Any(Vec<CompiledShape>),
}

impl CompiledShape {
    fn holds(&self, document: &JsonDocument<'_>) -> bool {
        match self {
            Self::Present(path) => document.matches_pattern(path),
            Self::Excludes { path, regex } => !document
                .expand(path)
                .iter()
                .filter_map(|concrete| document.get(concrete))
                .any(|field| regex.is_match(&field.text)),
            Self::Matches { path, regex } => document
                .expand(path)
                .iter()
                .filter_map(|path| document.get(path))
                .any(|field| regex.is_match(&field.text)),
            Self::Type { path, json_type } => document
                .expand(path)
                .iter()
                .filter_map(|path| document.get(path))
                .any(|field| {
                    json_type
                        .types()
                        .iter()
                        .any(|kind| matches_json_type(*kind, field))
                }),
            Self::Any(alternatives) => alternatives.iter().any(|shape| shape.holds(document)),
        }
    }
}

#[derive(Debug)]
struct CompiledBody {
    code_segment: &'static str,
    source_param: Option<String>,
    source_field: Option<String>,
    decoded_source_field: Option<String>,
    decoded_source_condition: Option<CompiledShape>,
    source_max_length: Option<usize>,
    source_max_length_when: Option<CompiledShape>,
    condition: Option<RuleCondition>,
    encoding: ParamEncoding,
    encoding_severity: Severity,
    field_encoding: ParamEncoding,
    scope: Option<ScopeSpec>,
    scope_exclusions: Vec<String>,
    params: Vec<CompiledParam>,
    rules: Vec<CompiledRule>,
    exact_names: BTreeSet<String>,
}

#[derive(Debug)]
struct CompiledQuery {
    spec: QuerySpec,
    params: Vec<CompiledParam>,
    rules: Vec<CompiledRule>,
    exact_names: BTreeSet<String>,
}

fn strongest_encoding_severity<'a>(bodies: impl Iterator<Item = &'a CompiledBody>) -> Severity {
    bodies
        .map(|body| body.encoding_severity)
        .max_by_key(|severity| match severity {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Error => 2,
        })
        .unwrap_or(Severity::Error)
}

/// A rulepack compiled from a [`RulePackManifest`].
#[derive(Debug)]
pub struct ManifestRulePack {
    metadata: RulePackMetadata,
    gdpr_non_applicable_values: Vec<String>,
    gdpr_consent_aliases: Vec<String>,
    http_url_presence_overrides: BTreeSet<String>,
    code_prefix: String,
    vendor: Option<String>,
    docs: Option<String>,
    param_style: ParamStyle,
    path_pattern: Option<Regex>,
    client_fragment_params: BTreeSet<String>,
    matcher: MatchSpec,
    params: Vec<CompiledParam>,
    exact_param_names: BTreeSet<String>,
    rules: Vec<CompiledRule>,
    bodies: Vec<CompiledBody>,
    queries: Vec<CompiledQuery>,
    shapes: Vec<CompiledShape>,
    http: Option<Box<ManifestRulePack>>,
    http_queries: Vec<RequestQuerySpec>,
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
        ShapeMatch::Matches { path, pattern } => CompiledShape::Matches {
            path: path.clone(),
            regex: compile_regex(pack_id, pattern)?,
        },
        ShapeMatch::Type { path, json_type } => {
            if json_type.types().is_empty() {
                return Err(ManifestError::InvalidConstraint {
                    pack_id: pack_id.to_string(),
                    name: path.clone(),
                    reason: "shape json_type must not be empty".to_string(),
                });
            }
            CompiledShape::Type {
                path: path.clone(),
                json_type: json_type.clone(),
            }
        }
        ShapeMatch::Any { any_of } => CompiledShape::Any(compile_shapes(pack_id, any_of)?),
    })
}

/// Compiles one list of parameter contracts, returning the names it defines so
/// rules can be checked against them.
fn validate_constraints(
    pack_id: &str,
    contract: &ParamContract,
    is_body: bool,
) -> Result<(), ManifestError> {
    let reject = |reason: &str| ManifestError::InvalidConstraint {
        pack_id: pack_id.to_string(),
        name: contract.name.clone(),
        reason: reason.to_string(),
    };
    if contract.name_pattern_parent.is_some() && (!is_body || contract.name_pattern.is_none()) {
        return Err(reject("name_pattern_parent requires a body name_pattern"));
    }
    if let Some(path) = &contract.root_path {
        if !is_body
            || contract.name_pattern.is_some()
            || !contract.aliases.is_empty()
            || contract.ancestor_path.is_some()
        {
            return Err(reject(
                "root_path requires an exact body contract without aliases",
            ));
        }
        if !path.is_empty() && !json::is_valid_pattern(path) {
            return Err(reject("root_path must be a JSON path"));
        }
    }
    if let Some(ancestor) = &contract.ancestor_path {
        if !is_body || contract.name_pattern.is_some() || !contract.aliases.is_empty() {
            return Err(reject(
                "ancestor_path requires an exact body contract without aliases",
            ));
        }
        if ancestor.levels == 0
            || (!ancestor.path.is_empty() && !json::is_valid_pattern(&ancestor.path))
        {
            return Err(reject(
                "ancestor_path needs positive levels and a valid JSON path",
            ));
        }
    }
    if let Some(parent) = &contract.name_pattern_parent
        && !parent.is_empty()
        && !json::is_valid_pattern(parent)
    {
        return Err(reject("name_pattern_parent must be a JSON object path"));
    }
    if !is_body
        && (contract.null_as_missing
            || contract.json_type.is_some()
            || contract.min_items.is_some()
            || contract.max_items.is_some()
            || contract.min_properties.is_some()
            || contract.max_properties.is_some()
            || !contract.property_exclusions.is_empty()
            || contract.max_depth.is_some()
            || contract.max_member_values.is_some()
            || contract.max_joined_length.is_some()
            || contract.join_separator.is_some()
            || contract.max_compact_bytes.is_some()
            || contract.max_key_length.is_some()
            || contract.key_pattern.is_some()
            || contract.name.is_empty())
    {
        return Err(reject(
            "JSON types, container limits, and the root path are body-only",
        ));
    }
    if contract.max_joined_length.is_some() != contract.join_separator.is_some() {
        return Err(reject(
            "max_joined_length and join_separator must be declared together",
        ));
    }
    if !contract.property_exclusions.is_empty()
        && contract.min_properties.is_none()
        && contract.max_properties.is_none()
    {
        return Err(reject(
            "property_exclusions requires an object property limit",
        ));
    }
    if contract
        .json_type
        .as_ref()
        .is_some_and(|types| types.types().is_empty())
    {
        return Err(reject("json_type cannot be an empty union"));
    }
    for (min, max, label) in [
        (contract.min_items, contract.max_items, "array size"),
        (
            contract.min_properties,
            contract.max_properties,
            "object size",
        ),
        (contract.min_length, contract.max_length, "string length"),
        (
            contract.min_byte_length,
            contract.max_byte_length,
            "string byte length",
        ),
    ] {
        if let (Some(min), Some(max)) = (min, max)
            && min > max
        {
            return Err(reject(&format!("minimum {label} exceeds maximum")));
        }
    }
    if let (Some(min), Some(max)) = (&contract.minimum, &contract.maximum)
        && compare_numbers(min, max) == Some(std::cmp::Ordering::Greater)
    {
        return Err(reject("minimum exceeds maximum"));
    }
    for bound in [&contract.minimum, &contract.maximum, &contract.multiple_of]
        .into_iter()
        .flatten()
    {
        if compare_numbers(bound, bound).is_none() {
            return Err(reject("numeric bound exponent exceeds the supported range"));
        }
    }
    if let Some(step) = &contract.multiple_of
        && compare_numbers(step, &serde_json::Number::from(0)) != Some(std::cmp::Ordering::Greater)
    {
        return Err(reject("multiple_of must be positive"));
    }
    if let Some(step) = &contract.multiple_of
        && decimal_parts(&step.to_string())
            .is_none_or(|(_, digits, _)| digits.parse::<u128>().is_err())
    {
        return Err(reject(
            "multiple_of significand exceeds the supported 128-bit range",
        ));
    }
    if let Some(types) = &contract.json_type {
        for (present, allowed, label) in [
            (
                contract.min_items.is_some() || contract.max_items.is_some(),
                vec![JsonType::Array],
                "array limits",
            ),
            (
                contract.min_properties.is_some()
                    || contract.max_properties.is_some()
                    || contract.max_member_values.is_some(),
                vec![JsonType::Object],
                "object limits",
            ),
            (
                contract.max_depth.is_some(),
                vec![JsonType::Object, JsonType::Array],
                "nesting depth",
            ),
            (
                contract.min_length.is_some()
                    || contract.max_length.is_some()
                    || contract.max_utf16_length.is_some()
                    || contract.min_byte_length.is_some()
                    || contract.max_byte_length.is_some(),
                vec![JsonType::String],
                "string limits",
            ),
            (
                contract.minimum.is_some()
                    || contract.maximum.is_some()
                    || contract.multiple_of.is_some(),
                vec![JsonType::Number, JsonType::Integer, JsonType::String],
                "numeric limits",
            ),
        ] {
            if present && !types.types().iter().any(|kind| allowed.contains(kind)) {
                return Err(reject(&format!("{label} do not apply to json_type")));
            }
        }
    }
    Ok(())
}

fn compile_params(
    pack_id: &str,
    manifest: &RulePackManifest,
    contracts: &[ParamContract],
    is_body: bool,
) -> Result<(Vec<CompiledParam>, BTreeSet<String>), ManifestError> {
    let mut seen_names = BTreeSet::new();
    let mut params = Vec::with_capacity(contracts.len());

    for contract in contracts {
        validate_constraints(pack_id, contract, is_body)?;
        if let Some(condition) = &contract.condition {
            validate_condition(pack_id, &contract.name, condition)?;
        }
        if let Some(format) = &contract.format {
            validate_format(pack_id, &contract.name, format)?;
        }
        if contract.name_pattern.is_some() {
            if !contract.aliases.is_empty() {
                return Err(ManifestError::NamePatternWithAliases {
                    pack_id: pack_id.to_string(),
                    name: contract.name.clone(),
                });
            }
            match contract.requirement {
                Requirement::Required | Requirement::Recommended => {
                    return Err(ManifestError::NamePatternNotOptional {
                        pack_id: pack_id.to_string(),
                        name: contract.name.clone(),
                    });
                }
                Requirement::Optional | Requirement::Forbidden | Requirement::Deprecated => {}
            }
        }

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

        let name_regex = match &contract.name_pattern {
            Some(pattern) => Some(compile_regex(pack_id, pattern)?),
            None => None,
        };
        let key_regex = contract
            .key_pattern
            .as_ref()
            .map(|pattern| compile_regex(pack_id, pattern))
            .transpose()?;

        let mut normalized_contract = contract.clone();
        if let Some(condition) = &mut normalized_contract.condition {
            normalize_condition_aliases(condition, contracts);
        }
        params.push(CompiledParam {
            contract: normalized_contract,
            names,
            regex,
            name_regex,
            key_regex,
            doc,
            source_level,
        });
    }

    for contract in contracts {
        for name in contract.condition.iter().flat_map(RuleCondition::params) {
            if !seen_names.contains(name) {
                return Err(ManifestError::UnknownParam {
                    pack_id: pack_id.to_string(),
                    code: format!("{}.condition", contract.name),
                    name: name.clone(),
                });
            }
        }
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
    contracts: &[ParamContract],
) -> Result<Vec<CompiledRule>, ManifestError> {
    let mut rules = Vec::with_capacity(pack_rules.len());

    for original_rule in pack_rules {
        let mut normalized_rule = original_rule.clone();
        for param in assertion_params_mut(&mut normalized_rule.assertion) {
            normalize_alias(param, contracts);
        }
        if let Some(condition) = &mut normalized_rule.condition {
            normalize_condition_aliases(condition, contracts);
        }
        let rule = &normalized_rule;
        if let Assertion::UniqueArrayBy { field, .. } = &rule.assertion
            && field.is_empty()
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "unique_array_by requires a nonempty literal object member name".into(),
            });
        }
        if let Assertion::GppFieldValues { field, values, .. } = &rule.assertion
            && (!crate::gpp_structure::us_field_supported(field) || values.is_empty())
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "GPP field choice checks need a field name and permitted values".into(),
            });
        }
        if let Assertion::TcfConsent {
            vendor_id,
            purpose_ids,
            ..
        } = &rule.assertion
            && (*vendor_id == 0 || purpose_ids.iter().any(|id| !(1..=24).contains(id)))
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "TCF vendor ID must be positive and purpose IDs must be 1 through 24"
                    .into(),
            });
        }
        if let Some(condition) = &rule.condition {
            validate_condition(pack_id, &rule.code, condition)?;
        }
        if let Assertion::FormatWhen { format, .. } | Assertion::Format { format, .. } =
            &rule.assertion
        {
            validate_format(pack_id, &rule.code, format)?;
        }
        if let Assertion::EqualOccurrences { params } | Assertion::EqualArrayLengths { params } =
            &rule.assertion
            && params.len() < 2
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "array length and occurrence equality require at least two fields"
                    .to_string(),
            });
        }
        if let Assertion::TimeWindow {
            max_age_seconds,
            max_future_seconds,
            ..
        } = &rule.assertion
            && (max_age_seconds.is_none() && max_future_seconds.is_none())
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "time_window requires an age or future bound".to_string(),
            });
        }
        if let Assertion::EqualSplitLengths { params, separator } = &rule.assertion
            && (params.len() < 2 || separator.is_empty())
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "equal_split_lengths requires two fields and a nonempty separator"
                    .to_string(),
            });
        }
        if let Assertion::DelimitedSum {
            separator,
            value_separator,
            ..
        } = &rule.assertion
            && (separator.is_empty() || value_separator.is_empty())
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "delimited_sum requires nonempty separators".to_string(),
            });
        }
        if let Assertion::RequireAnyOf { groups, .. } = &rule.assertion
            && (groups.is_empty() || groups.iter().any(Vec::is_empty))
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id: pack_id.to_string(),
                name: rule.code.clone(),
                reason: "require_any_of needs nonempty groups".to_string(),
            });
        }
        if !rule.code.starts_with(&format!("{code_prefix}.")) {
            return Err(ManifestError::CodePrefix {
                pack_id: pack_id.to_string(),
                code: rule.code.clone(),
                expected_prefix: format!("{code_prefix}."),
            });
        }

        for name in assertion_params(&rule.assertion)
            .into_iter()
            .chain(rule.condition.iter().flat_map(RuleCondition::params))
        {
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
            Assertion::FormatWhen {
                format: ValueFormat::Regex { pattern },
                ..
            }
            | Assertion::Format {
                format: ValueFormat::Regex { pattern },
                ..
            } => Some(compile_regex(pack_id, pattern)?),
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
    pub fn compile(mut manifest: RulePackManifest) -> Result<Self, ManifestError> {
        let http = if let Some(specs) = manifest.http.take() {
            let mut transport = manifest.clone();
            transport.params.clear();
            transport.rules.clear();
            transport.query_scopes.clear();
            transport.http_queries.clear();
            transport.path_pattern = None;
            transport.client_fragment_params.clear();
            transport.gdpr_non_applicable_values.clear();
            transport.gdpr_consent_aliases.clear();
            transport.http_url_presence_overrides.clear();
            transport.matcher.json_paths = vec![ShapeMatch::Present("method".into())];
            let mut bodies = specs.specs().to_vec();
            for spec in &mut bodies {
                if spec.source_param.is_some()
                    || spec.source_field.is_some()
                    || spec.decoded_source_field.is_some()
                    || spec.source_max_length.is_some()
                    || spec
                        .rules
                        .iter()
                        .any(|rule| matches!(rule.assertion, Assertion::MaxBodyBytes { .. }))
                {
                    return Err(ManifestError::Parse("http contracts use normalized request fields; decoding and raw body byte limits belong in body contracts".into()));
                }
                spec.condition = None;
            }
            transport.body = Some(BodySpecs::Many(bodies));
            let mut compiled = Self::compile(transport)?;
            for (body, original) in compiled.bodies.iter_mut().zip(specs.specs()) {
                body.code_segment = "http";
                if let Some(condition) = &original.condition {
                    validate_condition(&manifest.id, "http.condition", condition)?;
                    for name in condition.params() {
                        if !body.params.iter().any(|p| p.names.contains(name)) {
                            return Err(ManifestError::UnknownParam {
                                pack_id: manifest.id.clone(),
                                code: "http.condition".into(),
                                name: name.clone(),
                            });
                        }
                    }
                    let mut guard = condition.clone();
                    normalize_condition_aliases(&mut guard, &original.params);
                    body.condition = Some(guard);
                }
            }
            Some(Box::new(compiled))
        } else {
            None
        };
        let pack_id = manifest.id.clone();
        validate_pack_id(&pack_id)?;
        for spec in &manifest.http_queries {
            if spec.check_chronological_order
                && spec.native_map != Some(RequestQueryMapCoercion::MatomoPhp8)
            {
                return Err(ManifestError::InvalidConstraint {
                    pack_id: pack_id.clone(),
                    name: "http_queries.check_chronological_order".into(),
                    reason: "chronological order uses the explicit matomo_php8 cdt source profile"
                        .into(),
                });
            }
            if !spec.source_field.starts_with("body.")
                || !spec.source_field.ends_with("[]")
                || !json::is_valid_pattern(&spec.source_field)
                || spec
                    .inherited_params
                    .values()
                    .any(|path| !path.starts_with("body.") || !json::is_valid_pattern(path))
            {
                return Err(ManifestError::InvalidConstraint {
                    pack_id: pack_id.clone(),
                    name: "http_queries".into(),
                    reason:
                        "HTTP query sources need a body array path and valid body fallback paths"
                            .into(),
                });
            }
            for name in spec.inherited_params.keys() {
                if !manifest
                    .params
                    .iter()
                    .any(|param| &param.name == name || param.aliases.contains(name))
                {
                    return Err(ManifestError::UnknownParam {
                        pack_id: pack_id.clone(),
                        code: "http_queries.inherited_params".into(),
                        name: name.clone(),
                    });
                }
            }
        }

        for (field, value) in [
            ("display_name", &manifest.display_name),
            ("description", &manifest.description),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::EmptyField { pack_id, field });
            }
        }
        if manifest
            .matcher
            .query_params_any
            .iter()
            .chain(&manifest.matcher.query_params_none)
            .any(|name| name.is_empty())
            || manifest
                .matcher
                .query_params_any
                .iter()
                .any(|name| manifest.matcher.query_params_none.contains(name))
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id,
                name: "match.query_params".into(),
                reason: "query selectors need nonempty, nonconflicting key names".into(),
            });
        }

        if manifest.matcher.any_host && !manifest.matcher.path_contains.is_empty() {
            return Err(ManifestError::InvalidConstraint {
                pack_id,
                name: "match.any_host".to_string(),
                reason: "any_host cannot combine exact paths with path_contains alternatives"
                    .to_string(),
            });
        }
        if manifest.matcher.any_host
            && ((manifest.matcher.paths.is_empty() && manifest.matcher.path_prefixes.is_empty())
                || manifest
                    .matcher
                    .paths
                    .iter()
                    .chain(&manifest.matcher.path_prefixes)
                    .any(|path| path == "/" || !path.starts_with('/')))
        {
            return Err(ManifestError::InvalidConstraint {
                pack_id,
                name: "match.any_host".to_string(),
                reason: "any_host requires a specific non-root path or prefix".to_string(),
            });
        }
        if !manifest.matcher.any_host
            && manifest.matcher.hosts.is_empty()
            && manifest.matcher.host_suffixes.is_empty()
        {
            return Err(ManifestError::MatcherTooBroad(pack_id));
        }
        if manifest
            .client_fragment_params
            .iter()
            .any(|name| name.is_empty())
        {
            return Err(ManifestError::EmptyField {
                pack_id,
                field: "client_fragment_params",
            });
        }
        if !manifest.client_fragment_params.is_empty() && manifest.path_pattern.is_none() {
            return Err(ManifestError::InvalidConstraint {
                pack_id,
                name: "client_fragment_params".to_string(),
                reason: "browser fragment declarations require path_pattern".to_string(),
            });
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
        let (params, url_names) = compile_params(&pack_id, &manifest, &manifest.params, false)?;
        let rules = compile_rules(
            &pack_id,
            &code_prefix,
            &manifest,
            &manifest.rules,
            &url_names,
            &manifest.params,
        )?;

        let mut queries = Vec::new();
        for spec in &manifest.query_scopes {
            if let Some(condition) = &spec.condition {
                validate_condition(&pack_id, "query_scopes.condition", condition)?;
            }
            if spec.last_index.is_some_and(|last| last < spec.first_index) {
                return Err(ManifestError::InvalidConstraint {
                    pack_id,
                    name: "query_scopes".to_string(),
                    reason: "last_index must not precede first_index".to_string(),
                });
            }
            let expected_style = match spec.source {
                QuerySource::QuerySemicolon => ParamStyle::QuerySemicolon,
                QuerySource::PathItems => ParamStyle::ColonPath,
            };
            if manifest.param_style != expected_style {
                return Err(ManifestError::InvalidConstraint {
                    pack_id,
                    name: "query_scopes".to_string(),
                    reason: "query_scopes source must agree with the parameter style".to_string(),
                });
            }
            let (params, names) = compile_params(&pack_id, &manifest, &spec.params, false)?;
            let rules = compile_rules(
                &pack_id,
                &code_prefix,
                &manifest,
                &spec.rules,
                &names,
                &spec.params,
            )?;
            let exact_names = exact_param_names(&params);
            if !spec.global_params.is_empty() && spec.source != QuerySource::PathItems {
                return Err(ManifestError::InvalidConstraint {
                    pack_id,
                    name: "global_params".to_string(),
                    reason: "global_params requires path_items source".to_string(),
                });
            }
            let mut global_names = BTreeSet::new();
            for name in &spec.global_params {
                if !exact_names.contains(name) || !global_names.insert(name) {
                    return Err(ManifestError::InvalidConstraint {
                        pack_id,
                        name: name.clone(),
                        reason: "global_params requires distinct exact declared parameter names"
                            .to_string(),
                    });
                }
            }
            for name in spec.condition.iter().flat_map(RuleCondition::params) {
                if !names.contains(name) {
                    return Err(ManifestError::UnknownParam {
                        pack_id,
                        code: "query_scopes.condition".to_string(),
                        name: name.clone(),
                    });
                }
            }
            let mut unique_names = BTreeSet::new();
            for name in &spec.unique_by {
                if !exact_names.contains(name) || !unique_names.insert(name) {
                    return Err(ManifestError::InvalidConstraint {
                        pack_id,
                        name: name.clone(),
                        reason: "unique_by requires distinct exact parameter names".to_string(),
                    });
                }
            }
            let mut normalized_spec = spec.clone();
            if let Some(condition) = &mut normalized_spec.condition {
                normalize_condition_aliases(condition, &spec.params);
            }
            for name in &mut normalized_spec.unique_by {
                normalize_alias(name, &spec.params);
            }
            for name in &mut normalized_spec.global_params {
                normalize_alias(name, &spec.params);
            }
            queries.push(CompiledQuery {
                spec: normalized_spec,
                params,
                rules,
                exact_names,
            });
        }

        // Body contracts get their own name space, because an endpoint is free
        // to accept the same field in the query string and in the payload, and
        // the two are different contracts with different findings.
        let mut bodies = Vec::new();

        match &manifest.body {
            Some(specs) => {
                let has_plain_body = specs.specs().iter().any(|spec| spec.source_param.is_none());
                if has_plain_body && manifest.matcher.json_paths.is_empty() {
                    return Err(ManifestError::BodyWithoutShape(pack_id));
                }
                if !has_plain_body && !manifest.matcher.json_paths.is_empty() {
                    return Err(ManifestError::ShapeWithoutBody(pack_id));
                }

                for spec in specs.specs() {
                    if spec.encoding_severity.is_some()
                        && spec.source_param.is_none()
                        && spec.source_field.is_none()
                    {
                        return Err(ManifestError::Parse(
                            "body encoding_severity requires source_param or source_field".into(),
                        ));
                    }
                    if spec.rules.iter().any(|rule| {
                        matches!(
                            rule.assertion,
                            Assertion::RequireHttps | Assertion::MaxUrlLength { .. }
                        )
                    }) {
                        return Err(ManifestError::Parse("HTTPS and complete URL length assertions belong in URL rules, not body rules".into()));
                    }
                    if (spec.decoded_source_field.is_some() || spec.source_max_length.is_some())
                        && spec.source_field.is_none()
                        || spec.decoded_source_condition.is_some()
                            && spec.decoded_source_field.is_none()
                        || spec.source_max_length_when.is_some() && spec.source_max_length.is_none()
                    {
                        return Err(ManifestError::Parse("nested decoding and source length guards require their source fields and limits".to_string()));
                    }
                    if spec.field_encoding != ParamEncoding::Json
                        && (spec.source_param.is_none() || spec.source_field.is_none())
                    {
                        return Err(ManifestError::Parse(
                            "body field_encoding requires both source_param and source_field"
                                .to_string(),
                        ));
                    }
                    if let Some(path) = &spec.source_field
                        && (path.is_empty() || !json::is_valid_pattern(path))
                    {
                        return Err(ManifestError::InvalidJsonPath {
                            pack_id,
                            path: path.clone(),
                        });
                    }
                    if let Some(path) = &spec.decoded_source_field
                        && (path.is_empty() || !json::is_valid_pattern(path))
                    {
                        return Err(ManifestError::InvalidJsonPath {
                            pack_id,
                            path: path.clone(),
                        });
                    }
                    for shape in spec
                        .decoded_source_condition
                        .iter()
                        .chain(spec.source_max_length_when.iter())
                    {
                        for path in shape.paths() {
                            if !path.is_empty() && !json::is_valid_pattern(path) {
                                return Err(ManifestError::InvalidJsonPath {
                                    pack_id,
                                    path: path.to_string(),
                                });
                            }
                        }
                    }
                    if let Some(condition) = &spec.condition {
                        validate_condition(&pack_id, "body.condition", condition)?;
                        if spec.source_param.is_none() {
                            return Err(ManifestError::Parse(
                                "body condition requires source_param".to_string(),
                            ));
                        }
                        for name in condition.params() {
                            if !url_names.contains(name) {
                                return Err(ManifestError::UnknownParam {
                                    pack_id,
                                    code: "body.condition".to_string(),
                                    name: name.clone(),
                                });
                            }
                        }
                    }
                    if let Some(source_param) = &spec.source_param
                        && !url_names.contains(source_param)
                    {
                        return Err(ManifestError::UnknownParam {
                            pack_id,
                            code: "body.source_param".to_string(),
                            name: source_param.clone(),
                        });
                    }
                    let (params, names) = compile_params(&pack_id, &manifest, &spec.params, true)?;
                    if let Some(compiled) = params.iter().find(|param| {
                        param.name_regex.is_some() && param.contract.name_pattern_parent.is_none()
                    }) {
                        return Err(ManifestError::NamePatternOnBody {
                            pack_id,
                            name: compiled.contract.name.clone(),
                        });
                    }
                    let rules = compile_rules(
                        &pack_id,
                        &code_prefix,
                        &manifest,
                        &spec.rules,
                        &names,
                        &spec.params,
                    )?;

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
                        .chain(spec.scope_exclusions.iter().cloned())
                        .collect::<Vec<_>>();

                    for path in &paths {
                        if !path.is_empty() && !json::is_valid_pattern(path) {
                            return Err(ManifestError::InvalidJsonPath {
                                pack_id,
                                path: path.clone(),
                            });
                        }
                    }

                    let exact_names = exact_param_names(&params);
                    let mut source_param = spec.source_param.clone();
                    if let Some(name) = &mut source_param {
                        normalize_alias(name, &manifest.params);
                    }
                    let mut condition = spec.condition.clone();
                    if let Some(guard) = &mut condition {
                        normalize_condition_aliases(guard, &manifest.params);
                    }
                    bodies.push(CompiledBody {
                        code_segment: "body",
                        source_param,
                        source_field: spec.source_field.clone(),
                        decoded_source_field: spec.decoded_source_field.clone(),
                        decoded_source_condition: spec
                            .decoded_source_condition
                            .as_ref()
                            .map(|shape| compile_shape(&pack_id, shape))
                            .transpose()?,
                        source_max_length: spec.source_max_length,
                        source_max_length_when: spec
                            .source_max_length_when
                            .as_ref()
                            .map(|shape| compile_shape(&pack_id, shape))
                            .transpose()?,
                        condition,
                        encoding: spec.encoding,
                        encoding_severity: spec.encoding_severity.unwrap_or(Severity::Error),
                        field_encoding: spec.field_encoding,
                        scope: spec.scope.clone(),
                        scope_exclusions: spec.scope_exclusions.clone(),
                        params,
                        rules,
                        exact_names,
                    });
                }

                shapes = compile_shapes(&pack_id, &manifest.matcher.json_paths)?;
            }
            None => {
                if !manifest.matcher.json_paths.is_empty() {
                    return Err(ManifestError::ShapeWithoutBody(pack_id));
                }
            }
        }

        let mut matcher = manifest.matcher;
        for host in &mut matcher.hosts {
            host.make_ascii_lowercase();
        }
        for suffix in &mut matcher.host_suffixes {
            suffix.make_ascii_lowercase();
        }
        let exact_param_names = exact_param_names(&params);
        for value in &manifest.gdpr_non_applicable_values {
            if value.is_empty() || value == "1" || matcher.any_host ||
                !manifest.params.iter().any(|contract| contract.name == "gdpr" && matches!(&contract.format, Some(ValueFormat::Enum { values, .. }) if values.contains(value))) {
                return Err(ManifestError::InvalidConstraint { pack_id, name: "gdpr_non_applicable_values".to_string(), reason: "alternate GDPR values require a host-bound destination and a documented gdpr enum containing each non-applicable value".to_string() });
            }
        }
        for alias in &manifest.gdpr_consent_aliases {
            if alias.is_empty()
                || alias == "gdpr_consent"
                || matcher.any_host
                || (matcher.hosts.is_empty() && matcher.host_suffixes.is_empty())
                || !manifest.params.iter().any(|contract| {
                    contract.name == "gdpr_consent" && contract.aliases.contains(alias)
                })
            {
                return Err(ManifestError::InvalidConstraint {
                    pack_id,
                    name: "gdpr_consent_aliases".to_string(),
                    reason: "consent aliases require a host-bound destination and a declared alias of gdpr_consent".to_string(),
                });
            }
        }

        let mut http_url_presence_overrides = BTreeSet::new();
        for name in &manifest.http_url_presence_overrides {
            if !http_url_presence_overrides.insert(name.clone())
                || matcher.any_host
                || (matcher.hosts.is_empty() && matcher.host_suffixes.is_empty())
                || http
                    .as_ref()
                    .is_none_or(|transport| transport.bodies.is_empty())
                || !manifest.params.iter().any(|contract| {
                    contract.name == *name
                        && matches!(
                            contract.requirement,
                            Requirement::Required | Requirement::Recommended
                        )
                })
            {
                return Err(ManifestError::InvalidConstraint {
                    pack_id,
                    name: "http_url_presence_overrides".to_string(),
                    reason: "HTTP presence overrides require unique declared required or recommended URL fields, a host-bound endpoint, and complete HTTP contract scopes".to_string(),
                });
            }
        }

        Ok(Self {
            gdpr_non_applicable_values: manifest.gdpr_non_applicable_values,
            gdpr_consent_aliases: manifest.gdpr_consent_aliases,
            http_url_presence_overrides,
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
            client_fragment_params: manifest.client_fragment_params.into_iter().collect(),
            matcher,
            params,
            exact_param_names,
            rules,
            bodies,
            queries,
            shapes,
            http,
            http_queries: manifest.http_queries,
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
            ) || (kind == ArtifactKind::JsonPayload && !self.bodies.is_empty());
        }

        self.matcher.artifact_kinds.contains(&kind)
    }

    /// Whether this artifact should be read as a JSON body rather than a URL.
    /// An explicit `json` kind says so outright; an unstated kind is treated as
    /// a body when it opens like one and the pack has body contracts to apply.
    fn reads_as_body(&self, kind: ArtifactKind, artifact: &str) -> bool {
        if !self.bodies.iter().any(|body| body.source_param.is_none()) {
            return false;
        }

        kind == ArtifactKind::JsonPayload
            || (kind == ArtifactKind::Unknown && JsonDocument::looks_like_json(artifact))
    }

    /// Whether the payload has the shape this pack claims. A bare body carries
    /// no host, so its shape is the only thing that can identify it.
    fn matches_shape(&self, document: &JsonDocument<'_>) -> bool {
        !self.shapes.is_empty() && self.shapes.iter().all(|shape| shape.holds(document))
    }

    fn matches_parsed_url(&self, parsed: Option<&ArtifactUrl>, params: &[RawParam<'_>]) -> bool {
        if !self.matches_parsed_endpoint(parsed) {
            return false;
        }
        if !self.matcher.query_params_any.is_empty() || !self.matcher.query_params_none.is_empty() {
            let query_names: BTreeSet<_> = params
                .iter()
                .filter(|param| param.component == ViolationTargetComponent::QueryParam)
                .map(|param| param.name.as_ref())
                .collect();
            if !self.matcher.query_params_any.is_empty()
                && !self
                    .matcher
                    .query_params_any
                    .iter()
                    .any(|name| query_names.contains(name.as_str()))
                || self
                    .matcher
                    .query_params_none
                    .iter()
                    .any(|name| query_names.contains(name.as_str()))
            {
                return false;
            }
        }

        true
    }

    fn matches_parsed_endpoint(&self, parsed: Option<&ArtifactUrl>) -> bool {
        let Some(parsed) = parsed else {
            return false;
        };
        let Some(host) = parsed.host.as_deref() else {
            return false;
        };

        let host_matches = self.matcher.any_host
            || self.matcher.hosts.iter().any(|candidate| candidate == host)
            || self
                .matcher
                .host_suffixes
                .iter()
                .any(|suffix| prepare::host_matches_suffix(host, suffix));

        if !host_matches {
            return false;
        }

        let path = parsed.path.as_ref();
        let path_constrained = !self.matcher.paths.is_empty()
            || !self.matcher.path_prefixes.is_empty()
            || !self.matcher.path_contains.is_empty();

        if !path_constrained {
            return true;
        }

        self.matcher.paths.iter().any(|candidate| candidate == path)
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

fn matomo_explicit_timestamp(value: &str) -> Option<String> {
    static NUMERIC: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    if NUMERIC
        .get_or_init(|| Regex::new(r"^-?[0-9]+(?:\.[0-9]+)?$").expect("static Matomo cdt grammar"))
        .is_match(value)
    {
        return value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|_| value.to_string());
    }
    if value.len() != 19 || !value.contains(' ') {
        return None;
    }
    let millis = crate::timestamp::timestamp_millis(value, TimestampUnit::DatetimeUtc)?;
    Some((millis / 1000).to_string())
}

impl ValidatorPlugin for ManifestRulePack {
    fn supports_http(&self, prepared: &PreparedArtifact<'_>) -> bool {
        self.matches_parsed_url(prepared.url(), prepared.params(ParamStyle::Query))
    }

    fn validate_http(
        &self,
        prepared: &PreparedArtifact<'_>,
        body: Option<&PreparedArtifact<'_>>,
        raw_body_len: usize,
        normalized: &str,
        run_core_url_rules: bool,
    ) -> ValidationReport {
        let matches_endpoint = self.supports_http(prepared);
        let document = JsonDocument::parse_http(normalized).ok();
        let bulk = matches_endpoint
            && document.as_ref().is_some_and(|document| {
                self.http_queries.iter().any(|spec| {
                    document.contains(
                        spec.source_field
                            .strip_suffix("[]")
                            .unwrap_or(&spec.source_field),
                    )
                })
            });
        let mut report = if bulk {
            ValidationReport {
                plugin_id: self.metadata.id.clone(),
                detected_vendor: self.vendor.clone(),
                violations: Vec::new(),
            }
        } else {
            self.validate_prepared(prepared)
        };
        if !matches_endpoint {
            return report;
        }
        report.detected_vendor = self.vendor.clone();
        if bulk && let Some(document) = &document {
            let endpoint = prepared
                .trimmed()
                .split(['?', '#'])
                .next()
                .unwrap_or_default();
            for spec in &self.http_queries {
                let mut previous_timestamp: Option<String> = None;
                let source_parent = spec
                    .source_field
                    .strip_suffix("[]")
                    .unwrap_or(&spec.source_field);
                let paths = if spec.native_map == Some(RequestQueryMapCoercion::MatomoPhp8)
                    && document
                        .get(source_parent)
                        .is_some_and(|source| source.kind == JsonValueKind::Object)
                {
                    document
                        .members(source_parent)
                        .into_iter()
                        .map(|(_, path)| path)
                        .collect()
                } else {
                    document.expand(&spec.source_field)
                };
                for path in paths {
                    let Some(field) = document.get(&path) else {
                        continue;
                    };
                    let uncertain_outer_token = spec.native_map
                        == Some(RequestQueryMapCoercion::MatomoPhp8)
                        && spec
                            .inherited_params
                            .get("token_auth")
                            .and_then(|source| document.get(source))
                            .is_some_and(|token| {
                                let value: serde_json::Value =
                                    serde_json::from_str(&normalized[token.start..token.end])
                                        .expect("validated normalized JSON field");
                                crate::php_query::matomo_php8_auth_string(&value).is_err()
                            });
                    if uncertain_outer_token {
                        report.violations.push(Violation {
                            code: format!("{}.http.query_source.unvalidated", self.code_prefix),
                            message: "The native outer token needs PHP architecture or precision configuration. This item's authentication-dependent event checks remain unvalidated.".into(),
                            severity: Severity::Info,
                            field: Some(format!("http.{path}")),
                            fix_hint: None,
                            source: self.source_for(RuleSourceLevel::OfficialTemplate, Some(crate::php_query::MATOMO_PHP8_SOURCE)),
                            targets: Vec::new(),
                        });
                        continue;
                    }
                    let native_pairs = if spec.native_map
                        == Some(RequestQueryMapCoercion::MatomoPhp8)
                        && field.kind != JsonValueKind::String
                    {
                        let native: serde_json::Value =
                            serde_json::from_str(&normalized[field.start..field.end])
                                .expect("validated normalized JSON field");
                        match crate::php_query::matomo_php8_map(&native) {
                            crate::php_query::MatomoMap::Ignored => continue,
                            crate::php_query::MatomoMap::Ready(pairs) => Some(pairs),
                            crate::php_query::MatomoMap::Unsupported(names) => {
                                report.violations.push(Violation {
                                    code: format!("{}.http.query_source.unvalidated", self.code_prefix),
                                    message: format!("This native map needs PHP architecture, precision configuration, or an unmapped field reader for: {}. Its event checks remain unvalidated.", names.join(", ")),
                                    severity: Severity::Info,
                                    field: Some(format!("http.{path}")),
                                    fix_hint: None,
                                    source: self.source_for(RuleSourceLevel::OfficialTemplate, Some(crate::php_query::MATOMO_PHP8_SOURCE)),
                                    targets: Vec::new(),
                                });
                                continue;
                            }
                        }
                    } else {
                        if field.kind == JsonValueKind::Object {
                            report.violations.push(Violation {
                                code:format!("{}.http.query_source.unvalidated", self.code_prefix),
                                message:"Native parameter maps need destination-specific coercion contracts; this item's event fields were not validated.".into(),
                                severity:Severity::Info, field:Some(format!("http.{path}")), fix_hint:None,
                                source:self.source_for(RuleSourceLevel::OfficialVendor,self.docs.as_deref()), targets:Vec::new(),
                            });
                            continue;
                        }
                        if field.kind != JsonValueKind::String {
                            continue;
                        }
                        None
                    };
                    let query = match spec.encoding {
                        RequestQueryEncoding::Query => {
                            field.text.strip_prefix('?').unwrap_or(&field.text)
                        }
                        RequestQueryEncoding::UrlQuery => field
                            .text
                            .split('#')
                            .next()
                            .unwrap_or_default()
                            .split_once('?')
                            .map(|(_, query)| query)
                            .unwrap_or_default(),
                    };
                    if query.is_empty() && native_pairs.is_none() {
                        continue;
                    }
                    let native = native_pairs.is_some();
                    let mut pairs: Vec<(String, String)> = native_pairs.unwrap_or_else(|| {
                        url::form_urlencoded::parse(query.as_bytes())
                            .map(|(name, value)| (name.into_owned(), value.into_owned()))
                            .collect()
                    });
                    let matomo = spec.native_map == Some(RequestQueryMapCoercion::MatomoPhp8);
                    if matomo {
                        pairs.retain(|(name, value)| {
                            name != "token_auth" || (!value.is_empty() && value != "0")
                        });
                    }
                    let mut inherited = Vec::new();
                    for (name, fallback_path) in &spec.inherited_params {
                        let contract = self.params.iter().find(|param| param.names.contains(name));
                        let supplied = pairs.iter().any(|(submitted, _)| {
                            contract.is_some_and(|param| param.names.contains(submitted))
                        });
                        let fallback = document.get(fallback_path).and_then(|fallback| {
                            if matomo && name == "token_auth" {
                                let value: serde_json::Value =
                                    serde_json::from_str(&normalized[fallback.start..fallback.end])
                                        .expect("validated normalized JSON field");
                                crate::php_query::matomo_php8_auth_string(&value)
                                    .ok()
                                    .flatten()
                            } else {
                                (fallback.kind == JsonValueKind::String
                                    && !fallback.text.is_empty())
                                .then(|| fallback.text.to_string())
                            }
                        });
                        if (spec.inherited_overrides || !supplied)
                            && let Some(fallback) = fallback
                        {
                            if spec.inherited_overrides {
                                pairs.retain(|(submitted, _)| {
                                    !contract.is_some_and(|param| param.names.contains(submitted))
                                });
                            }
                            inherited.push((name.clone(), fallback));
                        }
                    }
                    let mut wire_query = if native {
                        url::form_urlencoded::Serializer::new(String::new())
                            .extend_pairs(pairs.iter().map(|(name, value)| (name, value)))
                            .finish()
                    } else if (spec.inherited_overrides && !inherited.is_empty()) || matomo {
                        // Only remove overridden keys. Preserve every other
                        // original field's encoding, order and macro spans.
                        query
                            .split('&')
                            .filter(|part| {
                                let name = url::form_urlencoded::parse(part.as_bytes())
                                    .next()
                                    .map(|(name, _)| name.into_owned());
                                let empty_matomo_token = matomo
                                    && url::form_urlencoded::parse(part.as_bytes())
                                        .next()
                                        .is_some_and(|(name, value)| {
                                            name == "token_auth"
                                                && (value.is_empty() || value == "0")
                                        });
                                !empty_matomo_token
                                    && !inherited.iter().any(|(inherited_name, _)| {
                                        self.params
                                            .iter()
                                            .find(|param| param.names.contains(inherited_name))
                                            .is_some_and(|param| {
                                                name.as_ref()
                                                    .is_some_and(|name| param.names.contains(name))
                                            })
                                    })
                            })
                            .collect::<Vec<_>>()
                            .join("&")
                    } else {
                        query.to_string()
                    };
                    if !inherited.is_empty() {
                        if !wire_query.is_empty() {
                            wire_query.push('&');
                        }
                        wire_query.push_str(
                            &url::form_urlencoded::Serializer::new(String::new())
                                .extend_pairs(inherited.iter().map(|(name, value)| (name, value)))
                                .finish(),
                        );
                    }
                    if spec.check_chronological_order {
                        let clocks: Vec<_> =
                            pairs.iter().filter(|(name, _)| name == "cdt").collect();
                        if clocks.len() == 1
                            && let Some(timestamp) = matomo_explicit_timestamp(&clocks[0].1)
                        {
                            if previous_timestamp.as_deref().is_some_and(|previous| {
                                compare_numeric_text(previous, &timestamp)
                                    == Some(std::cmp::Ordering::Greater)
                            }) {
                                report.violations.push(Violation {
                                    code: format!("{}.http.chronological_order", self.code_prefix),
                                    message: "Matomo recommends bulk events in chronological order, oldest first. This explicit cdt is earlier than the previous explicit event time.".into(),
                                    severity: Severity::Warning,
                                    field: Some(format!("http.{path}.cdt")),
                                    fix_hint: Some("Order events with explicit timestamps from oldest to newest.".into()),
                                    source: self.source_for(RuleSourceLevel::OfficialVendor, self.docs.as_deref()),
                                    targets: Vec::new(),
                                });
                            }
                            previous_timestamp = Some(timestamp);
                        }
                    }
                    let inner_request = ValidationRequest {
                        artifact_kind: ArtifactKind::Url,
                        artifact: format!("{endpoint}?{wire_query}"),
                        claimed_vendor: prepared.request().claimed_vendor.clone(),
                        expansion_state: prepared.request().expansion_state,
                    };
                    let mut inner = PreparedArtifact::from_request_at(
                        &inner_request,
                        prepared.reference_time_unix_seconds(),
                    );
                    self.prepare_vendor_context(&mut inner);
                    let mut violations = self.validate_prepared(&inner).violations;
                    if run_core_url_rules {
                        violations.extend(
                            crate::CoreRulePack::default()
                                .validate_prepared(&inner)
                                .violations,
                        );
                    }
                    for mut violation in violations {
                        violation.field = Some(match violation.field {
                            Some(field) => format!("http.{path}.{field}"),
                            None => format!("http.{path}"),
                        });
                        report.violations.push(violation);
                    }
                }
            }
        }
        if let Some(body) = body
            && let Some(Ok(document)) = body.json()
        {
            for spec in self
                .bodies
                .iter()
                .filter(|spec| spec.source_param.is_none() && spec.source_field.is_none())
            {
                self.check_body_spec(
                    spec,
                    document,
                    body.trimmed(),
                    raw_body_len,
                    body.reference_time_unix_seconds(),
                    &mut report.violations,
                );
            }
            self.check_embedded_bodies(
                document,
                None,
                &[],
                body.reference_time_unix_seconds(),
                &mut report.violations,
            );
        }
        if let Some(http) = &self.http
            && let Some(document) = &document
        {
            for spec in &http.bodies {
                http.check_body_spec(
                    spec,
                    document,
                    normalized,
                    normalized.len(),
                    prepared.reference_time_unix_seconds(),
                    &mut report.violations,
                );
            }
        }
        report
    }
    fn prepare_vendor_context(&self, prepared: &mut PreparedArtifact<'_>) {
        if self.accepts_client_fragment(prepared) {
            prepared.mark_client_fragment_configuration();
        }
        if self.matches_parsed_url(prepared.url(), prepared.params(ParamStyle::Query)) {
            for value in &self.gdpr_non_applicable_values {
                prepared.allow_non_applicable_gdpr(value);
            }
            for alias in &self.gdpr_consent_aliases {
                prepared.allow_gdpr_consent_alias(alias);
            }
        }
    }
    fn metadata(&self) -> &RulePackMetadata {
        &self.metadata
    }

    fn routing(&self) -> PluginRouting {
        if self.matcher.any_host {
            return PluginRouting::Always;
        }
        PluginRouting::Indexed {
            hosts: self.matcher.hosts.clone(),
            suffixes: self.matcher.host_suffixes.clone(),
            json: self.bodies.iter().any(|body| body.source_param.is_none()),
        }
    }

    fn supports(&self, request: &ValidationRequest) -> bool {
        self.supports_prepared(&PreparedArtifact::from_request(request))
    }

    fn supports_prepared(&self, prepared: &PreparedArtifact<'_>) -> bool {
        let artifact = prepared.trimmed();

        if !self.matches_artifact_kind(prepared.request().artifact_kind) {
            return false;
        }

        if self.reads_as_body(prepared.request().artifact_kind, artifact) {
            return match prepared.json() {
                Some(Ok(document)) => self.matches_shape(document),
                _ => false,
            };
        }

        self.matches_parsed_url(prepared.url(), prepared.params(ParamStyle::Query))
    }

    fn validate(&self, request: &ValidationRequest) -> ValidationReport {
        self.validate_prepared(&PreparedArtifact::from_request(request))
    }

    fn validate_prepared(&self, prepared: &PreparedArtifact<'_>) -> ValidationReport {
        let request = prepared.request();
        let artifact = prepared.trimmed();
        let mut violations = Vec::new();

        if self.reads_as_body(request.artifact_kind, artifact) {
            return self.validate_body(
                request,
                artifact,
                prepared.json(),
                false,
                prepared.reference_time_unix_seconds(),
            );
        }

        let matches_endpoint =
            self.matches_parsed_url(prepared.url(), prepared.params(ParamStyle::Query));
        if !matches_endpoint {
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

            if !self.matches_parsed_endpoint(prepared.url()) {
                return ValidationReport {
                    plugin_id: self.metadata.id.clone(),
                    detected_vendor: None,
                    violations,
                };
            }
        }

        let extracted = prepared.params(self.param_style);
        let basket_ranges: Vec<_> = if self
            .queries
            .iter()
            .any(|query| query.spec.source == QuerySource::PathItems)
        {
            crate::path_items::path_items(artifact, 0)
                .unwrap_or_default()
                .into_iter()
                .map(|group| (group.start, group.end))
                .collect()
        } else {
            Vec::new()
        };
        let path_params = self.extract_path_params(artifact);
        let owned;
        let params: &[RawParam] = if !prepared.has_form_body()
            && self.path_pattern.is_none()
            && path_params.is_empty()
            && basket_ranges.is_empty()
            && !extracted.iter().any(|param| {
                self.params.iter().any(|contract| {
                    contract
                        .contract
                        .aliases
                        .iter()
                        .any(|alias| alias == param.name.as_ref())
                })
            }) {
            extracted
        } else {
            owned = {
                // A query key cannot override an identifier or discriminator
                // physically carried in the endpoint path.
                let mut merged: Vec<_> = extracted
                    .iter()
                    .filter(|param| {
                        let supplies_path_capture =
                            self.path_pattern.as_ref().is_some_and(|pattern| {
                                pattern.capture_names().flatten().any(|capture| {
                                    param.name.as_ref() == capture
                                        || self.params.iter().any(|contract| {
                                            contract.names.iter().any(|name| name == capture)
                                                && contract
                                                    .names
                                                    .iter()
                                                    .any(|name| name == param.name.as_ref())
                                        })
                                })
                            });
                        if supplies_path_capture {
                            return false;
                        }
                        param.component != ViolationTargetComponent::BodyField
                            || (!self.path_pattern.as_ref().is_some_and(|pattern| {
                                pattern
                                    .capture_names()
                                    .flatten()
                                    .any(|name| name == param.name)
                            }) && !self.params.iter().any(|contract| {
                                contract
                                    .names
                                    .iter()
                                    .any(|name| name == param.name.as_ref())
                                    && extracted.iter().any(|url_field| {
                                        url_field.component == ViolationTargetComponent::QueryParam
                                            && contract
                                                .names
                                                .iter()
                                                .any(|name| name == url_field.name.as_ref())
                                    })
                            }))
                    })
                    .filter(|param| !path_params.iter().any(|path| path.name == param.name))
                    .filter(|param| {
                        !basket_ranges
                            .iter()
                            .any(|(start, end)| param.start >= *start && param.end <= *end)
                    })
                    .cloned()
                    .collect();
                normalize_param_aliases(&mut merged, &self.params);
                merged.retain(|param| !path_params.iter().any(|path| path.name == param.name));
                merged.extend(path_params);
                merged
            };
            &owned
        };

        let scope = Scope {
            code_segment: "param",
            artifact,
            raw_artifact_len: request.artifact.len(),
            reference_time_unix_seconds: prepared.reference_time_unix_seconds(),
            body: None,
            query: None,
        };

        let exact_names = &self.exact_param_names;
        for compiled in &self.params {
            if matches_endpoint
                && prepared.is_complete_http_request()
                && self
                    .http_url_presence_overrides
                    .contains(&compiled.contract.name)
                && matches!(
                    compiled.contract.requirement,
                    Requirement::Required | Requirement::Recommended
                )
                && !params.iter().any(|field| {
                    !field.missing
                        && compiled
                            .names
                            .iter()
                            .any(|name| name == field.name.as_ref())
                })
            {
                continue;
            }
            // A multipart/compressed form may contain these query-style keys.
            // Path captures remain observable and cannot be supplied by a form.
            if prepared.has_unavailable_form_body()
                && !self.path_pattern.as_ref().is_some_and(|pattern| {
                    pattern
                        .capture_names()
                        .flatten()
                        .any(|name| compiled.names.iter().any(|candidate| candidate == name))
                })
                && !params.iter().any(|field| {
                    !field.missing
                        && compiled
                            .names
                            .iter()
                            .any(|name| name == field.name.as_ref())
                })
            {
                continue;
            }
            self.check_param(&scope, compiled, params, exact_names, &mut violations);
        }

        for compiled in &self.rules {
            if prepared.has_unavailable_form_body()
                && assertion_requires_presence(&compiled.rule.assertion)
            {
                continue;
            }
            self.check_rule(&scope, compiled, params, &mut violations);
        }
        self.check_query_scopes(prepared, &mut violations);
        self.check_encoded_bodies(
            params,
            prepared.reference_time_unix_seconds(),
            &mut violations,
        );

        if matches_endpoint
            && let (Some(claimed), Some(vendor)) =
                (request.claimed_vendor.as_deref(), self.vendor())
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
            detected_vendor: matches_endpoint.then(|| self.vendor.clone()).flatten(),
            violations,
        }
    }
    fn validate_selected_prepared(&self, prepared: &PreparedArtifact<'_>) -> ValidationReport {
        let artifact = prepared.trimmed();
        if self.reads_as_body(prepared.request().artifact_kind, artifact) {
            self.validate_body(
                prepared.request(),
                artifact,
                prepared.json(),
                true,
                prepared.reference_time_unix_seconds(),
            )
        } else {
            self.validate_prepared(prepared)
        }
    }

    fn accepts_client_fragment(&self, prepared: &PreparedArtifact<'_>) -> bool {
        if self.client_fragment_params.is_empty()
            || !matches!(
                prepared.request().artifact_kind,
                ArtifactKind::Url | ArtifactKind::Unknown
            )
            || !self.matches_parsed_url(prepared.url(), prepared.params(ParamStyle::Query))
        {
            return false;
        }
        let Some(parsed) = prepared.url() else {
            return false;
        };
        if !self
            .path_pattern
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(&parsed.path))
        {
            return false;
        }
        let Some((_, fragment)) = prepared.trimmed().split_once('#') else {
            return false;
        };
        let mut keys = url::form_urlencoded::parse(fragment.as_bytes()).map(|(key, _)| key);
        let Some(first) = keys.next() else {
            return false;
        };
        self.client_fragment_params.contains(first.as_ref())
            && keys.all(|key| self.client_fragment_params.contains(key.as_ref()))
    }
}

impl ManifestRulePack {
    fn check_query_scopes(&self, prepared: &PreparedArtifact<'_>, violations: &mut Vec<Violation>) {
        for query in &self.queries {
            let mut seen: BTreeMap<(&str, &str), ViolationTarget> = BTreeMap::new();
            let code_segment = match query.spec.source {
                QuerySource::QuerySemicolon => "query",
                QuerySource::PathItems => "path_items",
            };
            let mut segments = match query.spec.source {
                QuerySource::QuerySemicolon => crate::query_segments::query_segments(
                    prepared.trimmed(),
                    query.spec.first_index,
                ),
                QuerySource::PathItems => {
                    match crate::path_items::path_items(prepared.trimmed(), query.spec.first_index)
                    {
                        Ok(groups) => groups,
                        Err((start, end)) => {
                            violations.push(Violation {
                                code: format!("{}.path_items.malformed", self.code_prefix),
                                message: "Bracketed path items must close at a path boundary."
                                    .to_string(),
                                severity: Severity::Error,
                                field: Some("path_items".to_string()),
                                fix_hint: Some(
                                    "Close each basket with ] before the next path segment."
                                        .to_string(),
                                ),
                                source: self
                                    .source_for(self.metadata.source_level, self.docs.as_deref()),
                                targets: vec![ViolationTarget {
                                    component: ViolationTargetComponent::Path,
                                    name: Some("path_items".to_string()),
                                    value: None,
                                    start,
                                    end,
                                }],
                            });
                            continue;
                        }
                    }
                }
            };
            let global_params = if query.spec.global_params.is_empty() {
                Vec::new()
            } else {
                let mut params = prepared.params(self.param_style).to_vec();
                normalize_param_aliases(&mut params, &query.params);
                let all_items =
                    crate::path_items::path_items(prepared.trimmed(), 0).unwrap_or_default();
                params.retain(|param| {
                    query
                        .spec
                        .global_params
                        .iter()
                        .any(|name| name == param.name.as_ref())
                        && !all_items
                            .iter()
                            .any(|segment| param.start >= segment.start && param.end <= segment.end)
                });
                params
            };
            for segment in &mut segments {
                normalize_param_aliases(&mut segment.params, &query.params);
                segment.params.retain(|param| {
                    !query
                        .spec
                        .global_params
                        .iter()
                        .any(|name| name == param.name.as_ref())
                });
                segment.params.extend(global_params.iter().cloned());
                if query
                    .spec
                    .last_index
                    .is_some_and(|last| segment.index > last)
                {
                    break;
                }
                if query
                    .spec
                    .condition
                    .as_ref()
                    .is_some_and(|guard| guard.evaluate(&segment.params) != Some(true))
                {
                    continue;
                }
                let scope = Scope {
                    code_segment,
                    artifact: prepared.trimmed(),
                    raw_artifact_len: prepared.request().artifact.len(),
                    reference_time_unix_seconds: prepared.reference_time_unix_seconds(),
                    body: None,
                    query: Some(QuerySpan {
                        index: segment.index,
                        start: segment.start,
                        end: segment.end,
                    }),
                };
                for compiled in &query.params {
                    self.check_param(
                        &scope,
                        compiled,
                        &segment.params,
                        &query.exact_names,
                        violations,
                    );
                }
                for compiled in &query.rules {
                    self.check_rule(&scope, compiled, &segment.params, violations);
                }
                for name in &query.spec.unique_by {
                    let compiled = query
                        .params
                        .iter()
                        .find(|param| param.names.contains(name))
                        .expect("validated exact parameter");
                    for param in segment.params.iter().filter(|param| {
                        param.name.as_ref() == name
                            && !param.value.is_empty()
                            && !contains_macro(param.value.as_ref())
                    }) {
                        let key = (name.as_str(), param.value.as_ref());
                        if let Some(previous) = seen.get(&key) {
                            violations.push(Violation {
                                code: format!("{}.{code_segment}.{name}.duplicate", self.code_prefix),
                                message: format!("`{name}` must have a distinct value in each selected query group."),
                                severity: compiled.contract.severity.unwrap_or(Severity::Error),
                                field: Some(scope.field(name)),
                                fix_hint: compiled.contract.fix_hint.clone(),
                                source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                                targets: vec![previous.clone(), param.target()],
                            });
                        } else {
                            seen.insert(key, param.target());
                        }
                    }
                }
            }
        }
    }

    /// Validates a JSON request body.
    ///
    /// The contracts are written against one element of the batch, so the body
    /// is evaluated once per element: an array of three events that all omit
    /// `event_name` reports three findings, each pointing at its own event. A
    /// payload the pack does not recognize is reported the same way a URL that
    /// misses the endpoint is, since both mean the caller aimed the pack at the
    /// wrong artifact.
    fn validate_body(
        &self,
        request: &ValidationRequest,
        artifact: &str,
        parsed: Option<&Result<JsonDocument<'_>, json::JsonError>>,
        selected: bool,
        reference_time_unix_seconds: i64,
    ) -> ValidationReport {
        // A body that does not parse is the core pack's finding to report, and
        // it has nothing this pack can contract.
        let owned;
        let document = match parsed {
            Some(Ok(document)) => document,
            Some(Err(_)) => {
                return ValidationReport {
                    plugin_id: self.metadata.id.clone(),
                    detected_vendor: None,
                    violations: Vec::new(),
                };
            }
            None => match JsonDocument::parse(artifact) {
                Ok(document) => {
                    owned = document;
                    &owned
                }
                Err(_) => {
                    return ValidationReport {
                        plugin_id: self.metadata.id.clone(),
                        detected_vendor: None,
                        violations: Vec::new(),
                    };
                }
            },
        };

        let mut violations = Vec::new();

        let matches_shape = self.matches_shape(document);
        if !matches_shape {
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

            if !selected {
                return ValidationReport {
                    plugin_id: self.metadata.id.clone(),
                    detected_vendor: None,
                    violations,
                };
            }
        }

        for body in self
            .bodies
            .iter()
            .filter(|body| body.source_param.is_none() && body.source_field.is_none())
        {
            self.check_body_spec(
                body,
                document,
                artifact,
                request.artifact.len(),
                reference_time_unix_seconds,
                &mut violations,
            );
        }
        self.check_embedded_bodies(
            document,
            None,
            &[],
            reference_time_unix_seconds,
            &mut violations,
        );

        if matches_shape
            && let (Some(claimed), Some(vendor)) =
                (request.claimed_vendor.as_deref(), self.vendor())
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
            detected_vendor: matches_shape.then(|| self.vendor.clone()).flatten(),
            violations,
        }
    }

    /// Pulls named captures out of the path so an identifier carried there can
    /// be contracted like any query parameter.
    fn extract_path_params<'a>(&self, artifact: &'a str) -> Vec<RawParam<'a>> {
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
                    Cow::Owned(name.to_string()),
                    decode_param_value(capture.as_str()),
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

    fn check_body_spec(
        &self,
        body: &CompiledBody,
        document: &JsonDocument<'_>,
        artifact: &str,
        raw_artifact_len: usize,
        reference_time_unix_seconds: i64,
        violations: &mut Vec<Violation>,
    ) {
        for scope_path in body_scopes(document, body.scope.as_ref()) {
            let unavailable_body = body.code_segment == "http"
                && document
                    .get("body_encoding")
                    .is_some_and(|field| field.text == "unsupported");
            if unavailable_body
                && (scope_path == "body"
                    || scope_path.starts_with("body.")
                    || scope_path.starts_with("body["))
            {
                continue;
            }
            let unavailable_content_type = body.code_segment == "http"
                && document
                    .get("content_type_unavailable")
                    .is_some_and(|field| field.text == "true");
            let param_unavailable = |param: &CompiledParam| {
                (unavailable_body && http_param_reads_body(param))
                    || (unavailable_content_type
                        && (param.contract.root_path.as_deref() == Some("content_type")
                            || param.names.iter().any(|name| name == "content_type")))
            };
            let condition_reads_unavailable_body = |condition: &RuleCondition| {
                (unavailable_body || unavailable_content_type)
                    && condition.params().iter().any(|name| {
                        body.params
                            .iter()
                            .any(|param| param.names.contains(name) && param_unavailable(param))
                    })
            };
            if body
                .condition
                .as_ref()
                .is_some_and(condition_reads_unavailable_body)
            {
                continue;
            }
            if document.get(&scope_path).is_some_and(|selected| {
                body.scope_exclusions
                    .iter()
                    .flat_map(|pattern| document.expand(pattern))
                    .filter_map(|path| document.get(&path))
                    .any(|excluded| {
                        selected.start >= excluded.start && selected.end <= excluded.end
                    })
            }) {
                continue;
            }
            let params = collect_body_params(document, &scope_path, &body.params);
            if body.code_segment == "http"
                && body
                    .condition
                    .as_ref()
                    .is_some_and(|guard| guard.evaluate(&params) != Some(true))
            {
                continue;
            }
            let scope = Scope {
                code_segment: body.code_segment,
                artifact,
                raw_artifact_len,
                reference_time_unix_seconds,
                body: Some(BodyScope {
                    document,
                    path: &scope_path,
                }),
                query: None,
            };
            for compiled in &body.params {
                if (unavailable_body || unavailable_content_type)
                    && (param_unavailable(compiled)
                        || compiled
                            .contract
                            .condition
                            .as_ref()
                            .is_some_and(condition_reads_unavailable_body))
                {
                    continue;
                }
                self.check_param(&scope, compiled, &params, &body.exact_names, violations);
            }
            for compiled in &body.rules {
                if (unavailable_body || unavailable_content_type)
                    && (assertion_params(&compiled.rule.assertion)
                        .iter()
                        .any(|name| {
                            body.params
                                .iter()
                                .any(|param| param.names.contains(name) && param_unavailable(param))
                        })
                        || compiled
                            .rule
                            .condition
                            .as_ref()
                            .is_some_and(condition_reads_unavailable_body))
                {
                    continue;
                }
                self.check_rule(&scope, compiled, &params, violations);
            }
        }
    }

    fn check_encoded_bodies(
        &self,
        params: &[RawParam<'_>],
        reference_time_unix_seconds: i64,
        violations: &mut Vec<Violation>,
    ) {
        let sources: BTreeSet<_> = self
            .bodies
            .iter()
            .filter_map(|body| body.source_param.as_ref().map(|name| (name, body.encoding)))
            .collect();
        for (source, encoding) in sources {
            let active: Vec<_> = self
                .bodies
                .iter()
                .filter(|body| {
                    body.source_param.as_ref() == Some(source)
                        && body.encoding == encoding
                        && body
                            .condition
                            .as_ref()
                            .is_none_or(|guard| guard.evaluate(params) == Some(true))
                })
                .collect();
            if active.is_empty() {
                continue;
            }
            let encoding_severity = strongest_encoding_severity(active.iter().copied());
            for param in params.iter().filter(|param| {
                param.name.as_ref() == source && !param.missing && !param.value.is_empty()
            }) {
                let decoded = encoding.decode(param.value.as_ref());
                let decoded = match decoded {
                    Ok(decoded) => decoded,
                    Err(reason) => {
                        if encoding == ParamEncoding::PercentEncodedJson
                            || !contains_macro(param.value.as_ref())
                        {
                            self.encoded_body_error(
                                source,
                                param,
                                &reason,
                                encoding_severity,
                                violations,
                            );
                        }
                        continue;
                    }
                };
                let document = match JsonDocument::parse(&decoded) {
                    Ok(document) => document,
                    Err(error) => {
                        if !contains_macro(param.value.as_ref())
                            && !(encoding == ParamEncoding::PercentEncodedJson
                                && contains_macro(&decoded))
                        {
                            self.encoded_body_error(
                                source,
                                param,
                                &error.to_string(),
                                encoding_severity,
                                violations,
                            );
                        }
                        continue;
                    }
                };
                let mut decoded_violations = Vec::new();
                for body in self.bodies.iter().filter(|body| {
                    body.source_param.as_ref() == Some(source)
                        && body.source_field.is_none()
                        && body.encoding == encoding
                        && body
                            .condition
                            .as_ref()
                            .is_none_or(|guard| guard.evaluate(params) == Some(true))
                }) {
                    self.check_body_spec(
                        body,
                        &document,
                        &decoded,
                        encoding.decoded_byte_length(param.value.as_ref(), &decoded),
                        reference_time_unix_seconds,
                        &mut decoded_violations,
                    );
                }
                self.check_embedded_bodies(
                    &document,
                    Some((source, encoding)),
                    params,
                    reference_time_unix_seconds,
                    &mut decoded_violations,
                );
                // Decoding changes byte offsets. Point at the original parameter
                // rather than presenting offsets in decoded text as URL offsets.
                for mut violation in decoded_violations {
                    if let Some(field) = &mut violation.field {
                        *field = format!(
                            "param.{source}.{}",
                            field.strip_prefix("body.").unwrap_or(field)
                        );
                    }
                    violation.targets = vec![param.target()];
                    violations.push(violation);
                }
            }
        }
    }

    fn check_embedded_bodies(
        &self,
        outer: &JsonDocument<'_>,
        parent_source: Option<(&str, ParamEncoding)>,
        params: &[RawParam<'_>],
        reference_time_unix_seconds: i64,
        violations: &mut Vec<Violation>,
    ) {
        let sources: BTreeSet<_> = self
            .bodies
            .iter()
            .filter(|body| match parent_source {
                None => body.source_param.is_none(),
                Some((name, encoding)) => {
                    body.source_param.as_deref() == Some(name) && body.encoding == encoding
                }
            })
            .filter(|body| {
                body.condition
                    .as_ref()
                    .is_none_or(|guard| guard.evaluate(params) == Some(true))
            })
            .filter_map(|body| {
                body.source_field.as_ref().map(|name| {
                    (
                        name,
                        if parent_source.is_some() {
                            body.field_encoding
                        } else {
                            body.encoding
                        },
                    )
                })
            })
            .collect();
        for (source, encoding) in sources {
            let active: Vec<_> = self
                .bodies
                .iter()
                .filter(|body| {
                    body.source_field.as_ref() == Some(source)
                        && match parent_source {
                            None => body.source_param.is_none() && body.encoding == encoding,
                            Some((name, outer_encoding)) => {
                                body.source_param.as_deref() == Some(name)
                                    && body.encoding == outer_encoding
                                    && body.field_encoding == encoding
                            }
                        }
                        && body
                            .condition
                            .as_ref()
                            .is_none_or(|guard| guard.evaluate(params) == Some(true))
                })
                .collect();
            let encoding_severity = strongest_encoding_severity(active.iter().copied());
            for path in outer.expand(source) {
                let Some(field) = outer.get(&path) else {
                    continue;
                };
                // The outer field contract owns presence and native type checks.
                if field.kind != JsonValueKind::String || field.text.is_empty() {
                    continue;
                }
                let param = RawParam::query(
                    Cow::Borrowed(path.as_str()),
                    field.text.clone(),
                    field.start,
                    field.end,
                );
                let decoded = encoding.decode(field.text.as_ref());
                let decoded = match decoded {
                    Ok(decoded) => decoded,
                    Err(reason) => {
                        if encoding == ParamEncoding::PercentEncodedJson
                            || !contains_macro(field.text.as_ref())
                        {
                            self.embedded_body_error(
                                source,
                                &path,
                                &param,
                                &reason,
                                encoding_severity,
                                violations,
                            );
                        }
                        continue;
                    }
                };
                let document = match JsonDocument::parse(&decoded) {
                    Ok(document) => document,
                    Err(error) => {
                        if !contains_macro(field.text.as_ref())
                            && !(encoding == ParamEncoding::PercentEncodedJson
                                && contains_macro(&decoded))
                        {
                            self.embedded_body_error(
                                source,
                                &path,
                                &param,
                                &error.to_string(),
                                encoding_severity,
                                violations,
                            );
                        }
                        continue;
                    }
                };
                let mut decoded_violations = Vec::new();
                let mut nested_errors = BTreeSet::new();
                for body in self.bodies.iter().filter(|body| {
                    body.source_field.as_ref() == Some(source)
                        && match parent_source {
                            None => body.source_param.is_none() && body.encoding == encoding,
                            Some((name, outer_encoding)) => {
                                body.source_param.as_deref() == Some(name)
                                    && body.encoding == outer_encoding
                                    && body.field_encoding == encoding
                            }
                        }
                        && body
                            .condition
                            .as_ref()
                            .is_none_or(|guard| guard.evaluate(params) == Some(true))
                }) {
                    if let Some(maximum) = body.source_max_length
                        && body
                            .source_max_length_when
                            .as_ref()
                            .is_none_or(|guard| guard.holds(&document))
                        && field.text.chars().count() > maximum
                        && !contains_macro(field.text.as_ref())
                    {
                        violations.push(Violation {
                            code: format!("{}.body.{source}.source_length", self.code_prefix),
                            message: format!("`{source}` exceeds {maximum} Unicode characters for the selected decoded representation."),
                            severity: Severity::Error, field: Some(format!("body.{path}")),
                            fix_hint: Some("Shorten the original JSON string for this representation.".into()),
                            source: self.source_for(self.metadata.source_level, self.docs.as_deref()),
                            targets: vec![ViolationTarget { component: ViolationTargetComponent::BodyField, name: Some(path.clone()), value: Some(field.text.to_string()), start: field.start, end: field.end }],
                        });
                    }
                    if let Some(nested_source) = &body.decoded_source_field {
                        if body
                            .decoded_source_condition
                            .as_ref()
                            .is_some_and(|guard| !guard.holds(&document))
                        {
                            continue;
                        }
                        for nested_path in document.expand(nested_source) {
                            let Some(nested) = document.get(&nested_path) else {
                                continue;
                            };
                            if nested.kind != JsonValueKind::String
                                || nested.text.is_empty()
                                || contains_macro(nested.text.as_ref())
                            {
                                continue;
                            }
                            match JsonDocument::parse(nested.text.as_ref()) {
                                Ok(nested_doc) => {
                                    let mut findings = Vec::new();
                                    self.check_body_spec(
                                        body,
                                        &nested_doc,
                                        nested.text.as_ref(),
                                        nested.text.len(),
                                        reference_time_unix_seconds,
                                        &mut findings,
                                    );
                                    for mut finding in findings {
                                        if let Some(field) = &mut finding.field {
                                            let suffix =
                                                field.strip_prefix("body.").unwrap_or(field);
                                            *field = format!(
                                                "body.{nested_path}{}{suffix}",
                                                if suffix.starts_with('[') || suffix.is_empty() {
                                                    ""
                                                } else {
                                                    "."
                                                }
                                            );
                                        }
                                        decoded_violations.push(finding);
                                    }
                                }
                                Err(error) => {
                                    if nested_errors.insert(nested_path.clone()) {
                                        let severity = strongest_encoding_severity(
                                            active.iter().copied().filter(|candidate| {
                                                candidate.decoded_source_field.as_ref().is_some_and(
                                                    |pattern| {
                                                        document
                                                            .expand(pattern)
                                                            .contains(&nested_path)
                                                    },
                                                ) && candidate
                                                    .decoded_source_condition
                                                    .as_ref()
                                                    .is_none_or(|guard| guard.holds(&document))
                                            }),
                                        );
                                        self.embedded_body_error(
                                            nested_source,
                                            &nested_path,
                                            &param,
                                            &error.to_string(),
                                            severity,
                                            &mut decoded_violations,
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        self.check_body_spec(
                            body,
                            &document,
                            &decoded,
                            encoding.decoded_byte_length(field.text.as_ref(), &decoded),
                            reference_time_unix_seconds,
                            &mut decoded_violations,
                        );
                    }
                }
                for mut violation in decoded_violations {
                    if let Some(field) = &mut violation.field {
                        *field = format!(
                            "body.{path}.{}",
                            field.strip_prefix("body.").unwrap_or(field)
                        );
                    }
                    violation.targets = vec![ViolationTarget {
                        component: ViolationTargetComponent::BodyField,
                        start: param.start,
                        end: param.end,
                        name: Some(path.clone()),
                        value: Some(field.text.to_string()),
                    }];
                    violations.push(violation);
                }
            }
        }
    }

    fn embedded_body_error(
        &self,
        source: &str,
        path: &str,
        param: &RawParam<'_>,
        reason: &str,
        severity: Severity,
        violations: &mut Vec<Violation>,
    ) {
        self.encoded_body_error(source, param, reason, severity, violations);
        let violation = violations
            .last_mut()
            .expect("encoded_body_error adds a finding");
        violation.field = Some(format!("body.{path}"));
        violation.targets[0].component = ViolationTargetComponent::BodyField;
        violation.targets[0].name = Some(path.to_string());
    }

    fn encoded_body_error(
        &self,
        source: &str,
        param: &RawParam<'_>,
        reason: &str,
        severity: Severity,
        violations: &mut Vec<Violation>,
    ) {
        violations.push(Violation {
            code: format!("{}.body.{source}.invalid", self.code_prefix),
            message: format!("`{source}` must encode valid JSON: {reason}."),
            severity,
            field: Some(format!("param.{source}")),
            fix_hint: Some("Encode a JSON value in the documented parameter format.".to_string()),
            source: self.source_for(self.metadata.source_level, self.docs.as_deref()),
            targets: vec![param.target()],
        });
    }

    fn check_param(
        &self,
        scope: &Scope,
        compiled: &CompiledParam,
        params: &[RawParam<'_>],
        exact_names: &BTreeSet<String>,
        violations: &mut Vec<Violation>,
    ) {
        let contract = &compiled.contract;
        if contract
            .condition
            .as_ref()
            .is_some_and(|guard| guard.evaluate(params) != Some(true))
        {
            return;
        }
        let addressed: Vec<&RawParam> = match &compiled.name_regex {
            Some(name_regex) => params
                .iter()
                .filter(|param| {
                    if exact_names.contains(param.name.as_ref()) {
                        return false;
                    }
                    let name = match (&contract.name_pattern_parent, &scope.body) {
                        (Some(parent), Some(_)) => match (&param.family_parent, &param.member_name)
                        {
                            (Some(actual), Some(name)) if actual == parent => name.as_str(),
                            _ => return false,
                        },
                        _ => param.name.as_ref(),
                    };
                    name_regex.is_match(name)
                })
                .collect(),
            None => params
                .iter()
                .filter(|param| {
                    compiled
                        .names
                        .iter()
                        .any(|name| name.as_str() == param.name.as_ref())
                })
                .collect(),
        };
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
                        if targets.is_empty() {
                            return;
                        }
                        let field = scope.field(&contract.name);
                        let source =
                            self.source_for(compiled.source_level, compiled.doc.as_deref());
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
                    let missing: Vec<_> = addressed
                        .iter()
                        .filter(|slot| slot.missing)
                        .map(|slot| slot.target())
                        .collect();
                    // Exact aliases identify one logical field in this scope.
                    // A present alternate spelling satisfies its presence.
                    let exact_aliases = !contract.aliases.is_empty()
                        && compiled
                            .names
                            .iter()
                            .all(|name| !name.contains("[]") && !name.contains('*'));
                    if !exact_aliases || addressed.iter().all(|slot| slot.missing) {
                        report_missing(
                            if exact_aliases {
                                missing.into_iter().take(1).collect()
                            } else {
                                missing
                            },
                            violations,
                        );
                    }
                } else if addressed.iter().all(|param| param.missing) {
                    report_missing(vec![scope.fallback_for(&contract.name)], violations);
                    return;
                }
            }
            Requirement::Forbidden => {
                for param in addressed.iter().filter(|param| !param.missing) {
                    let code_name = finding_name(compiled, param);
                    violations.push(Violation {
                        code: format!(
                            "{}.{}.{}.forbidden",
                            self.code_prefix, scope.code_segment, code_name
                        ),
                        message: describe(
                            format!(
                                "`{}` must not be sent to {}.",
                                param.name, self.metadata.display_name
                            ),
                            contract.description.as_deref(),
                        ),
                        severity: contract.severity.unwrap_or(Severity::Error),
                        field: Some(scope.field(code_name)),
                        fix_hint: contract
                            .fix_hint
                            .clone()
                            .or_else(|| Some(format!("Remove the `{}` parameter.", param.name))),
                        source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                        targets: vec![param.target()],
                    });
                }
                return;
            }
            Requirement::Deprecated => {
                for param in addressed.iter().filter(|param| !param.missing) {
                    let code_name = finding_name(compiled, param);
                    violations.push(Violation {
                        code: format!(
                            "{}.{}.{}.deprecated",
                            self.code_prefix, scope.code_segment, code_name
                        ),
                        message: describe(
                            format!(
                                "`{}` is deprecated by {}.",
                                param.name, self.metadata.display_name
                            ),
                            contract.description.as_deref(),
                        ),
                        severity: contract.severity.unwrap_or(Severity::Warning),
                        field: Some(scope.field(code_name)),
                        fix_hint: contract.fix_hint.clone(),
                        source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                        targets: vec![param.target()],
                    });
                }
            }
            Requirement::Optional => {}
        }

        for param in addressed.iter().filter(|param| !param.missing) {
            let code_name = finding_name(compiled, param);
            let json_field = scope.body.as_ref().and_then(|body| {
                param
                    .location
                    .as_deref()
                    .and_then(|path| body.document.get(path))
            });
            let normalized_value = normalize_string(param.value.as_ref(), contract.normalization);

            // Structured constraints run before emptiness and scalar formats.
            // Otherwise a malformed object could satisfy a numeric contract,
            // and an empty array could hide a required batch's size violation.
            if let Some(reason) = constraint_violation(
                contract,
                param,
                json_field,
                scope.body.as_ref().map(|body| body.document),
                compiled.key_regex.as_ref(),
            ) {
                violations.push(Violation {
                    code: format!(
                        "{}.{}.{}.invalid",
                        self.code_prefix, scope.code_segment, code_name
                    ),
                    message: describe(
                        format!("`{}` {reason}", param.name),
                        contract.description.as_deref(),
                    ),
                    severity: contract
                        .format_severity
                        .or(contract.severity)
                        .unwrap_or(Severity::Error),
                    field: Some(scope.field(code_name)),
                    fix_hint: contract.fix_hint.clone(),
                    source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                    targets: vec![param.target()],
                });
                continue;
            }
            // Typed containers can be empty unless a minimum size is declared.
            // Explicit nullable contracts accept null without inventing a value.
            if let Some(field) = json_field
                && contract.json_type.is_some()
                && matches!(
                    field.kind,
                    JsonValueKind::Array | JsonValueKind::Object | JsonValueKind::Null
                )
                && !(field.kind != JsonValueKind::Null
                    && field.is_blank()
                    && matches!(contract.format, Some(ValueFormat::NonEmpty)))
            {
                continue;
            }
            if normalized_value.is_empty() {
                if contract.allow_empty {
                    continue;
                }
                violations.push(Violation {
                    code: format!(
                        "{}.{}.{}.empty",
                        self.code_prefix, scope.code_segment, code_name
                    ),
                    message: format!("`{}` is present but has an empty value.", param.name),
                    severity: contract.severity.unwrap_or(match contract.requirement {
                        Requirement::Required => Severity::Error,
                        _ => Severity::Warning,
                    }),
                    field: Some(scope.field(code_name)),
                    fix_hint: contract
                        .fix_hint
                        .clone()
                        .or_else(|| Some(format!("Populate `{}` before firing.", param.name))),
                    source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                    targets: vec![param.target()],
                });
                continue;
            }

            // Unexpanded macros are the core pack's business. Checking the
            // literal macro text against a value format would double-report the
            // same defect with a worse message.
            if contains_macro(param.value.as_ref()) {
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

            if let Some(reason) =
                format_violation(format, compiled.regex.as_ref(), normalized_value.as_ref())
            {
                violations.push(Violation {
                    code: format!(
                        "{}.{}.{}.invalid",
                        self.code_prefix, scope.code_segment, code_name
                    ),
                    message: describe(
                        format!("`{}` {reason}", param.name),
                        contract.description.as_deref(),
                    ),
                    severity: contract
                        .format_severity
                        .or(contract.severity)
                        .unwrap_or(Severity::Error),
                    field: Some(scope.field(code_name)),
                    fix_hint: contract.fix_hint.clone(),
                    source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                    targets: vec![param.target()],
                });
            } else {
                self.check_format_warnings(
                    &format!("{}.{}.{}", self.code_prefix, scope.code_segment, code_name),
                    format,
                    param,
                    &scope.field(code_name),
                    violations,
                );
            }
        }
    }

    fn check_format_warnings(
        &self,
        code: &str,
        format: &ValueFormat,
        param: &RawParam<'_>,
        field: &str,
        violations: &mut Vec<Violation>,
    ) {
        let kind = match format {
            ValueFormat::Tcf => "tcf",
            ValueFormat::Gpp => "gpp",
            ValueFormat::AdditionalConsent => "additional_consent",
            _ => return,
        };
        for message in crate::privacy::literal_format_warnings(kind, param.value.as_ref()) {
            violations.push(Violation {
                code: format!("{code}.policy_warning"),
                message: format!("`{}`: {message}", param.name),
                severity: Severity::Warning,
                field: Some(field.to_string()),
                fix_hint: Some(
                    "Check the CMP output against the cited consent specification.".to_string(),
                ),
                source: RuleSource {
                    level: RuleSourceLevel::Normative,
                    name: if kind == "additional_consent" {
                        "Google Additional Consent specification".to_string()
                    } else {
                        format!("IAB {kind} consent specification")
                    },
                    reference: Some(crate::privacy::consent_spec(kind).to_string()),
                },
                targets: vec![param.target()],
            });
        }
    }

    fn check_rule(
        &self,
        scope: &Scope,
        compiled: &CompiledRule,
        params: &[RawParam<'_>],
        violations: &mut Vec<Violation>,
    ) {
        let rule = &compiled.rule;
        if rule
            .condition
            .as_ref()
            .is_some_and(|condition| condition.evaluate(params) != Some(true))
        {
            return;
        }
        // An empty slot is not a value, so a rule must not read it as one.
        let live = |param: &RawParam| !param.missing;
        let present = |name: &str| {
            params
                .iter()
                .any(|param| live(param) && param.name.as_ref() == name)
        };
        let named = |name: &str| {
            params
                .iter()
                .find(|param| live(param) && param.name.as_ref() == name)
        };
        let hits_named = |names: &[String]| {
            params
                .iter()
                .filter(|param| {
                    live(param)
                        && names
                            .iter()
                            .any(|name| name.as_str() == param.name.as_ref())
                })
                .collect::<Vec<_>>()
        };
        let value_in = |values: &[String], param: &RawParam| {
            values
                .iter()
                .any(|value| value.as_str() == param.value.as_ref())
        };

        let (triggered, targets) = match &rule.assertion {
            Assertion::RequireOneOf { params: names } => {
                if names.iter().any(|name| present(name)) {
                    (false, Vec::new())
                } else {
                    (true, vec![scope.fallback()])
                }
            }
            Assertion::MutuallyExclusive { params: names } => {
                let hits = hits_named(names);
                if hits.len() > 1 {
                    (true, hits.iter().map(|param| param.target()).collect())
                } else {
                    (false, Vec::new())
                }
            }
            Assertion::RequiredWith { when, requires } => {
                if present(when) && !requires.iter().all(|name| present(name)) {
                    let target = named(when)
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
                    .filter(|param| live(param) && param.name.as_ref() == when)
                    .filter(|param| !contains_macro(param.value.as_ref()))
                    .any(|param| value_in(equals, param));

                if triggered && !requires.iter().all(|name| present(name)) {
                    let target = named(when)
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

                // Bracketed reserved names such as Amplitude's `[Amplitude]
                // Start Session` look like ad-tech macros. The forbid still
                // has to read the literal; a format check can wait for the
                // template to expand, this rule cannot.
                let hits: Vec<&RawParam> = params
                    .iter()
                    .filter(|param| {
                        live(param)
                            && (names.is_empty()
                                || names
                                    .iter()
                                    .any(|name| name.as_str() == param.name.as_ref()))
                    })
                    .filter(|param| regex.is_match(param.value.as_ref()))
                    .collect();

                if hits.is_empty() {
                    (false, Vec::new())
                } else {
                    (true, hits.iter().map(|param| param.target()).collect())
                }
            }
            Assertion::ValueWhen {
                when,
                equals,
                param,
                value,
            } => {
                let triggered = params
                    .iter()
                    .filter(|candidate| live(candidate) && candidate.name.as_ref() == when)
                    .filter(|candidate| !contains_macro(candidate.value.as_ref()))
                    .any(|candidate| value_in(equals, candidate));

                if !triggered {
                    (false, Vec::new())
                } else {
                    match named(param) {
                        Some(field)
                            if !contains_macro(field.value.as_ref())
                                && field.value.as_ref() != value =>
                        {
                            (true, vec![field.target()])
                        }
                        _ => (false, Vec::new()),
                    }
                }
            }
            Assertion::ForbiddenWhenValue {
                when,
                equals,
                params: names,
            } => {
                let triggered = params
                    .iter()
                    .filter(|candidate| live(candidate) && candidate.name.as_ref() == when)
                    .filter(|candidate| !contains_macro(candidate.value.as_ref()))
                    .any(|candidate| value_in(equals, candidate));

                if !triggered {
                    (false, Vec::new())
                } else {
                    let hits = hits_named(names);
                    if hits.is_empty() {
                        (false, Vec::new())
                    } else {
                        (true, hits.iter().map(|param| param.target()).collect())
                    }
                }
            }
            Assertion::RequireAnyOf {
                groups,
                allow_empty,
            } => {
                let meaningful = |name: &str| {
                    params.iter().any(|param| {
                        live(param)
                            && param.name.as_ref() == name
                            && (*allow_empty || param.container || !param.value.is_empty())
                    })
                };
                let satisfied = groups
                    .iter()
                    .any(|group| group.iter().all(|name| meaningful(name)));
                (
                    !satisfied,
                    if satisfied {
                        Vec::new()
                    } else {
                        vec![scope.fallback()]
                    },
                )
            }
            Assertion::FormatWhen {
                when,
                equals,
                param,
                format,
                pair_occurrences,
            } => {
                let applies = params.iter().any(|candidate| {
                    live(candidate)
                        && candidate.name.as_ref() == when
                        && !contains_macro(candidate.value.as_ref())
                        && value_in(equals, candidate)
                });
                let discriminators: Vec<_> = params
                    .iter()
                    .filter(|candidate| !candidate.missing && candidate.name.as_ref() == when)
                    .collect();
                for (index, candidate) in params
                    .iter()
                    .filter(|field| !field.missing && field.name.as_ref() == param)
                    .enumerate()
                {
                    let paired_applies = !*pair_occurrences
                        || discriminators.get(index).is_some_and(|field| {
                            !contains_macro(field.value.as_ref()) && value_in(equals, field)
                        });
                    if applies
                        && paired_applies
                        && !candidate.container
                        && !contains_macro(candidate.value.as_ref())
                    {
                        self.check_format_warnings(
                            &rule.code,
                            format,
                            candidate,
                            &scope.field(param),
                            violations,
                        );
                    }
                }
                let hits: Vec<_> = params
                    .iter()
                    .filter(|candidate| !candidate.missing && candidate.name.as_ref() == param)
                    .enumerate()
                    .filter(|(index, candidate)| {
                        let paired_applies = !*pair_occurrences
                            || discriminators.get(*index).is_some_and(|field| {
                                !contains_macro(field.value.as_ref()) && value_in(equals, field)
                            });
                        applies
                            && paired_applies
                            && live(candidate)
                            && candidate.name.as_ref() == param
                            && !candidate.container
                            && !contains_macro(candidate.value.as_ref())
                            && format_violation(
                                format,
                                compiled.regex.as_ref(),
                                candidate.value.as_ref(),
                            )
                            .is_some()
                    })
                    .map(|(_, candidate)| candidate)
                    .collect();
                (
                    !hits.is_empty(),
                    hits.iter().map(|param| param.target()).collect(),
                )
            }
            Assertion::LastParam { param } => {
                let last = params
                    .iter()
                    .filter(|candidate| candidate.component == ViolationTargetComponent::QueryParam)
                    .map(|candidate| candidate.start)
                    .max();
                let hits: Vec<_> = params
                    .iter()
                    .filter(|candidate| {
                        candidate.name.as_ref() == param
                            && candidate.component == ViolationTargetComponent::QueryParam
                            && Some(candidate.start) != last
                    })
                    .collect();
                (
                    !hits.is_empty(),
                    hits.iter().map(|param| param.target()).collect(),
                )
            }
            Assertion::MaxQueryLength { max_length } => {
                let query = scope
                    .artifact
                    .split_once('?')
                    .map(|(_, query)| query.split('#').next().unwrap_or(query));
                let triggered = query.is_some_and(|query| query.chars().count() > *max_length);
                (
                    triggered,
                    if triggered {
                        vec![scope.fallback()]
                    } else {
                        Vec::new()
                    },
                )
            }
            Assertion::RequireHttps => {
                let triggered = scope.body.is_none()
                    && scope.artifact.split_once(':').is_some_and(|(scheme, _)| {
                        !contains_macro(scheme) && !scheme.eq_ignore_ascii_case("https")
                    });
                (
                    triggered,
                    if triggered {
                        vec![scope.fallback()]
                    } else {
                        Vec::new()
                    },
                )
            }
            Assertion::MaxUrlLength { max_length } => {
                let triggered = scope.body.is_none()
                    && !contains_macro(scope.artifact)
                    && scope.artifact.chars().count() > *max_length;
                (
                    triggered,
                    if triggered {
                        vec![scope.fallback()]
                    } else {
                        Vec::new()
                    },
                )
            }
            Assertion::MaxBodyBytes { max_bytes } => {
                let triggered = scope.body.is_some() && scope.raw_artifact_len > *max_bytes;
                (
                    triggered,
                    if triggered {
                        vec![scope.fallback()]
                    } else {
                        Vec::new()
                    },
                )
            }
            Assertion::EqualValues { left, right } => match (named(left), named(right)) {
                (Some(left), Some(right))
                    if !left.container
                        && !right.container
                        && !left.value.is_empty()
                        && !right.value.is_empty()
                        && !contains_macro(&left.value)
                        && !contains_macro(&right.value)
                        && left.value != right.value =>
                {
                    (true, vec![left.target(), right.target()])
                }
                _ => (false, Vec::new()),
            },
            Assertion::UrlPathLength { param, max_length } => {
                let hits: Vec<_> = params
                    .iter()
                    .filter(|field| {
                        live(field)
                            && field.name.as_ref() == param
                            && !field.container
                            && !contains_macro(&field.value)
                            && url_path_character_count(&field.value) > *max_length
                    })
                    .collect();
                (
                    !hits.is_empty(),
                    hits.iter().map(|field| field.target()).collect(),
                )
            }
            Assertion::ValueWith { when, param, value } => match named(param) {
                Some(field)
                    if present(when)
                        && !contains_macro(field.value.as_ref())
                        && field.value.as_ref() != value =>
                {
                    (true, vec![field.target()])
                }
                _ => (false, Vec::new()),
            },
            Assertion::Format { param, format } => {
                for candidate in params.iter().filter(|field| {
                    live(field)
                        && field.name.as_ref() == param
                        && !field.container
                        && !contains_macro(field.value.as_ref())
                }) {
                    self.check_format_warnings(
                        &rule.code,
                        format,
                        candidate,
                        &scope.field(param),
                        violations,
                    );
                }
                let hits: Vec<_> = params
                    .iter()
                    .filter(|field| {
                        live(field)
                            && field.name.as_ref() == param
                            && !field.container
                            && !contains_macro(field.value.as_ref())
                            && format_violation(
                                format,
                                compiled.regex.as_ref(),
                                field.value.as_ref(),
                            )
                            .is_some()
                    })
                    .collect();
                (
                    !hits.is_empty(),
                    hits.iter().map(|field| field.target()).collect(),
                )
            }
            Assertion::LessEqual {
                left,
                right,
                strict,
                unit,
            } => match (named(left), named(right)) {
                (Some(left), Some(right))
                    if !contains_macro(left.value.as_ref())
                        && !contains_macro(right.value.as_ref())
                        && compare_ordered_values(
                            left.value.as_ref(),
                            right.value.as_ref(),
                            *unit,
                        )
                        .is_some_and(|order| {
                            order == std::cmp::Ordering::Greater
                                || (*strict && order == std::cmp::Ordering::Equal)
                        }) =>
                {
                    (true, vec![left.target(), right.target()])
                }
                _ => (false, Vec::new()),
            },
            Assertion::EqualSplitLengths {
                params: names,
                separator,
            } => {
                let hits: Vec<_> = hits_named(names)
                    .into_iter()
                    .filter(|param| {
                        !param.container
                            && !param.value.is_empty()
                            && !contains_macro(param.value.as_ref())
                    })
                    .collect();
                let first = hits
                    .first()
                    .map(|param| param.value.split(separator).count());
                let triggered = first.is_some_and(|length| {
                    hits.iter()
                        .any(|param| param.value.split(separator).count() != length)
                });
                (
                    triggered,
                    if triggered {
                        hits.iter().map(|param| param.target()).collect()
                    } else {
                        Vec::new()
                    },
                )
            }
            Assertion::EqualOccurrences { params: names } => {
                let counts: Vec<_> = names
                    .iter()
                    .map(|name| {
                        params
                            .iter()
                            .filter(|field| !field.missing && field.name.as_ref() == name)
                            .count()
                    })
                    .collect();
                let triggered = counts.windows(2).any(|counts| counts[0] != counts[1]);
                let targets = if triggered {
                    let hits: Vec<_> = params
                        .iter()
                        .filter(|field| {
                            !field.missing && names.iter().any(|name| field.name.as_ref() == name)
                        })
                        .map(RawParam::target)
                        .collect();
                    if hits.is_empty() {
                        vec![scope.fallback()]
                    } else {
                        hits
                    }
                } else {
                    Vec::new()
                };
                (triggered, targets)
            }
            Assertion::DelimitedSum {
                param,
                total,
                separator,
                value_separator,
            } => {
                let mut targets = Vec::new();
                for parts in hits_named(std::slice::from_ref(param)) {
                    for amount in hits_named(std::slice::from_ref(total)) {
                        if !parts.container
                            && !amount.container
                            && !contains_macro(parts.value.as_ref())
                            && !contains_macro(amount.value.as_ref())
                            && crate::decimal_sum::sum_matches(
                                parts.value.as_ref(),
                                amount.value.as_ref(),
                                separator,
                                value_separator,
                            ) == Some(false)
                        {
                            targets.extend([parts.target(), amount.target()]);
                        }
                    }
                }
                (!targets.is_empty(), targets)
            }
            Assertion::AwinBasket { parts, check } => {
                use crate::awin_basket::{BasketIssueKind, BasketParam};
                let pairs: Vec<_> = params
                    .iter()
                    .filter(|param| live(param))
                    .map(|param| BasketParam {
                        name: param.name.as_ref(),
                        value: param.value.as_ref(),
                        start: param.start,
                        end: param.end,
                    })
                    .collect();
                let group_parts = named(parts);
                let issues = crate::awin_basket::inspect_basket(
                    &pairs,
                    group_parts.map(|param| param.value.as_ref()),
                );
                let mut targets: Vec<_> = issues
                    .into_iter()
                    .filter(|issue| match check {
                        AwinBasketCheck::IndexSequence => matches!(
                            issue.kind,
                            BasketIssueKind::IndexGap | BasketIssueKind::DuplicateIndex
                        ),
                        AwinBasketCheck::GroupMembership => {
                            issue.kind == BasketIssueKind::GroupMissing
                        }
                    })
                    .map(|issue| ViolationTarget {
                        component: ViolationTargetComponent::QueryParam,
                        name: Some(issue.name),
                        value: Some(issue.value),
                        start: issue.start,
                        end: issue.end,
                    })
                    .collect();
                if !targets.is_empty()
                    && *check == AwinBasketCheck::GroupMembership
                    && let Some(parts) = group_parts
                {
                    targets.push(parts.target());
                }
                (!targets.is_empty(), targets)
            }
            Assertion::GppSections { param, sections } => {
                let mut ids = Vec::new();
                let mut id_targets = Vec::new();
                for field in hits_named(std::slice::from_ref(sections)) {
                    if field.container {
                        if let (Some(body), Some(path)) = (&scope.body, &field.location) {
                            for path in body.document.expand(&format!("{path}[]")) {
                                if let Some(id) = body.document.get(&path)
                                    && id.kind == JsonValueKind::Number
                                    && let Some(id) = canonical_gpp_section_id(id.text.as_ref())
                                {
                                    ids.push(id);
                                }
                            }
                        }
                    } else if !field.value.is_empty() && !contains_macro(field.value.as_ref()) {
                        ids.extend(field.value.split(',').map(str::to_string));
                    }
                    id_targets.push(field.target());
                }
                let mut targets = Vec::new();
                if !ids.is_empty() {
                    let ids = ids.join(",");
                    for field in hits_named(std::slice::from_ref(param)) {
                        if field.container
                            || contains_macro(field.value.as_ref())
                            || field.value.eq_ignore_ascii_case("REDACTED")
                        {
                            continue;
                        }
                        if let Ok(structure) =
                            crate::gpp_structure::validate_gpp_structure(field.value.as_ref())
                            && crate::gpp_structure::gpp_sid_mismatch(&ids, &structure).is_some()
                        {
                            targets.push(field.target());
                            targets.extend(id_targets.iter().cloned());
                        }
                    }
                }
                (!targets.is_empty(), targets)
            }
            Assertion::TcfConsent {
                param,
                vendor_id,
                purpose_ids,
            } => {
                let targets: Vec<_> = hits_named(std::slice::from_ref(param))
                    .into_iter()
                    .filter(|field| {
                        !field.container
                            && !contains_macro(field.value.as_ref())
                            && crate::tcf_sections::tcf_consent_requirements(
                                field.value.as_ref(),
                                *vendor_id,
                                purpose_ids,
                            ) == Ok(false)
                    })
                    .map(RawParam::target)
                    .collect();
                (!targets.is_empty(), targets)
            }
            Assertion::GppFieldValues {
                param,
                section_param,
                field,
                values,
            } => {
                let ids: Vec<_> = hits_named(std::slice::from_ref(section_param))
                    .into_iter()
                    .filter(|field| !contains_macro(field.value.as_ref()))
                    .flat_map(|field| {
                        if field.container {
                            scope
                                .body
                                .as_ref()
                                .zip(field.location.as_ref())
                                .map(|(body, path)| {
                                    body.document
                                        .expand(&format!("{path}[]"))
                                        .iter()
                                        .filter_map(|path| body.document.get(path))
                                        .filter_map(|id| {
                                            canonical_gpp_section_id(id.text.as_ref())
                                                .and_then(|value| value.parse::<u32>().ok())
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        } else {
                            field
                                .value
                                .split(',')
                                .filter_map(|id| id.parse::<u32>().ok())
                                .collect()
                        }
                    })
                    .collect();
                let targets: Vec<_> = hits_named(std::slice::from_ref(param))
                    .into_iter()
                    .filter(|signal| {
                        !signal.container
                            && !contains_macro(signal.value.as_ref())
                            && crate::gpp_structure::decoded_us_sections(signal.value.as_ref())
                                .iter()
                                .any(|section| {
                                    ids.contains(&section.section_id)
                                        && section.fields.get(field).is_some_and(|choices| {
                                            choices.iter().any(|choice| !values.contains(choice))
                                        })
                                })
                    })
                    .map(RawParam::target)
                    .collect();
                (!targets.is_empty(), targets)
            }
            Assertion::UniqueArrayBy { param, field } => {
                let mut duplicates = Vec::new();
                if let Some(body) = &scope.body {
                    for array in hits_named(std::slice::from_ref(param)) {
                        let Some(path) = &array.location else {
                            continue;
                        };
                        let mut groups: BTreeMap<(u8, String), Vec<ViolationTarget>> =
                            BTreeMap::new();
                        for item in body.document.expand(&format!("{path}[]")) {
                            let member = json::member_path(&item, field);
                            let Some(value) = body.document.get(&member) else {
                                continue;
                            };
                            if contains_macro(value.text.as_ref()) {
                                continue;
                            }
                            let key = match value.kind {
                                JsonValueKind::String => (0, value.text.to_string()),
                                JsonValueKind::Bool => (1, value.text.to_string()),
                                JsonValueKind::Number => {
                                    decimal_parts(value.text.as_ref()).map_or_else(
                                        // Exponents beyond the exact normalizer's range still
                                        // identify repeated identical JSON number spellings.
                                        || (3, value.text.to_string()),
                                        |(negative, digits, exponent)| {
                                            (2, format!("{negative}:{digits}:{exponent}"))
                                        },
                                    )
                                }
                                _ => continue,
                            };
                            groups.entry(key).or_default().push(body_target(
                                body.document,
                                &member,
                                scope.artifact,
                            ));
                        }
                        duplicates
                            .extend(groups.into_values().filter(|hits| hits.len() > 1).flatten());
                    }
                }
                (!duplicates.is_empty(), duplicates)
            }
            Assertion::EqualArrayLengths { params: names } => {
                let arrays: Vec<_> = hits_named(names)
                    .into_iter()
                    .filter_map(|field| {
                        let body = scope.body.as_ref()?;
                        let path = field.location.as_ref()?;
                        let value = body.document.get(path)?;
                        (value.kind == JsonValueKind::Array).then_some((field, value.len?))
                    })
                    .collect();
                let triggered = arrays
                    .first()
                    .is_some_and(|(_, count)| arrays.iter().any(|(_, other)| count != other));
                (
                    triggered,
                    if triggered {
                        arrays
                            .into_iter()
                            .map(|(field, _)| field.target())
                            .collect()
                    } else {
                        Vec::new()
                    },
                )
            }
            Assertion::TimeWindow {
                param,
                fallback_param,
                unit,
                max_age_seconds,
                max_future_seconds,
            } => {
                let effective_param = if params
                    .iter()
                    .any(|candidate| !candidate.missing && candidate.name.as_ref() == param)
                {
                    param.as_str()
                } else {
                    fallback_param.as_deref().unwrap_or(param)
                };
                let hits: Vec<_> = params
                    .iter()
                    .filter(|candidate| {
                        if candidate.name.as_ref() != effective_param
                            || !live(candidate)
                            || candidate.container
                            || contains_macro(candidate.value.as_ref())
                        {
                            return false;
                        }
                        if *unit == TimestampUnit::Microseconds {
                            if candidate.value.parse::<serde_json::Number>().is_err() {
                                return false;
                            }
                            let reference = i128::from(scope.reference_time_unix_seconds);
                            let too_old = max_age_seconds.is_some_and(|age| {
                                let bound = ((reference - i128::from(age)) * 1_000_000).to_string();
                                compare_numeric_text(candidate.value.as_ref(), &bound)
                                    .is_none_or(|order| order == std::cmp::Ordering::Less)
                            });
                            let too_new = max_future_seconds.is_some_and(|future| {
                                let bound =
                                    ((reference + i128::from(future)) * 1_000_000).to_string();
                                compare_numeric_text(candidate.value.as_ref(), &bound)
                                    .is_none_or(|order| order == std::cmp::Ordering::Greater)
                            });
                            return too_old || too_new;
                        }
                        let Some(time) =
                            crate::timestamp::timestamp_millis(candidate.value.as_ref(), *unit)
                        else {
                            // A numeric timestamp that cannot be represented must
                            // not bypass the window. Non-numeric syntax belongs to
                            // the field's native type and scalar format contracts.
                            return !matches!(
                                unit,
                                TimestampUnit::Datetime | TimestampUnit::DatetimeUtc
                            ) && candidate.value.parse::<serde_json::Number>().is_ok();
                        };
                        let delta =
                            i128::from(time) - i128::from(scope.reference_time_unix_seconds) * 1000;
                        max_age_seconds.is_some_and(|age| delta < -(i128::from(age) * 1000))
                            || max_future_seconds
                                .is_some_and(|future| delta > i128::from(future) * 1000)
                    })
                    .collect();
                (
                    !hits.is_empty(),
                    hits.iter().map(|param| param.target()).collect(),
                )
            }
            Assertion::MinLengthFrom {
                params: names,
                length_param,
                default_length,
            } => {
                let min = match named(length_param) {
                    Some(field) if contains_macro(field.value.as_ref()) => return,
                    Some(field) => match decimal_usize(field.value.as_ref()) {
                        Some(length) => length,
                        None => return,
                    },
                    None => *default_length,
                };
                let hits: Vec<_> = hits_named(names)
                    .into_iter()
                    .filter(|field| {
                        !field.container
                            && !contains_macro(field.value.as_ref())
                            && field.value.chars().count() < min
                    })
                    .collect();
                (
                    !hits.is_empty(),
                    hits.iter().map(|field| field.target()).collect(),
                )
            }
        };

        if triggered {
            violations.push(Violation {
                code: rule.code.clone(),
                message: rule.message.clone(),
                severity: rule.severity,
                field: None,
                fix_hint: rule.fix_hint.clone(),
                source: self.source_for(compiled.source_level, compiled.doc.as_deref()),
                targets,
            });
        }
    }
}

fn normalize_alias(name: &mut String, contracts: &[ParamContract]) {
    if let Some(contract) = contracts
        .iter()
        .find(|contract| contract.aliases.contains(name))
    {
        *name = contract.name.clone();
    }
}

fn normalize_condition_aliases(condition: &mut RuleCondition, contracts: &[ParamContract]) {
    match condition {
        RuleCondition::Exists { param }
        | RuleCondition::Present { param }
        | RuleCondition::ValueIn { param, .. }
        | RuleCondition::ValuePattern { param, .. }
        | RuleCondition::JsonType { param, .. } => normalize_alias(param, contracts),
        RuleCondition::All { conditions } | RuleCondition::Any { conditions } => {
            for condition in conditions {
                normalize_condition_aliases(condition, contracts);
            }
        }
        RuleCondition::Not { condition } => normalize_condition_aliases(condition, contracts),
    }
}

fn normalize_param_aliases(params: &mut [RawParam<'_>], contracts: &[CompiledParam]) {
    for param in params {
        if let Some(contract) = contracts.iter().find(|contract| {
            contract
                .contract
                .aliases
                .iter()
                .any(|alias| alias == param.name.as_ref())
        }) {
            if param.location.is_none() {
                param.location = Some(param.name.to_string());
            }
            param.name = Cow::Owned(contract.contract.name.clone());
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

fn http_param_reads_body(param: &CompiledParam) -> bool {
    param.contract.root_path.as_ref().is_some_and(|path| {
        path == "body" || path.starts_with("body.") || path.starts_with("body[")
    }) || param
        .names
        .iter()
        .any(|name| name == "body" || name.starts_with("body.") || name.starts_with("body["))
}

fn assertion_requires_presence(assertion: &Assertion) -> bool {
    matches!(
        assertion,
        Assertion::RequireOneOf { .. }
            | Assertion::RequireAnyOf { .. }
            | Assertion::RequiredWith { .. }
            | Assertion::RequiredWhenValue { .. }
    )
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
        Assertion::ValueWhen { when, param, .. } => vec![when, param],
        Assertion::FormatWhen { when, param, .. } => vec![when, param],
        Assertion::Format { param, .. } => vec![param],
        Assertion::RequireAnyOf { groups, .. } => groups.iter().flatten().collect(),
        Assertion::LastParam { param } => vec![param],
        Assertion::MaxQueryLength { .. }
        | Assertion::MaxBodyBytes { .. }
        | Assertion::RequireHttps
        | Assertion::MaxUrlLength { .. } => Vec::new(),
        Assertion::ValueWith { when, param, .. } => vec![when, param],
        Assertion::LessEqual { left, right, .. } | Assertion::EqualValues { left, right } => {
            vec![left, right]
        }
        Assertion::UrlPathLength { param, .. } => vec![param],
        Assertion::EqualSplitLengths { params, .. } => params.iter().collect(),
        Assertion::DelimitedSum { param, total, .. } => vec![param, total],
        Assertion::AwinBasket { parts, .. } => vec![parts],
        Assertion::GppSections { param, sections } => vec![param, sections],
        Assertion::TcfConsent { param, .. } => vec![param],
        Assertion::GppFieldValues {
            param,
            section_param,
            ..
        } => vec![param, section_param],
        Assertion::EqualArrayLengths { params } => params.iter().collect(),
        Assertion::UniqueArrayBy { param, .. } => vec![param],
        Assertion::EqualOccurrences { params } => params.iter().collect(),
        Assertion::TimeWindow {
            param,
            fallback_param,
            ..
        } => std::iter::once(param)
            .chain(fallback_param.iter())
            .collect(),
        Assertion::MinLengthFrom {
            params,
            length_param,
            ..
        } => params.iter().chain(std::iter::once(length_param)).collect(),
        Assertion::ForbiddenWhenValue { when, params, .. } => {
            let mut names = vec![when];
            names.extend(params.iter());
            names
        }
    }
}

fn assertion_params_mut(assertion: &mut Assertion) -> Vec<&mut String> {
    match assertion {
        Assertion::RequireOneOf { params } | Assertion::MutuallyExclusive { params } => {
            params.iter_mut().collect()
        }
        Assertion::RequiredWith { when, requires }
        | Assertion::RequiredWhenValue { when, requires, .. } => {
            let mut names = vec![when];
            names.extend(requires.iter_mut());
            names
        }
        Assertion::ForbidValuePattern { params, .. } => params.iter_mut().collect(),
        Assertion::ValueWhen { when, param, .. } => vec![when, param],
        Assertion::FormatWhen { when, param, .. } => vec![when, param],
        Assertion::Format { param, .. } => vec![param],
        Assertion::RequireAnyOf { groups, .. } => groups.iter_mut().flatten().collect(),
        Assertion::LastParam { param } => vec![param],
        Assertion::MaxQueryLength { .. }
        | Assertion::MaxBodyBytes { .. }
        | Assertion::RequireHttps
        | Assertion::MaxUrlLength { .. } => Vec::new(),
        Assertion::ValueWith { when, param, .. } => vec![when, param],
        Assertion::LessEqual { left, right, .. } | Assertion::EqualValues { left, right } => {
            vec![left, right]
        }
        Assertion::UrlPathLength { param, .. } => vec![param],
        Assertion::EqualSplitLengths { params, .. } => params.iter_mut().collect(),
        Assertion::DelimitedSum { param, total, .. } => vec![param, total],
        Assertion::AwinBasket { parts, .. } => vec![parts],
        Assertion::GppSections { param, sections } => vec![param, sections],
        Assertion::TcfConsent { param, .. } => vec![param],
        Assertion::GppFieldValues {
            param,
            section_param,
            ..
        } => vec![param, section_param],
        Assertion::EqualArrayLengths { params } => params.iter_mut().collect(),
        Assertion::UniqueArrayBy { param, .. } => vec![param],
        Assertion::EqualOccurrences { params } => params.iter_mut().collect(),
        Assertion::TimeWindow {
            param,
            fallback_param,
            ..
        } => std::iter::once(param)
            .chain(fallback_param.iter_mut())
            .collect(),
        Assertion::MinLengthFrom {
            params,
            length_param,
            ..
        } => params
            .iter_mut()
            .chain(std::iter::once(length_param))
            .collect(),
        Assertion::ForbiddenWhenValue { when, params, .. } => {
            let mut names = vec![when];
            names.extend(params.iter_mut());
            names
        }
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

fn exact_param_names(params: &[CompiledParam]) -> BTreeSet<String> {
    params
        .iter()
        .filter(|compiled| compiled.name_regex.is_none())
        .flat_map(|compiled| compiled.names.iter().cloned())
        .collect()
}

fn finding_name<'a>(compiled: &'a CompiledParam, param: &'a RawParam<'_>) -> &'a str {
    if compiled.contract.name.is_empty() {
        return "root";
    }
    if compiled.name_regex.is_some() {
        param.name.as_ref()
    } else {
        compiled.contract.name.as_str()
    }
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

/// Compare integer bounds without rounding them through f64 first.
fn compare_numbers(
    left: &serde_json::Number,
    right: &serde_json::Number,
) -> Option<std::cmp::Ordering> {
    compare_numeric_text(&left.to_string(), &right.to_string())
}

// Normalize decimal text without rounding a JSON number through a float.
fn decimal_parts(value: &str) -> Option<(bool, String, i64)> {
    let _number = value.parse::<serde_json::Number>().ok()?;
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (significand, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((significand, exponent)) => (significand, exponent.parse::<i64>().ok()?),
        None => (unsigned, 0),
    };
    let fraction_len = significand
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let digits = significand.replace('.', "");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, "0".to_string(), 0));
    }
    let stripped = digits.trim_end_matches('0');
    let exponent = exponent
        .checked_sub(i64::try_from(fraction_len).ok()?)?
        .checked_add(i64::try_from(digits.len() - stripped.len()).ok()?)?;
    Some((negative, stripped.to_string(), exponent))
}

fn compare_numeric_text(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let (left_negative, mut left_digits, left_exp) = decimal_parts(left)?;
    let (right_negative, mut right_digits, right_exp) = decimal_parts(right)?;
    if left_negative != right_negative {
        return Some(if left_negative {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        });
    }
    let order = match (left_digits == "0", right_digits == "0") {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => {
            let left_places = i64::try_from(left_digits.len())
                .ok()?
                .checked_add(left_exp)?;
            let right_places = i64::try_from(right_digits.len())
                .ok()?
                .checked_add(right_exp)?;
            let magnitude = left_places.cmp(&right_places);
            if magnitude == std::cmp::Ordering::Equal {
                let width = left_digits.len().max(right_digits.len());
                left_digits.extend(std::iter::repeat_n('0', width - left_digits.len()));
                right_digits.extend(std::iter::repeat_n('0', width - right_digits.len()));
                left_digits.cmp(&right_digits)
            } else {
                magnitude
            }
        }
    };
    Some(if left_negative {
        order.reverse()
    } else {
        order
    })
}

fn compare_ordered_values(
    left: &str,
    right: &str,
    unit: Option<TimestampUnit>,
) -> Option<std::cmp::Ordering> {
    match unit {
        Some(unit @ (TimestampUnit::Datetime | TimestampUnit::DatetimeUtc)) => {
            let left_seconds = crate::timestamp::timestamp_millis(left, unit)?.div_euclid(1000);
            let right_seconds = crate::timestamp::timestamp_millis(right, unit)?.div_euclid(1000);
            let order = left_seconds.cmp(&right_seconds);
            if order != std::cmp::Ordering::Equal {
                return Some(order);
            }
            let fraction = |value: &str| {
                value
                    .split_once('.')
                    .map(|(_, fraction)| {
                        let count = fraction.bytes().take_while(u8::is_ascii_digit).count();
                        format!("0.{}", &fraction[..count])
                    })
                    .unwrap_or_else(|| "0".to_string())
            };
            compare_numeric_text(&fraction(left), &fraction(right))
        }
        Some(TimestampUnit::SecondsOrMilliseconds) => Some(
            crate::timestamp::timestamp_millis(left, TimestampUnit::SecondsOrMilliseconds)?.cmp(
                &crate::timestamp::timestamp_millis(right, TimestampUnit::SecondsOrMilliseconds)?,
            ),
        ),
        _ => compare_numeric_text(left, right),
    }
}

fn url_path_character_count(value: &str) -> usize {
    // Count the submitted path rather than URL::path(), which percent-encodes
    // Unicode and can change the number of characters on the wire.
    let without_suffix = value.split(['?', '#']).next().unwrap_or(value);
    let path = if let Some((_, authority)) = without_suffix.split_once("://") {
        authority.find('/').map_or("/", |start| &authority[start..])
    } else if let Some(authority) = without_suffix.strip_prefix("//") {
        authority.find('/').map_or("/", |start| &authority[start..])
    } else {
        without_suffix
    };
    path.chars().count()
}

pub(crate) fn decode_base64_json(value: &str) -> Result<String, String> {
    String::from_utf8(decode_base64_bytes(value)?)
        .map_err(|_| "decoded base64 is not UTF-8 JSON".to_string())
}

fn decode_base64_latin1_json(value: &str) -> Result<String, String> {
    Ok(decode_base64_bytes(value)?
        .into_iter()
        .map(char::from)
        .collect())
}

fn decode_percent_encoded_json(value: &str) -> Result<String, String> {
    let mut output = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes
                .next()
                .and_then(|digit| char::from(digit).to_digit(16));
            let low = bytes
                .next()
                .and_then(|digit| char::from(digit).to_digit(16));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("invalid percent escape in encoded JSON".into());
            };
            output.push((high * 16 + low) as u8);
        } else {
            output.push(byte);
        }
    }
    String::from_utf8(output).map_err(|_| "percent-decoded JSON is not UTF-8".into())
}

fn decode_query_params_json(value: &str) -> Result<String, String> {
    let mut fields = serde_json::Map::new();
    for field in value.split('&').filter(|field| !field.is_empty()) {
        let (name, value) = field.split_once('=').unwrap_or((field, ""));
        let name = decode_percent_encoded_json(&name.replace('+', " "))?;
        let value =
            serde_json::Value::String(decode_percent_encoded_json(&value.replace('+', " "))?);
        match fields.entry(name) {
            serde_json::map::Entry::Vacant(entry) => {
                entry.insert(value);
            }
            serde_json::map::Entry::Occupied(mut entry) => {
                if let serde_json::Value::Array(values) = entry.get_mut() {
                    values.push(value);
                } else {
                    let first = entry.get().clone();
                    entry.insert(serde_json::Value::Array(vec![first, value]));
                }
            }
        }
    }
    serde_json::to_string(&fields).map_err(|error| error.to_string())
}

fn decode_base64_bytes(value: &str) -> Result<Vec<u8>, String> {
    let data = value.trim_end_matches('=');
    let padding = value.len() - data.len();
    if padding > 2
        || data.len() % 4 == 1
        || (padding > 0 && (!value.len().is_multiple_of(4) || padding != (4 - data.len() % 4) % 4))
    {
        return Err("invalid base64 length or padding".to_string());
    }
    let mut output = Vec::with_capacity(data.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut bits = 0;
    for byte in data.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return Err("invalid base64 character".to_string()),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    if buffer != 0 {
        return Err("nonzero base64 padding bits".to_string());
    }
    Ok(output)
}

fn exact_multiple(value: &str, step: &serde_json::Number) -> Option<bool> {
    let (_, digits, exponent) = decimal_parts(value)?;
    let (_, divisor, divisor_exponent) = decimal_parts(&step.to_string())?;
    if digits == "0" {
        return Some(true);
    }
    let powers = exponent.checked_sub(divisor_exponent)?;
    // Normalized digits have no trailing zero. An extra factor of ten in the
    // divisor therefore cannot divide the numerator exactly.
    if powers < 0 {
        return Some(false);
    }
    let divisor: u128 = divisor.parse().ok()?;
    let mut remainder = 0_u128;
    for digit in digits.bytes() {
        remainder = add_mod(
            multiply_mod(remainder, 10 % divisor, divisor),
            u128::from(digit - b'0') % divisor,
            divisor,
        );
    }
    let mut powers = u64::try_from(powers).ok()?;
    let mut factor = 10_u128 % divisor;
    while powers > 0 {
        if powers % 2 == 1 {
            remainder = multiply_mod(remainder, factor, divisor);
        }
        factor = multiply_mod(factor, factor, divisor);
        powers /= 2;
    }
    Some(remainder == 0)
}

fn add_mod(left: u128, right: u128, modulus: u128) -> u128 {
    if left >= modulus - right {
        left - (modulus - right)
    } else {
        left + right
    }
}

fn multiply_mod(mut left: u128, mut right: u128, modulus: u128) -> u128 {
    if let Some(product) = left.checked_mul(right) {
        return product % modulus;
    }
    let mut result = 0;
    while right > 0 {
        if right % 2 == 1 {
            result = add_mod(result, left, modulus);
        }
        left = add_mod(left, left, modulus);
        right /= 2;
    }
    result
}

fn decimal_usize(value: &str) -> Option<usize> {
    let (negative, mut digits, exponent) = decimal_parts(value)?;
    if negative || !(0..=20).contains(&exponent) {
        return None;
    }
    digits.extend(std::iter::repeat_n('0', usize::try_from(exponent).ok()?));
    digits.parse().ok()
}

// JSON integers can use decimal or exponent notation. Preserve every integral
// ID for membership checking without allocating the expanded exponent.
fn canonical_gpp_section_id(value: &str) -> Option<String> {
    let (negative, mut digits, exponent) = decimal_parts(value)?;
    if exponent < 0 {
        return None;
    }
    if negative {
        return Some(
            if digits == "1" && exponent == 0 {
                "-1"
            } else {
                "-2"
            }
            .into(),
        );
    }
    if digits.len() > 10 || exponent > 10 || digits.len() + exponent as usize > 10 {
        return Some("4294967296".into());
    }
    digits.extend(std::iter::repeat_n('0', exponent as usize));
    Some(digits)
}

fn matches_json_type(kind: JsonType, field: &JsonField<'_>) -> bool {
    match kind {
        JsonType::String => field.kind == JsonValueKind::String,
        JsonType::Number => field.kind == JsonValueKind::Number,
        JsonType::Integer => {
            field.kind == JsonValueKind::Number
                && decimal_parts(field.text.as_ref()).is_some_and(|(_, _, exponent)| exponent >= 0)
        }
        JsonType::Boolean => field.kind == JsonValueKind::Bool,
        JsonType::Object => field.kind == JsonValueKind::Object,
        JsonType::Array => field.kind == JsonValueKind::Array,
        JsonType::Null => field.kind == JsonValueKind::Null,
    }
}

fn json_contains_macro(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => contains_macro(value),
        serde_json::Value::Array(values) => values.iter().any(json_contains_macro),
        serde_json::Value::Object(values) => values
            .iter()
            .any(|(key, value)| contains_macro(key) || json_contains_macro(value)),
        _ => false,
    }
}

fn constraint_violation(
    contract: &ParamContract,
    param: &RawParam<'_>,
    field: Option<&JsonField<'_>>,
    document: Option<&JsonDocument<'_>>,
    key_regex: Option<&Regex>,
) -> Option<String> {
    let unresolved_scalar = !param.container && contains_macro(param.value.as_ref());
    if let (Some(types), Some(field)) = (&contract.json_type, field)
        && !types
            .types()
            .iter()
            .any(|kind| matches_json_type(*kind, field))
        && !(unresolved_scalar
            && types
                .types()
                .iter()
                .any(|kind| !matches!(kind, JsonType::Object | JsonType::Array)))
    {
        return Some(format!(
            "must have JSON type {}, but has type {}.",
            types
                .types()
                .iter()
                .map(|kind| format!("{kind:?}").to_lowercase())
                .collect::<Vec<_>>()
                .join(" or "),
            field.kind.label()
        ));
    }
    if let (Some(document), Some(path)) = (document, param.location.as_deref()) {
        if let Some(name) = document.member_name(path) {
            if !contains_macro(name)
                && contract
                    .max_key_length
                    .is_some_and(|max| name.chars().count() > max)
            {
                return Some(format!(
                    "has an object member name longer than {} characters.",
                    contract.max_key_length.unwrap()
                ));
            }
            if !contains_macro(name) && key_regex.is_some_and(|regex| !regex.is_match(name)) {
                return Some(
                    "has an object member name outside the documented grammar.".to_string(),
                );
            }
        }
        if let (Some(max), Some(field)) = (contract.max_compact_bytes, field) {
            let bytes = serde_json::from_str::<serde_json::Value>(
                &document.artifact()[field.start..field.end],
            )
            .ok()
            .filter(|value| !json_contains_macro(value))
            .and_then(|value| serde_json::to_vec(&value).ok())
            .map(|bytes| bytes.len());
            if bytes.is_some_and(|bytes| bytes > max) {
                return Some(format!(
                    "exceeds the compact UTF-8 JSON size threshold of {max} bytes."
                ));
            }
        }
    }
    for (min, max, kind, label) in [
        (
            contract.min_items,
            contract.max_items,
            JsonValueKind::Array,
            "array elements",
        ),
        (
            contract.min_properties,
            contract.max_properties,
            JsonValueKind::Object,
            "object properties",
        ),
    ] {
        if min.is_none() && max.is_none() {
            continue;
        }
        let Some(field) = field else {
            continue;
        };
        if field.kind != kind {
            if contract.json_type.is_some() {
                continue;
            }
            return Some(format!(
                "must be a JSON {} to check its size.",
                kind.label()
            ));
        }
        let len = if kind == JsonValueKind::Object && !contract.property_exclusions.is_empty() {
            field
                .member_names
                .iter()
                .filter(|name| !contract.property_exclusions.contains(name))
                .count()
        } else {
            field.len.unwrap_or(0)
        };
        if let Some(min) = min
            && len < min
        {
            return Some(format!(
                "must contain at least {min} {label}, but contains {len}."
            ));
        }
        if let Some(max) = max
            && len > max
        {
            return Some(format!(
                "must contain at most {max} {label}, but contains {len}."
            ));
        }
    }
    if !param.container && contains_macro(param.value.as_ref()) {
        return None;
    }
    if let Some(maximum) = contract.max_utf16_length {
        let applies = field.is_none_or(|field| field.kind == JsonValueKind::String);
        if !applies && contract.json_type.is_none() {
            return Some("must be a string to check its length.".to_string());
        }
        if applies {
            let length = normalize_string(param.value.as_ref(), contract.normalization)
                .encode_utf16()
                .count();
            if length > maximum {
                return Some(format!(
                    "must contain at most {maximum} UTF-16 code units, but contains {length}."
                ));
            }
        }
    }
    if let (Some(document), Some(path), Some(field)) = (document, param.location.as_deref(), field)
    {
        if let (Some(max), Some(separator)) = (contract.max_joined_length, &contract.join_separator)
            && field.kind == JsonValueKind::Array
        {
            let values: Vec<_> = document
                .expand(&format!("{path}[]"))
                .iter()
                .filter_map(|path| document.get(path))
                .collect();
            if values.iter().all(|value| {
                value.kind == JsonValueKind::String && !contains_macro(value.text.as_ref())
            }) {
                let length = values
                    .iter()
                    .fold(0usize, |sum, value| {
                        sum.saturating_add(value.text.chars().count())
                    })
                    .saturating_add(
                        values
                            .len()
                            .saturating_sub(1)
                            .saturating_mul(separator.chars().count()),
                    );
                if length > max {
                    return Some(format!(
                        "must join to at most {max} characters, but contains {length}."
                    ));
                }
            }
        }
        if let Some(max) = contract.max_depth {
            if matches!(field.kind, JsonValueKind::Object | JsonValueKind::Array) {
                if let Some(depth) = document.subtree_depth(path)
                    && depth > max
                {
                    return Some(format!(
                        "must have nesting depth at most {max}, but has {depth}."
                    ));
                }
            } else if contract.json_type.is_none() {
                return Some("must be a JSON container to check nesting depth.".to_string());
            }
        }
        if let Some(max) = contract.max_member_values {
            if let Some(count) = document.member_value_count(path) {
                if count > max {
                    return Some(format!(
                        "must contain at most {max} member values, but contains {count}."
                    ));
                }
            } else if contract.json_type.is_none() {
                return Some("must be a JSON object to count member values.".to_string());
            }
        }
    }
    for (min, max, use_bytes) in [
        (contract.min_length, contract.max_length, false),
        (contract.min_byte_length, contract.max_byte_length, true),
    ] {
        if min.is_none() && max.is_none() {
            continue;
        }
        let applies = field.is_none_or(|field| field.kind == JsonValueKind::String);
        if !applies && contract.json_type.is_none() {
            return Some("must be a string to check its length.".to_string());
        }
        if applies {
            let value = normalize_string(
                field.map_or(param.value.as_ref(), |field| field.text.as_ref()),
                contract.normalization,
            );
            let len = if use_bytes {
                value.len()
            } else {
                value.chars().count()
            };
            let units = if use_bytes {
                "UTF-8 bytes"
            } else {
                "characters"
            };
            if let Some(min) = min
                && len < min
            {
                return Some(format!(
                    "must contain at least {min} {units}, but contains {len}."
                ));
            }
            if let Some(max) = max
                && len > max
            {
                return Some(format!(
                    "must contain at most {max} {units}, but contains {len}."
                ));
            }
        }
    }
    if contract.minimum.is_some() || contract.maximum.is_some() || contract.multiple_of.is_some() {
        let applies = field.is_none_or(|field| {
            field.kind == JsonValueKind::Number
                || (field.kind == JsonValueKind::String
                    && (contract.numeric_strings
                        || contract.json_type.as_ref().is_none_or(|types| {
                            !types.types().contains(&JsonType::Number)
                                && !types.types().contains(&JsonType::Integer)
                        })))
        });
        if !applies && contract.json_type.is_some() {
            return None;
        }
        let value = normalize_string(
            field.map_or(param.value.as_ref(), |field| field.text.as_ref()),
            contract.normalization,
        );
        let Ok(number) = value.parse::<serde_json::Number>() else {
            return Some("must be a number to check its bounds.".to_string());
        };
        if decimal_parts(value.as_ref()).is_none() {
            return Some(
                "must have a representable decimal exponent to check its bounds.".to_string(),
            );
        }
        if let Some(min) = &contract.minimum
            && compare_numeric_text(value.as_ref(), &min.to_string())
                == Some(std::cmp::Ordering::Less)
        {
            return Some(format!("must be at least {min}, but is {number}."));
        }
        if let Some(max) = &contract.maximum
            && compare_numeric_text(value.as_ref(), &max.to_string())
                == Some(std::cmp::Ordering::Greater)
        {
            return Some(format!("must be at most {max}, but is {number}."));
        }
        if let Some(step) = &contract.multiple_of
            && exact_multiple(value.as_ref(), step) != Some(true)
        {
            return Some(format!("must be a multiple of {step}, but is {number}."));
        }
    }
    None
}

fn parse_cidr(value: &str) -> Option<(std::net::IpAddr, u8)> {
    let (address, prefix) = value.split_once('/')?;
    let address: std::net::IpAddr = address.parse().ok()?;
    let prefix: u8 = prefix.parse().ok()?;
    (prefix <= if address.is_ipv4() { 32 } else { 128 }).then_some((address, prefix))
}

fn in_cidr(address: std::net::IpAddr, network: std::net::IpAddr, prefix: u8) -> bool {
    match (address, network) {
        (std::net::IpAddr::V4(address), std::net::IpAddr::V4(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            (u32::from(address) & mask) == (u32::from(network) & mask)
        }
        (std::net::IpAddr::V6(address), std::net::IpAddr::V6(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            (u128::from(address) & mask) == (u128::from(network) & mask)
        }
        _ => false,
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
            match url::Url::parse(decoded.as_ref()) {
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
        ValueFormat::Ip {
            version,
            exclude_ranges,
        } => {
            let valid = value
                .parse::<std::net::IpAddr>()
                .ok()
                .is_some_and(|address| {
                    (match version {
                        None => true,
                        Some(IpVersion::V4) => address.is_ipv4(),
                        Some(IpVersion::V6) => address.is_ipv6(),
                    }) && !exclude_ranges.iter().any(|range| {
                        let (network, prefix) =
                            parse_cidr(range).expect("CIDR validated at compilation");
                        in_cidr(address, network, prefix)
                    })
                });
            if valid {
                None
            } else {
                Some(format!(
                    "must be an IP address literal of the documented family outside excluded ranges, but is `{value}`."
                ))
            }
        }
        ValueFormat::DateTime {
            require_timezone,
            allow_date_only,
            allow_basic,
            allow_space_separator,
            allow_javascript_date,
            allow_unpadded_date,
        } => {
            let normalized;
            let value = if *allow_space_separator && value.as_bytes().get(10) == Some(&b' ') {
                normalized = format!("{}T{}", &value[..10], &value[11..]);
                normalized.as_str()
            } else {
                value
            };
            if valid_datetime(value, *require_timezone, *allow_date_only, *allow_basic)
                || (*allow_javascript_date && crate::javascript_date::valid_javascript_date(value))
                || (*allow_date_only && *allow_unpadded_date && valid_unpadded_date(value))
            {
                None
            } else {
                Some(format!(
                    "must be a calendar-valid RFC 3339 date and time{}; `{value}` is invalid.",
                    if *require_timezone {
                        " with a timezone"
                    } else {
                        ""
                    }
                ))
            }
        }
        ValueFormat::Date => {
            if valid_datetime(&format!("{value}T00:00:00Z"), true, false, false) {
                None
            } else {
                Some(format!(
                    "must be a valid YYYY-MM-DD calendar date, but is `{value}`."
                ))
            }
        }
        ValueFormat::Json => serde_json::from_str::<serde_json::Value>(value)
            .err()
            .map(|error| format!("must contain valid JSON: {error}")),
        ValueFormat::AdobeProducts => crate::adobe_products::validate_products(value),
        ValueFormat::AdobeEvents => crate::adobe_events::validate_events(value),
        ValueFormat::AdditionalConsent => {
            crate::google_additional_consent::validate_additional_consent(value)
        }
        ValueFormat::BrazeTime => crate::braze_time::validate_braze_time(value).err(),
        ValueFormat::Currency {
            allow_historical,
            case_insensitive,
        } => {
            if crate::currency::known_currency(value, *allow_historical)
                && (*case_insensitive || value == value.to_ascii_uppercase())
            {
                None
            } else {
                Some(format!(
                    "must be a{} assigned ISO 4217 currency code, but is `{value}`.",
                    if *case_insensitive {
                        "n"
                    } else {
                        "n uppercase"
                    }
                ))
            }
        }
        ValueFormat::Tcf => crate::privacy::literal_format_error("tcf", value),
        ValueFormat::Gpp => crate::privacy::literal_format_error("gpp", value),
        ValueFormat::UsPrivacy => crate::privacy::literal_format_error("us_privacy", value),
    }
}

pub(crate) fn valid_datetime(
    value: &str,
    require_timezone: bool,
    allow_date_only: bool,
    allow_basic: bool,
) -> bool {
    if !value.is_ascii() {
        return false;
    }
    let Some((date, time)) = value.split_once(['T', 't']) else {
        return allow_date_only
            && valid_datetime(&format!("{value}T00:00:00Z"), true, false, allow_basic);
    };
    let normalized_date;
    let date = if allow_basic && date.len() == 8 && date.bytes().all(|byte| byte.is_ascii_digit()) {
        normalized_date = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]);
        normalized_date.as_str()
    } else {
        date
    };
    let normalized_time;
    let time =
        if allow_basic && time.len() >= 6 && time.as_bytes()[..6].iter().all(u8::is_ascii_digit) {
            normalized_time = format!(
                "{}:{}:{}{}",
                &time[..2],
                &time[2..4],
                &time[4..6],
                &time[6..]
            );
            normalized_time.as_str()
        } else {
            time
        };
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return false;
    }
    let digits = |text: &str| -> Option<u32> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse().ok()
    };
    let (Some(year), Some(month), Some(day)) =
        (digits(&date[..4]), digits(&date[5..7]), digits(&date[8..]))
    else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return false,
    };
    if day == 0
        || day > days
        || time.len() < 8
        || time.as_bytes()[2] != b':'
        || time.as_bytes()[5] != b':'
        || !time.as_bytes()[6..8].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let (Some(hour), Some(minute), Some(second)) =
        (digits(&time[..2]), digits(&time[3..5]), digits(&time[6..8]))
    else {
        return false;
    };
    if hour > 23 || minute > 59 || second > 60 {
        return false;
    }
    let mut rest = &time[8..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let count = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if count == 0 {
            return false;
        }
        rest = &fraction[count..];
    }
    if rest.is_empty() {
        return !require_timezone;
    }
    if rest == "Z" || rest == "z" {
        return true;
    }
    if allow_basic && rest.len() == 5 && matches!(rest.as_bytes()[0], b'+' | b'-') {
        return matches!((digits(&rest[1..3]),digits(&rest[3..5])),(Some(hour),Some(minute)) if hour<=23 && minute<=59);
    }
    if rest.len() != 6 || !matches!(rest.as_bytes()[0], b'+' | b'-') || rest.as_bytes()[3] != b':' {
        return false;
    }
    matches!((digits(&rest[1..3]), digits(&rest[4..6])), (Some(hour),Some(minute)) if hour <= 23 && minute <= 59)
}

fn valid_unpadded_date(value: &str) -> bool {
    let fields: Vec<_> = value.split('-').collect();
    if fields.len() != 3
        || fields[0].len() != 4
        || !(1..=2).contains(&fields[1].len())
        || !(1..=2).contains(&fields[2].len())
        || fields
            .iter()
            .any(|field| !field.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let normalized = format!(
        "{}-{:0>2}-{:0>2}T00:00:00Z",
        fields[0], fields[1], fields[2]
    );
    valid_datetime(&normalized, true, false, false)
}

pub(crate) fn contains_macro(value: &str) -> bool {
    if !value
        .as_bytes()
        .iter()
        .any(|&byte| matches!(byte, b'$' | b'[' | b'{' | b'%' | b'!'))
    {
        return false;
    }
    !detect_macro_spans(value).is_empty()
}

/// A parameter as it appears in the raw artifact, with byte offsets preserved so
/// findings can point at the exact span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawParam<'a> {
    pub(crate) name: Cow<'a, str>,
    pub(crate) value: Cow<'a, str>,
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
    family_parent: Option<String>,
    member_name: Option<String>,
    json_kind: Option<JsonValueKind>,
}

impl<'a> RawParam<'a> {
    /// A query parameter, the common case.
    pub(crate) fn query(
        name: impl Into<Cow<'a, str>>,
        value: impl Into<Cow<'a, str>>,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            start,
            end,
            component: ViolationTargetComponent::QueryParam,
            location: None,
            container: false,
            missing: false,
            family_parent: None,
            member_name: None,
            json_kind: None,
        }
    }

    pub(crate) fn target(&self) -> ViolationTarget {
        ViolationTarget {
            component: self.component,
            name: Some(
                self.location
                    .as_deref()
                    .unwrap_or(self.name.as_ref())
                    .to_string(),
            ),
            value: Some(self.value.as_ref().to_string()),
            start: self.start,
            end: self.end,
        }
    }
}

/// The concrete scopes a body spec evaluates over: one per element of the batch
/// array, or a single empty scope covering the whole document when the endpoint
/// takes one event per request.
fn body_scopes(document: &JsonDocument<'_>, scope: Option<&ScopeSpec>) -> Vec<String> {
    let Some(scope) = scope else {
        return vec![String::new()];
    };

    let patterns = scope.patterns();
    for pattern in &patterns {
        if pattern.is_empty() {
            return vec![String::new()];
        }

        // `expand` keeps a missing final key so a presence contract can report
        // it. Alternative envelopes must not treat that ghost path as a hit:
        // Adobe collect posts `events[]`, and `event` is the interact envelope.
        let named_array_present = pattern.strip_suffix("[]").is_some_and(|parent| {
            document
                .expand(parent)
                .iter()
                .filter_map(|path| document.get(path))
                .any(|field| {
                    // A root array alternative distinguishes [] from an object.
                    // A named envelope, even malformed, must not fall back to the
                    // single-event shape and report spurious missing event fields.
                    field.kind == JsonValueKind::Array
                        || (!parent.is_empty() && !patterns.contains(&parent))
                })
        });
        if document.matches_pattern(pattern) || named_array_present {
            return document
                .expand(pattern)
                .into_iter()
                .filter(|path| document.contains(path))
                .collect();
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
type BodyParamEntry<'a> = (Cow<'a, str>, String, Option<String>, Option<String>);

fn collect_body_params<'a>(
    document: &'a JsonDocument<'_>,
    scope: &str,
    contracts: &'a [CompiledParam],
) -> Vec<RawParam<'a>> {
    let mut params = Vec::new();

    for compiled in contracts {
        let entries: Vec<BodyParamEntry<'a>> = match &compiled.contract.name_pattern_parent {
            Some(parent) => {
                let parent_path = match (scope.is_empty(), parent.is_empty()) {
                    (true, _) => parent.clone(),
                    (false, true) => scope.to_string(),
                    (false, false) => format!("{scope}.{parent}"),
                };
                document
                    .expand(&parent_path)
                    .iter()
                    .flat_map(|path| document.members(path))
                    .map(|(key, path)| {
                        (
                            Cow::Owned(json::member_path(parent, &key)),
                            path,
                            Some(parent.clone()),
                            Some(key),
                        )
                    })
                    .collect()
            }
            None => compiled
                .names
                .iter()
                .flat_map(|name| {
                    let pattern = if let Some(root_path) = &compiled.contract.root_path {
                        root_path.clone()
                    } else if let Some(ancestor) = &compiled.contract.ancestor_path {
                        let mut prefix = Some(scope);
                        for _ in 0..ancestor.levels {
                            prefix = prefix.and_then(json::parent_path);
                        }
                        let Some(prefix) = prefix else {
                            return Vec::new();
                        };
                        match (prefix.is_empty(), ancestor.path.is_empty()) {
                            (true, _) => ancestor.path.clone(),
                            (false, true) => prefix.to_string(),
                            (false, false) => format!("{prefix}.{}", ancestor.path),
                        }
                    } else {
                        match scope.is_empty() {
                            true => name.to_string(),
                            false if name.is_empty() => scope.to_string(),
                            false => format!("{scope}.{name}"),
                        }
                    };
                    document
                        .expand(&pattern)
                        .into_iter()
                        .map(move |path| {
                            (
                                Cow::Borrowed(compiled.contract.name.as_str()),
                                path,
                                None,
                                None,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        };
        let alias_key = |path: &str| {
            let relative = if scope.is_empty() {
                path
            } else {
                let relative = path.strip_prefix(scope)?;
                relative.strip_prefix('.').unwrap_or(relative)
            };
            compiled
                .names
                .iter()
                .find_map(|name| json::alias_slot_key(name, relative))
        };
        let present_alias_slots: BTreeSet<_> = if compiled.contract.aliases.is_empty() {
            BTreeSet::new()
        } else {
            entries
                .iter()
                .filter_map(|(_, path, _, _)| {
                    let field = document.get(path)?;
                    if compiled.contract.null_as_missing && field.kind == JsonValueKind::Null {
                        return None;
                    }
                    alias_key(path)
                })
                .collect()
        };
        let mut missing_alias_slots = BTreeSet::new();
        for (name, path, family_parent, member_name) in entries {
            let missing = document.get(&path).is_none_or(|field| {
                compiled.contract.null_as_missing && field.kind == JsonValueKind::Null
            });
            if missing
                && !compiled.contract.aliases.is_empty()
                && let Some(key) = alias_key(&path)
                && (present_alias_slots.contains(&key) || !missing_alias_slots.insert(key))
            {
                continue;
            }
            let Some(field) = document.get(&path) else {
                // The slot is addressable but empty. Recording it lets the
                // contract report once per place the value belongs.
                let (start, end) = match document.nearest_present_ancestor(&path) {
                    Some((_, field)) => (field.start, field.end),
                    None => (0, 0),
                };

                params.push(RawParam {
                    name: name.clone(),
                    value: Cow::Borrowed(""),
                    start,
                    end,
                    component: ViolationTargetComponent::BodyField,
                    location: Some(path),
                    container: false,
                    missing: true,
                    family_parent,
                    member_name,
                    json_kind: None,
                });
                continue;
            };

            // Containers have no text of their own. Standing in the label
            // keeps a populated object from being reported as empty, while
            // an empty one still is.
            let value = match (field.kind, field.is_blank()) {
                (_, true) => Cow::Borrowed(""),
                (JsonValueKind::Object | JsonValueKind::Array, false) => {
                    Cow::Borrowed(field.kind.label())
                }
                _ => Cow::Borrowed(field.text.as_ref()),
            };

            params.push(RawParam {
                name: name.clone(),
                value,
                start: field.start,
                end: field.end,
                component: ViolationTargetComponent::BodyField,
                location: Some(path),
                container: matches!(field.kind, JsonValueKind::Object | JsonValueKind::Array),
                missing: compiled.contract.null_as_missing && field.kind == JsonValueKind::Null,
                family_parent,
                member_name,
                json_kind: Some(field.kind),
            });
        }
    }

    let mut seen = BTreeSet::new();
    params.retain(|param| seen.insert((param.name.to_string(), param.location.clone())));
    params
}

/// Where to point when a body finding has no field of its own to blame: the
/// enclosing event if there is one, otherwise the whole payload.
fn body_target(document: &JsonDocument<'_>, scope: &str, artifact: &str) -> ViolationTarget {
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

/// Splits the raw artifact into parameters, keeping byte offsets into the
/// original string. Query style reads `?a=1&b=2`; query-semicolon also splits
/// on `;`, the shape Adform uses; matrix style reads the semicolon-delimited
/// pairs Floodlight puts in the path; colon-path style reads `/key:value/`
/// segments, including Partnerize basket `[...]` groups.
pub(crate) fn extract_params(artifact: &str, style: ParamStyle) -> Vec<RawParam<'_>> {
    if style == ParamStyle::ColonPath {
        return extract_colon_path_params(artifact);
    }

    let length = artifact.len();
    let fragment_start = artifact.find('#').unwrap_or(length);
    let query_start = artifact[..fragment_start].find('?');

    let (region_start, region_end) = match style {
        ParamStyle::Query | ParamStyle::QuerySemicolon => match query_start {
            Some(start) => (start + 1, fragment_start),
            None => return Vec::new(),
        },
        ParamStyle::Matrix => {
            let (start, end) = path_span(artifact);
            // Matrix pairs are `name=value`. A query pixel's path has no `=`,
            // so skip walking it on every validate.
            if start >= end || !artifact.as_bytes()[start..end].contains(&b'=') {
                return Vec::new();
            }
            (start, end)
        }
        ParamStyle::ColonPath => unreachable!("colon_path returns above"),
    };

    if region_start >= region_end {
        return Vec::new();
    }

    let region = &artifact[region_start..region_end];
    let mut params = Vec::with_capacity(param_pair_capacity(region, style));
    let mut cursor = region_start;

    while cursor <= region_end {
        let segment_end = next_pair_break(&artifact[cursor..region_end], style)
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
                decode_param_value(value),
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

fn param_pair_capacity(region: &str, style: ParamStyle) -> usize {
    let bytes = region.as_bytes();
    let separators = match style {
        ParamStyle::Query => bytes.iter().filter(|&&byte| byte == b'&').count(),
        ParamStyle::Matrix => bytes.iter().filter(|&&byte| byte == b';').count(),
        ParamStyle::QuerySemicolon => bytes
            .iter()
            .filter(|&&byte| byte == b'&' || byte == b';')
            .count(),
        ParamStyle::ColonPath => 0,
    };
    separators + 1
}

fn next_pair_break(haystack: &str, style: ParamStyle) -> Option<usize> {
    match style {
        ParamStyle::Query => haystack.find('&'),
        ParamStyle::Matrix => haystack.find(';'),
        ParamStyle::QuerySemicolon => match (haystack.find('&'), haystack.find(';')) {
            (Some(ampersand), Some(semicolon)) => Some(ampersand.min(semicolon)),
            (Some(ampersand), None) => Some(ampersand),
            (None, Some(semicolon)) => Some(semicolon),
            (None, None) => None,
        },
        ParamStyle::ColonPath => None,
    }
}

/// Partnerize-style `/campaign:ID/clickref:ABC/[category:DVD/quantity:1]` paths.
fn extract_colon_path_params(artifact: &str) -> Vec<RawParam<'_>> {
    let (path_start, path_end) = path_span(artifact);
    if path_start >= path_end {
        return Vec::new();
    }

    let path = &artifact[path_start..path_end];
    // Only outer basket delimiters are transport syntax. A closing bracket in
    // sku:[SKU] is part of that macro, even in a middle path segment.
    let mut delimiters = BTreeSet::new();
    let mut basket_depth = 0usize;
    for (index, byte) in path.bytes().enumerate() {
        match byte {
            b'[' if basket_depth > 0 => basket_depth += 1,
            b'[' if index == 0 || path.as_bytes()[index - 1] == b'/' => {
                basket_depth = 1;
                delimiters.insert(index);
            }
            b']' if basket_depth > 0 => {
                basket_depth -= 1;
                if basket_depth == 0 {
                    delimiters.insert(index);
                }
            }
            _ => {}
        }
    }
    let mut params = Vec::with_capacity(path.bytes().filter(|&byte| byte == b'/').count());
    let mut rel = 0;

    for segment in path.split('/') {
        if !segment.is_empty() {
            let leading = usize::from(delimiters.contains(&rel) && segment.starts_with('['));
            let trailing = usize::from(
                delimiters.contains(&(rel + segment.len() - 1))
                    && segment.ends_with(']')
                    && segment.len() > leading,
            );
            let inner_end = segment.len() - trailing;
            if leading < inner_end {
                let inner = &segment[leading..inner_end];
                if let Some((name, value)) = inner.split_once(':')
                    && !name.is_empty()
                {
                    let abs = path_start + rel;
                    params.push(RawParam::query(
                        percent_decode(name),
                        decode_param_value(value),
                        abs + leading,
                        abs + inner_end,
                    ));
                }
            }
        }
        rel += segment.len() + 1;
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

/// Percent-decode a query value, unless the raw text is already a macro.
///
/// `%%CACHEBUSTER%%` and `[%ADID%]` contain `%` plus two hex digits. Decoding
/// that pair deletes the token, and the privacy skip then treats the leftover
/// as a literal flag. `%20` is not a macro, so it still decodes.
fn decode_param_value(value: &str) -> Cow<'_, str> {
    if contains_macro(value) {
        Cow::Borrowed(value)
    } else {
        percent_decode(value)
    }
}

/// Minimal percent-decoding for parameter names and values. `+` is decoded as a
/// space because form-encoded pixel payloads are common.
fn percent_decode(value: &str) -> Cow<'_, str> {
    if !value
        .as_bytes()
        .iter()
        .any(|&byte| byte == b'%' || byte == b'+')
    {
        return Cow::Borrowed(value);
    }

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

    match String::from_utf8(decoded) {
        Ok(text) => Cow::Owned(text),
        Err(error) => Cow::Owned(String::from_utf8_lossy(&error.into_bytes()).into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ExpansionState, ValidationOptions};
    use std::borrow::Cow;

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
    fn query_semicolon_params_split_on_ampersand_and_semicolon() {
        let manifest = TEST_MANIFEST.replace(
            r#""match": {"#,
            r#""param_style": "query_semicolon", "match": {"#,
        );
        let pack =
            ManifestRulePack::from_json(&manifest).expect("compile query_semicolon manifest");

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=12345;ev=PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=12345&ev=PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let artifact = "https://px.example.com/collect?id=12345;ev=Purchase;legacy=1";
        let report = pack.validate(&request(artifact));
        assert_eq!(codes(&report), vec!["vendor.test.param.legacy.deprecated"]);
        let target = &report.violations[0].targets[0];
        assert_eq!(&artifact[target.start..target.end], "legacy=1");
    }

    #[test]
    fn colon_path_params_are_read_from_the_path() {
        let manifest = TEST_MANIFEST
            .replace(
                r#""match": {"#,
                r#""param_style": "colon_path", "match": {"#,
            )
            .replace(
                r#""path_prefixes": ["/collect"]"#,
                r#""path_prefixes": ["/conversion"]"#,
            );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile colon_path manifest");
        let report = pack.validate(&request(
            "https://px.example.com/conversion/id:12345/ev:PageView",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let report = pack.validate(&request("https://px.example.com/conversion/id:12345"));
        assert_eq!(codes(&report), vec!["vendor.test.param.ev.missing"]);
    }

    #[test]
    fn colon_path_strips_basket_brackets() {
        let manifest = TEST_MANIFEST
            .replace(
                r#""match": {"#,
                r#""param_style": "colon_path", "match": {"#,
            )
            .replace(
                r#""path_prefixes": ["/collect"]"#,
                r#""path_prefixes": ["/conversion"]"#,
            );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile colon_path manifest");
        let artifact =
            "https://px.example.com/conversion/[id:12345]/ev:Purchase/[category:SHOES/quantity:1]";
        let report = pack.validate(&request(artifact));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let artifact = "https://px.example.com/conversion/[id:ab]/ev:PageView";
        let report = pack.validate(&request(artifact));
        assert_eq!(codes(&report), vec!["vendor.test.param.id.invalid"]);
        let target = &report.violations[0].targets[0];
        assert_eq!(&artifact[target.start..target.end], "id:ab");
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
    fn allow_empty_skips_blank_template_slots() {
        let manifest = TEST_MANIFEST.replace(
            r#""name": "url",
                "format": { "kind": "url", "require_https": true }"#,
            r#""name": "url",
                "format": { "kind": "url", "require_https": true },
                "allow_empty": true"#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile manifest");

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView&url=",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView&url=not-a-url",
        ));
        assert_eq!(codes(&report), vec!["vendor.test.param.url.invalid"]);
    }

    #[test]
    fn name_pattern_checks_present_keys_and_uses_the_matched_name() {
        let manifest = TEST_MANIFEST.replace(
            r#"{
                "name": "url",
                "format": { "kind": "url", "require_https": true }
            }"#,
            r#"{
                "name": "u1",
                "name_pattern": "^u([1-9]|[1-9][0-9]|100)$",
                "format": { "kind": "non_empty" }
            }"#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile name_pattern");

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView&u21=",
        ));
        assert_eq!(codes(&report), vec!["vendor.test.param.u21.empty"]);

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView&u21=filled",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn name_pattern_cannot_be_required() {
        let manifest = TEST_MANIFEST.replace(
            r#"{
                "name": "url",
                "format": { "kind": "url", "require_https": true }
            }"#,
            r#"{
                "name": "u1",
                "name_pattern": "^u[0-9]+$",
                "requirement": "required"
            }"#,
        );
        let error = ManifestRulePack::from_json(&manifest).expect_err("required glob is rejected");
        assert!(
            matches!(error, ManifestError::NamePatternNotOptional { .. }),
            "{error}"
        );
    }

    #[test]
    fn name_pattern_cannot_declare_aliases() {
        let manifest = TEST_MANIFEST.replace(
            r#"{
                "name": "url",
                "format": { "kind": "url", "require_https": true }
            }"#,
            r#"{
                "name": "u1",
                "name_pattern": "^u[0-9]+$",
                "aliases": ["u"]
            }"#,
        );
        let error = ManifestRulePack::from_json(&manifest).expect_err("aliases on a glob");
        assert!(
            matches!(error, ManifestError::NamePatternWithAliases { .. }),
            "{error}"
        );
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
    fn value_when_checks_a_present_field_against_the_required_value() {
        let manifest = TEST_MANIFEST.replace(
            r#"        "rules": ["#,
            r#"        "rules": [
            {
                "code": "vendor.test.purchase_url_must_be_https_shop",
                "kind": "value_when",
                "when": "ev",
                "equals": ["Purchase"],
                "param": "url",
                "value": "https://shop.example/thanks",
                "severity": "error",
                "message": "A purchase must send the documented shop URL."
            },"#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile manifest");

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase&url=https://other.example/",
        ));
        assert_eq!(
            codes(&report),
            vec!["vendor.test.purchase_url_must_be_https_shop"]
        );

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase&url=https://shop.example/thanks",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        // Presence is a different contract. Missing `url` does not fire this.
        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView&url=https://other.example/",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));
    }

    #[test]
    fn forbidden_when_value_rejects_a_present_field() {
        let manifest = TEST_MANIFEST.replace(
            r#"        "rules": ["#,
            r#"        "rules": [
            {
                "code": "vendor.test.purchase_forbids_url",
                "kind": "forbidden_when_value",
                "when": "ev",
                "equals": ["Purchase"],
                "params": ["url"],
                "severity": "error",
                "message": "A purchase must not send a landing URL."
            },"#,
        );
        let pack = ManifestRulePack::from_json(&manifest).expect("compile manifest");

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase&url=https://shop.example/thanks",
        ));
        assert_eq!(codes(&report), vec!["vendor.test.purchase_forbids_url"]);

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=Purchase",
        ));
        assert!(codes(&report).is_empty(), "{:?}", codes(&report));

        let report = pack.validate(&request(
            "https://px.example.com/collect?id=123&ev=PageView&url=https://shop.example/thanks",
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
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode(""), "");
        assert_eq!(percent_decode("%2Fpath%2F"), "/path/");
        assert_eq!(percent_decode("%2f"), "/");
        assert_eq!(percent_decode("%GG"), "%GG");
        assert_eq!(percent_decode("%2"), "%2");
        assert_eq!(percent_decode("100% done"), "100% done");
        assert_eq!(percent_decode("a%"), "a%");
        assert_eq!(percent_decode("caf%C3%A9"), "café");
        assert_eq!(percent_decode("%FF"), "\u{FFFD}");
        assert!(matches!(percent_decode("plain"), Cow::Borrowed("plain")));
        assert!(matches!(percent_decode(""), Cow::Borrowed("")));
    }

    #[test]
    fn typical_query_params_borrow_the_artifact() {
        let params = extract_params(
            "https://example.com/pixel?id=12345&ev=PageView",
            ParamStyle::Query,
        );
        assert_eq!(params.len(), 2);
        assert!(matches!(params[0].name, Cow::Borrowed("id")));
        assert!(matches!(params[0].value, Cow::Borrowed("12345")));
        assert!(matches!(params[1].name, Cow::Borrowed("ev")));
        assert!(matches!(params[1].value, Cow::Borrowed("PageView")));

        let decoded = extract_params("https://example.com/pixel?em=a%40b.com", ParamStyle::Query);
        assert!(matches!(decoded[0].value, Cow::Owned(_)));
        assert_eq!(decoded[0].value.as_ref(), "a@b.com");

        let matrix = extract_params(
            "https://www.facebook.com/tr?id=1&ev=PageView",
            ParamStyle::Matrix,
        );
        assert!(matrix.is_empty());
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
    fn alternative_scopes_skip_an_absent_first_envelope() {
        let manifest = r#"{
            "id": "vendor/alt",
            "display_name": "Alt",
            "description": "Tries interact then collect.",
            "docs": "https://example.com/docs",
            "match": {
                "hosts": ["api.example.com"],
                "json_paths": [{"any_of": ["event.x", "events[].x"]}]
            },
            "body": {
                "scope": ["event", "events[]"],
                "params": [
                    { "name": "x", "requirement": "required", "format": { "kind": "non_empty" } }
                ]
            }
        }"#;
        let pack = ManifestRulePack::from_json(manifest).expect("compiles");
        let report = pack.validate(&body_request(r#"{"events":[{"x":""}]}"#));

        assert_eq!(codes(&report), vec!["vendor.alt.body.x.empty"]);
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
    fn a_pack_can_contract_the_envelope_and_the_events_inside_it() {
        let manifest = r#"{
            "id": "vendor/two-level",
            "display_name": "Two Level",
            "description": "Contracts the envelope and each event.",
            "docs": "https://example.com/docs",
            "match": { "hosts": ["api.example.com"], "json_paths": ["events[].name"] },
            "body": [
                { "params": [{ "name": "client_id", "requirement": "required" }] },
                {
                    "scope": "events[]",
                    "params": [{ "name": "name", "requirement": "required" }]
                }
            ]
        }"#;

        let pack = ManifestRulePack::from_json(manifest).expect("compiles");
        let report = pack.validate(&body_request(r#"{"events":[{"name":"a"},{"id":1}]}"#));

        // The envelope is checked once and each event on its own.
        assert_eq!(
            codes(&report),
            vec![
                "vendor.two-level.body.client_id.missing",
                "vendor.two-level.body.name.missing",
            ]
        );
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
