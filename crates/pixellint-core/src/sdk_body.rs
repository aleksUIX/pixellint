//! Private, producer-specific preparation for selected built-in SDK endpoints.
use crate::json::{BoundedJsonError, JsonDocument, JsonField, JsonLimits, JsonValueKind};
use crate::manifest::{RawParam, decode_base64_bytes};
use crate::{
    PreparedArtifact, RuleSource, RuleSourceLevel, Severity, Violation, ViolationTargetComponent,
};
use serde_json::Value;

const TEXT: usize = 16 * 1024 * 1024;
const WORK: usize = 128_000_000;

#[derive(Clone)]
pub(crate) struct Profile {
    path: &'static str,
    projections: Vec<Projection>,
}
#[derive(Clone)]
struct Projection {
    encoding: String,
    scopes: Vec<String>,
    fields: Vec<FieldProjection>,
    rules: Vec<RuleProjection>,
}

#[derive(Clone)]
struct RuleProjection {
    contract: Value,
    matcher: Option<regex::Regex>,
    finding_bytes: usize,
}
#[derive(Clone)]
struct FieldProjection {
    contract: Value,
    names: Vec<String>,
    parent: Option<String>,
    matcher: Option<regex::Regex>,
    finding_bytes: usize,
}

impl Profile {
    pub(crate) fn builtin(id: &str, manifest: &str) -> Option<Self> {
        let path = match id {
            "vendor/mixpanel" => "/track",
            "vendor/mixpanel-engage" => "/engage",
            "vendor/mixpanel-groups" => "/groups",
            _ => return None,
        };
        let manifest: Value = serde_json::from_str(manifest).expect("compiled builtin manifest");
        let projections = manifest["body"]
            .as_array()
            .expect("builtin body specs")
            .iter()
            .filter(|body| body["source_param"] == "data")
            .map(|body| Projection {
                encoding: body["encoding"]
                    .as_str()
                    .expect("builtin data encoding")
                    .into(),
                scopes: match &body["scope"] {
                    Value::String(scope) => vec![scope.clone()],
                    Value::Array(scopes) => scopes
                        .iter()
                        .map(|s| s.as_str().expect("builtin scope").to_owned())
                        .collect(),
                    _ => vec![String::new()],
                },
                fields: body["params"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|contract| {
                        let mut names =
                            vec![contract["name"].as_str().unwrap_or_default().to_owned()];
                        names.extend(
                            contract["aliases"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str)
                                .map(str::to_owned),
                        );
                        FieldProjection {
                            contract: contract.clone(),
                            names,
                            parent: contract["name_pattern_parent"].as_str().map(str::to_owned),
                            matcher: contract["name_pattern"].as_str().map(|pattern| {
                                regex::Regex::new(pattern).expect("builtin name pattern")
                            }),
                            // Includes all contract strings, enum alternatives, fixes and references.
                            // Remaining bounded syntax/type/size reasons fit the fixed reserve.
                            finding_bytes: 2
                                * (contract.to_string().len()
                                    + manifest["display_name"].as_str().map_or(0, str::len)
                                    + id.len()
                                    + manifest["docs"].as_str().map_or(0, str::len))
                                + 512,
                        }
                    })
                    .collect(),
                rules: body["rules"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|rule| RuleProjection {
                        contract: rule.clone(),
                        matcher: rule["pattern"].as_str().map(|pattern| {
                            regex::Regex::new(pattern).expect("builtin rule pattern")
                        }),
                        finding_bytes: 2
                            * (rule.to_string().len()
                                + manifest["display_name"].as_str().map_or(0, str::len)
                                + id.len()
                                + manifest["docs"].as_str().map_or(0, str::len))
                            + 512,
                    })
                    .collect(),
            })
            .collect();
        Some(Self { path, projections })
    }

    pub(crate) fn endpoint(&self, method: &str, endpoint: &PreparedArtifact<'_>) -> bool {
        method == "POST"
            && endpoint.url().is_some_and(|url| {
                url.path == self.path || url.path.strip_suffix('/') == Some(self.path)
            })
    }

    pub(crate) fn prepare(
        &self,
        entity: &str,
        content_type: &str,
        query_data: bool,
    ) -> Preparation {
        if entity.len() > TEXT {
            return Preparation::Deferred(Problem::Limit);
        }
        if !utf8_mime(content_type) {
            return Preparation::Deferred(Problem::Unvalidated);
        }
        let Some((name, encoded)) = entity.split_once('=') else {
            return Preparation::Deferred(Problem::Unvalidated);
        };
        if entity.contains('&') {
            return Preparation::Deferred(Problem::Unvalidated);
        }
        let Some(name) = form_decode(name) else {
            return Preparation::Deferred(Problem::Unvalidated);
        };
        if name != "data" {
            return Preparation::Deferred(Problem::Unvalidated);
        }
        let Some(data) = form_decode(encoded) else {
            return Preparation::Deferred(Problem::Unvalidated);
        };
        if query_data {
            return Preparation::Deferred(Problem::Unvalidated);
        }
        if data.len() > TEXT {
            return Preparation::Deferred(Problem::Limit);
        }
        let raw_json = data.trim_start().starts_with(['{', '[']);
        let decoded = if raw_json {
            Some(data.clone())
        } else {
            match decode_base64_bytes(&data) {
                Ok(bytes) if bytes.len() > TEXT => return Preparation::Deferred(Problem::Limit),
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => Some(text),
                    Err(error) => {
                        if let Some(text) = paired_cesu8(error.as_bytes()) {
                            return match self.inspect(&text, false) {
                                Err(Problem::Limit) => Preparation::Deferred(Problem::Limit),
                                Ok(_) => Preparation::Deferred(Problem::Unvalidated),
                                _ => Preparation::Data(PreparedData {
                                    data,
                                    secrets: Vec::new(),
                                }),
                            };
                        }
                        None
                    }
                },
                Err(_) => None,
            }
        };
        let secrets = match decoded
            .as_deref()
            .map(|decoded| self.inspect(decoded, raw_json))
        {
            Some(Err(Problem::Limit)) => return Preparation::Deferred(Problem::Limit),
            Some(Ok(secrets)) => secrets,
            _ => Vec::new(),
        };
        // Existing encoded-data contracts retain malformed JSON/Base64 errors.
        Preparation::Data(PreparedData { data, secrets })
    }

    fn inspect(&self, json: &str, raw_json: bool) -> Result<Vec<String>, Problem> {
        if json.len() > TEXT {
            return Err(Problem::Limit);
        }
        let document = JsonDocument::parse_bounded(
            json,
            JsonLimits {
                nodes: 100_000,
                paths: TEXT,
                depth: 64,
                root_records: 2500,
            },
        )
        .map_err(|error| match error {
            BoundedJsonError::Limit => Problem::Limit,
            BoundedJsonError::Invalid => Problem::Invalid,
        })?;
        let mut budget = ProjectionBudget {
            text: 0,
            work: 0,
            finding_text: 0,
        };
        budget.charge(0, json.len().checked_mul(2).ok_or(Problem::Limit)?)?;
        for spec in &self.projections {
            if (spec.encoding == "json") != raw_json {
                continue;
            }
            for pattern in &spec.scopes {
                // Each expansion is a subset of the bounded concrete path set.
                // Charge each generated scope and possible finding before vendor evaluation.
                let (paths, expansion_work) = document.expand_with_work(pattern);
                budget.charge(0, expansion_work.checked_mul(2).ok_or(Problem::Limit)?)?;
                for path in paths {
                    budget.charge(path.len(), 1)?;
                    let mut scope_name_bytes = 0usize;
                    let mut scope_slots = 0usize;
                    for field in &spec.fields {
                        let mut slots = Vec::new();
                        if let Some(parent) = &field.parent {
                            let parent_pattern = relative(&path, parent);
                            let (parents, work) = document.expand_with_work(&parent_pattern);
                            budget.charge(
                                parent_pattern.len(),
                                work.checked_mul(2).ok_or(Problem::Limit)?,
                            )?;
                            for parent_path in parents {
                                for (key, value_path) in document.members(&parent_path) {
                                    let logical = crate::json::member_path(parent, &key);
                                    // The real collector retains every member before name filtering.
                                    budget.charge(
                                        checked_sum(&[
                                            logical.len(),
                                            value_path.len(),
                                            key.len(),
                                            parent.len(),
                                        ])?,
                                        key.len().checked_add(1).ok_or(Problem::Limit)?,
                                    )?;
                                    scope_name_bytes = scope_name_bytes
                                        .checked_add(logical.len())
                                        .ok_or(Problem::Limit)?;
                                    scope_slots =
                                        scope_slots.checked_add(1).ok_or(Problem::Limit)?;
                                    if field
                                        .matcher
                                        .as_ref()
                                        .is_none_or(|regex| regex.is_match(&key))
                                    {
                                        slots.push((logical, value_path));
                                    }
                                }
                            }
                        } else {
                            for name in &field.names {
                                let pattern = field.contract["root_path"]
                                    .as_str()
                                    .map_or_else(|| relative(&path, name), str::to_owned);
                                let (values, work) = document.expand_with_work(&pattern);
                                budget.charge(
                                    pattern.len(),
                                    work.checked_mul(2).ok_or(Problem::Limit)?,
                                )?;
                                for value_path in values {
                                    budget.charge(
                                        checked_sum(&[
                                            name.len(),
                                            value_path.len(),
                                            value_path.len(),
                                        ])?,
                                        1,
                                    )?;
                                    scope_name_bytes = scope_name_bytes
                                        .checked_add(name.len())
                                        .ok_or(Problem::Limit)?;
                                    scope_slots =
                                        scope_slots.checked_add(1).ok_or(Problem::Limit)?;
                                    slots.push((name.clone(), value_path));
                                }
                            }
                        }
                        for (logical, value_path) in slots {
                            let value = document.get(&value_path);
                            let value_bytes = value.map_or(0, |value| value.text.len());
                            let work = value_bytes
                                .checked_mul(8)
                                .and_then(|n| n.checked_add(value_path.len()))
                                .and_then(|n| n.checked_add(8))
                                .ok_or(Problem::Limit)?;
                            budget.charge(0, work)?;
                            if field.contract.get("max_depth").is_some() {
                                budget.charge(
                                    0,
                                    document
                                        .subtree_candidate_count(&value_path)
                                        .checked_mul(3)
                                        .ok_or(Problem::Limit)?,
                                )?;
                            }
                            if !proven_valid(&field.contract, value, &document, &value_path) {
                                // A finding retains logical/code names, a remapped field path,
                                // message/fix/source strings and one original data target.
                                let dynamic = checked_sum(&[
                                    logical.len(),
                                    logical.len(),
                                    logical.len(),
                                    value_path.len(),
                                    path.len(),
                                    value_bytes,
                                    value_bytes,
                                ])?;
                                budget.finding(
                                    field
                                        .finding_bytes
                                        .checked_add(dynamic)
                                        .ok_or(Problem::Limit)?,
                                    1,
                                )?;
                            }
                        }
                    }
                    // Each contract addresses the collected scope slots. Account for
                    // these scans with actual generated names, never all-map products.
                    budget.charge(
                        0,
                        scope_name_bytes
                            .checked_add(scope_slots)
                            .and_then(|n| n.checked_mul(spec.fields.len()))
                            .ok_or(Problem::Limit)?,
                    )?;
                    for rule in &spec.rules {
                        let mut present = Vec::new();
                        let mut rule_path_bytes = 0usize;
                        for name in rule.contract["params"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                        {
                            let pattern = relative(&path, name);
                            let (values, work) = document.expand_with_work(&pattern);
                            budget.charge(
                                pattern.len(),
                                work.checked_mul(2).ok_or(Problem::Limit)?,
                            )?;
                            for value_path in values {
                                rule_path_bytes = rule_path_bytes
                                    .checked_add(value_path.len())
                                    .ok_or(Problem::Limit)?;
                                budget.charge(value_path.len(), 1)?;
                                if let Some(value) = document.get(&value_path) {
                                    present.push(value);
                                }
                            }
                        }
                        let proven = match rule.contract["kind"].as_str() {
                            Some("forbid_value_pattern") => {
                                rule.matcher.as_ref().is_some_and(|regex| {
                                    present.iter().all(|field| !regex.is_match(&field.text))
                                })
                            }
                            Some("require_one_of") => present.iter().any(|field| !field.is_blank()),
                            _ => false,
                        };
                        if !proven {
                            let count = if rule.contract["kind"] == "require_one_of" {
                                1
                            } else {
                                present.len().max(1)
                            };
                            let text =
                                checked_sum(&[rule.finding_bytes, path.len(), rule_path_bytes])?
                                    .checked_mul(count)
                                    .ok_or(Problem::Limit)?;
                            budget.finding(
                                text,
                                scope_name_bytes.checked_add(16).ok_or(Problem::Limit)?,
                            )?;
                        }
                    }
                }
            }
        }
        let mut sensitive = Vec::new();
        for (path, field) in document.fields() {
            if matches!(
                field.kind,
                JsonValueKind::Object | JsonValueKind::Array | JsonValueKind::Null
            ) {
                continue;
            }
            budget.charge(0, path.len().checked_mul(8).ok_or(Problem::Limit)?)?;
            let lower = path.to_ascii_lowercase();
            if [
                "authorization",
                "cookie",
                "key",
                "token",
                "secret",
                "password",
            ]
            .iter()
            .any(|name| lower.contains(name))
            {
                sensitive.push(field.text.as_ref());
            }
        }
        let sensitive_bytes = sensitive.iter().try_fold(0usize, |sum, value| {
            sum.checked_add(value.len()).ok_or(Problem::Limit)
        })?;
        let sorting_factor = (usize::BITS - sensitive.len().max(1).leading_zeros()) as usize + 2;
        budget.charge(
            0,
            sensitive_bytes
                .checked_mul(sorting_factor)
                .ok_or(Problem::Limit)?,
        )?;
        let unique: std::collections::BTreeSet<_> = sensitive
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect();
        let unique_bytes = unique.iter().try_fold(0usize, |sum, value| {
            sum.checked_add(value.len()).ok_or(Problem::Limit)
        })?;
        let redaction_work = budget
            .finding_text
            .checked_mul(unique_bytes)
            .ok_or(Problem::Limit)?;
        let growth_factor = unique
            .iter()
            .map(|value| value.len())
            .min()
            .map_or(1, |minimum| 9 / minimum + 1);
        let redaction_growth = budget
            .finding_text
            .checked_mul(growth_factor - 1)
            .ok_or(Problem::Limit)?;
        budget.charge(
            redaction_growth
                .checked_add(unique_bytes)
                .ok_or(Problem::Limit)?,
            redaction_work,
        )?;
        let mut secrets: Vec<_> = unique.into_iter().map(str::to_owned).collect();
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        Ok(secrets)
    }
}

fn relative(scope: &str, name: &str) -> String {
    match (scope.is_empty(), name.is_empty()) {
        (true, _) => name.into(),
        (_, true) => scope.into(),
        _ => format!("{scope}.{name}"),
    }
}

/// Only omit projected findings when these current native contracts are proved
/// valid. Unknown features remain conservative and still run in the vendor pack.
fn proven_valid(
    contract: &Value,
    field: Option<&JsonField<'_>>,
    document: &JsonDocument<'_>,
    path: &str,
) -> bool {
    let Some(field) = field else {
        return contract["requirement"]
            .as_str()
            .is_none_or(|requirement| requirement == "optional");
    };
    if let Some(condition) = contract.get("condition") {
        if condition["kind"] == "json_type" && condition["param"] == "" {
            if !type_matches(&condition["json_type"], field) {
                return true;
            }
        } else {
            return false;
        }
    }
    let known = [
        "name",
        "aliases",
        "name_pattern",
        "name_pattern_parent",
        "description",
        "doc",
        "fix_hint",
        "source_level",
        "severity",
        "format_severity",
        "requirement",
        "json_type",
        "allow_empty",
        "max_items",
        "min_items",
        "max_properties",
        "min_properties",
        "max_byte_length",
        "max_depth",
        "condition",
        "format",
        "root_path",
    ];
    if contract
        .as_object()
        .is_none_or(|object| object.keys().any(|key| !known.contains(&key.as_str())))
    {
        return false;
    }
    if contract["requirement"]
        .as_str()
        .is_some_and(|requirement| ["forbidden", "deprecated"].contains(&requirement))
    {
        return false;
    }
    if contract
        .get("json_type")
        .is_some_and(|types| !type_matches(types, field))
    {
        return false;
    }
    for (kind, min, max) in [
        (JsonValueKind::Array, "min_items", "max_items"),
        (JsonValueKind::Object, "min_properties", "max_properties"),
    ] {
        if field.kind == kind
            && (contract[min]
                .as_u64()
                .is_some_and(|min| field.len.unwrap_or(0) < min as usize)
                || contract[max]
                    .as_u64()
                    .is_some_and(|max| field.len.unwrap_or(0) > max as usize))
        {
            return false;
        }
    }
    if field.kind == JsonValueKind::String
        && contract["max_byte_length"]
            .as_u64()
            .is_some_and(|max| field.text.len() > max as usize)
    {
        return false;
    }
    if matches!(field.kind, JsonValueKind::Object | JsonValueKind::Array)
        && contract["max_depth"].as_u64().is_some_and(|max| {
            document
                .subtree_depth(path)
                .is_some_and(|depth| depth > max as usize)
        })
    {
        return false;
    }
    if matches!(
        field.kind,
        JsonValueKind::Object | JsonValueKind::Array | JsonValueKind::Null
    ) && contract.get("json_type").is_some()
    {
        return field.kind == JsonValueKind::Null
            || field.len != Some(0)
            || contract["format"]["kind"] != "non_empty";
    }
    if field.text.is_empty() && contract["allow_empty"] != true {
        return false;
    }
    if contract["format"].as_object().is_some_and(|format| {
        format.keys().any(|key| {
            !["kind", "values", "case_insensitive", "require_https"].contains(&key.as_str())
        })
    }) {
        return false;
    }
    match contract["format"]["kind"].as_str() {
        None => true,
        Some("non_empty") => !field.text.trim().is_empty(),
        Some("integer") => {
            !field.text.is_empty() && field.text.bytes().all(|byte| byte.is_ascii_digit())
        }
        Some("enum") => contract["format"]["values"]
            .as_array()
            .is_some_and(|values| {
                values
                    .iter()
                    .any(|value| value.as_str() == Some(field.text.as_ref()))
            }),
        Some("url") => url::Url::parse(&field.text).is_ok_and(|url| {
            contract["format"]["require_https"] != true || url.scheme() == "https"
        }),
        _ => false,
    }
}
fn type_matches(types: &Value, field: &JsonField<'_>) -> bool {
    if let Some(types) = types.as_array() {
        return types.iter().any(|kind| type_matches(kind, field));
    }
    match types.as_str() {
        Some("object") => field.kind == JsonValueKind::Object,
        Some("array") => field.kind == JsonValueKind::Array,
        Some("string") => field.kind == JsonValueKind::String,
        Some("number") => field.kind == JsonValueKind::Number,
        Some("integer") => field.kind == JsonValueKind::Number && integer(&field.text),
        Some("boolean") => field.kind == JsonValueKind::Bool,
        Some("null") => field.kind == JsonValueKind::Null,
        _ => false,
    }
}
fn integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<i64>().is_ok()
}

fn checked_sum(values: &[usize]) -> Result<usize, Problem> {
    values.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(*value).ok_or(Problem::Limit)
    })
}
struct ProjectionBudget {
    text: usize,
    work: usize,
    finding_text: usize,
}
impl ProjectionBudget {
    fn finding(&mut self, text: usize, work: usize) -> Result<(), Problem> {
        self.finding_text = self.finding_text.checked_add(text).ok_or(Problem::Limit)?;
        self.charge(text, work)
    }
    fn charge(&mut self, text: usize, work: usize) -> Result<(), Problem> {
        self.text = self.text.checked_add(text).ok_or(Problem::Limit)?;
        self.work = self.work.checked_add(work).ok_or(Problem::Limit)?;
        if self.text > TEXT || self.work > WORK {
            Err(Problem::Limit)
        } else {
            Ok(())
        }
    }
}

pub(crate) struct PreparedData {
    pub data: String,
    pub secrets: Vec<String>,
}
pub(crate) enum Preparation {
    Data(PreparedData),
    Deferred(Problem),
}
#[derive(Clone, Copy)]
pub(crate) enum Problem {
    Unvalidated,
    Limit,
    Invalid,
}
impl Problem {
    pub(crate) fn finding(self) -> Violation {
        let (code, message) = match self {
            Self::Limit => (
                "sdk_body_limit",
                "The selected SDK body profile exceeded a local inspection bound. Body checks remain unvalidated; observed URL, query and HTTP checks still run.",
            ),
            _ => (
                "sdk_body_unvalidated",
                "The selected SDK body profile cannot safely interpret this capture's MIME, data envelope, query/body ambiguity or producer-specific text encoding. Body checks remain unvalidated; observed URL, query and HTTP checks still run.",
            ),
        };
        Violation {
            code: format!("core.request.{code}"), message: message.into(), severity: Severity::Info,
            field: Some("body".into()), fix_hint: None, targets: Vec::new(),
            source: RuleSource { level: RuleSourceLevel::Heuristic,
                name: "Pixellint local Mixpanel JavaScript SDK body profile".into(),
                reference: Some("https://github.com/mixpanel/mixpanel-js/blob/d6bdc7f80230a54f8ec470d344058d0a36a81aa2/src/mixpanel-core.js".into()) },
        }
    }
}
pub(crate) fn field(data: String) -> RawParam<'static> {
    let mut field = RawParam::query("data", data, 0, 0);
    field.component = ViolationTargetComponent::BodyField;
    field.omit_capture_target_value();
    field
}

/// Do not infer form encoding from arbitrary text. Only this exact MIME profile
/// supplies the selected owner's private data field.
fn utf8_mime(value: &str) -> bool {
    let mut parts = value.split(';');
    if !parts
        .next()
        .is_some_and(|part| part.trim().eq_ignore_ascii_case("text/plain"))
    {
        return false;
    }
    let mut charset = false;
    for part in parts {
        let Some((name, value)) = part.trim().split_once('=') else {
            return false;
        };
        if name.trim().eq_ignore_ascii_case("charset") {
            if charset {
                return false;
            }
            charset = true;
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            if !value.eq_ignore_ascii_case("utf-8") && !value.eq_ignore_ascii_case("utf8") {
                return false;
            }
        }
    }
    true
}
fn form_decode(value: &str) -> Option<String> {
    let mut output = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => output.push(b' '),
            b'%' => {
                let high = (bytes.next()? as char).to_digit(16)?;
                let low = (bytes.next()? as char).to_digit(16)?;
                output.push((high * 16 + low) as u8);
            }
            _ => output.push(byte),
        }
    }
    String::from_utf8(output).ok()
}

/// Recognize the pinned SDK's paired UTF-16 code-unit encoding. Conversion is
/// private proof of this exception, never input to vendor or generic checks.
fn paired_cesu8(bytes: &[u8]) -> Option<String> {
    let mut output = String::new();
    let mut index = 0;
    let mut found = false;
    while index < bytes.len() {
        let tail = &bytes[index..];
        if tail.len() >= 6
            && tail[0] == 0xed
            && (0xa0..=0xaf).contains(&tail[1])
            && (0x80..=0xbf).contains(&tail[2])
            && tail[3] == 0xed
            && (0xb0..=0xbf).contains(&tail[4])
            && (0x80..=0xbf).contains(&tail[5])
        {
            let high = 0xd000 + (((tail[1] & 63) as u32) << 6) + (tail[2] & 63) as u32;
            let low = 0xd000 + (((tail[4] & 63) as u32) << 6) + (tail[5] & 63) as u32;
            output.push(char::from_u32(
                0x10000 + ((high - 0xd800) << 10) + low - 0xdc00,
            )?);
            index += 6;
            found = true;
        } else {
            let width = match tail[0] {
                0..=127 => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => return None,
            };
            let text = std::str::from_utf8(tail.get(..width)?).ok()?;
            output.push_str(text);
            index += width;
        }
    }
    found.then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Profile {
        Profile::builtin(
            "vendor/mixpanel",
            include_str!("../rulepacks/vendor/mixpanel.json"),
        )
        .unwrap()
    }
    fn form(value: &str) -> String {
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("data", value)
            .finish()
    }
    #[test]
    fn sdk_percent_decoding_is_strict_and_runs_once() {
        let data = r#"{"event":"literal %2B and +","properties":{"token":"project"}}"#;
        let Preparation::Data(decoded) =
            profile().prepare(&form(data), "text/plain;charset=UTF-8", false)
        else {
            panic!("accepted source envelope");
        };
        assert_eq!(decoded.data, data);
        for invalid in [
            "data=%",
            "data=%GG",
            "data=%FF",
            "extra=1",
            "data=a&data=b",
            "data=a&extra=b",
        ] {
            assert!(matches!(
                profile().prepare(invalid, "text/plain", false),
                Preparation::Deferred(Problem::Unvalidated)
            ));
        }
    }
    #[test]
    fn mime_and_query_ambiguity_defer_without_vendor_rejection() {
        for mime in [
            "text/plain;charset=latin1",
            "text/plain;charset=utf-8;charset=utf-8",
            "text/plain;charset",
            "application/json",
        ] {
            assert!(matches!(
                profile().prepare("data=e30=", mime, false),
                Preparation::Deferred(Problem::Unvalidated)
            ));
        }
        assert!(matches!(
            profile().prepare("data=e30=", "text/plain", true),
            Preparation::Deferred(Problem::Unvalidated)
        ));
        assert!(matches!(
            profile().prepare(
                "data=e30=",
                "Text/Plain; charset=\"UTF-8\"; extra=safe",
                false
            ),
            Preparation::Data(_)
        ));
    }
    #[test]
    fn paired_cesu8_exception_is_complete_and_narrow() {
        assert_eq!(
            paired_cesu8(b"a\xed\xa0\xbd\xed\xb8\x80b"),
            Some("a😀b".into())
        );
        for bytes in [
            b"\xed\xa0\xbd\xed\xb8\x80\xff".as_slice(),
            b"\xed\xb8\x80\xed\xa0\xbd",
            b"\xed\xa0\xbd",
        ] {
            assert!(paired_cesu8(bytes).is_none());
        }
        assert!(paired_cesu8("ordinary 😀".as_bytes()).is_none());
    }
    #[test]
    fn sdk_two_thousand_one_records_reach_the_vendor_contract() {
        let event = serde_json::json!({"event":"producer control", "properties":{
            "token":"project", "distinct_id":"user", "$insert_id":"event", "time":1791648000,
            "price":1, "item":"example", "active":true, "source":"synthetic"
        }});
        let json = serde_json::to_string(&vec![event; 2001]).unwrap();
        assert!(matches!(
            profile().prepare(&form(&json), "text/plain", false),
            Preparation::Data(_)
        ));
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
        assert!(matches!(
            profile().prepare(&form(&encoded), "text/plain", false),
            Preparation::Data(_)
        ));
    }
    #[test]
    fn checked_resource_arithmetic_never_wraps() {
        assert!(checked_sum(&[usize::MAX, 1]).is_err());
        let mut budget = ProjectionBudget {
            text: usize::MAX,
            work: 0,
            finding_text: 0,
        };
        assert!(budget.charge(1, 0).is_err());
        let mut budget = ProjectionBudget {
            text: 0,
            work: usize::MAX,
            finding_text: 0,
        };
        assert!(budget.charge(0, 1).is_err());
    }
    #[test]
    fn source_target_suppression_is_private_to_the_new_profile() {
        let private = field("a large source data value".into());
        assert!(private.target().value.is_none());
        let legacy = RawParam::query("data", "a large source data value", 0, 0);
        assert_eq!(
            legacy.target().value.as_deref(),
            Some("a large source data value")
        );
    }
    #[test]
    fn signed_json_integer_does_not_prove_unsigned_format_validity() {
        let document = JsonDocument::parse("-1").unwrap();
        let field = document.get("").unwrap();
        assert!(type_matches(&serde_json::json!("integer"), field));
        assert!(!proven_valid(
            &serde_json::json!({"json_type":"integer", "format":{"kind":"integer"}}),
            Some(field),
            &document,
            ""
        ));
    }
    #[test]
    fn dynamic_operator_member_findings_are_projected_before_validation() {
        let profile = Profile::builtin(
            "vendor/mixpanel-engage",
            include_str!("../rulepacks/vendor/mixpanel-engage.json"),
        )
        .unwrap();
        let values: serde_json::Map<String, Value> = (0..90_000)
            .map(|index| {
                (
                    format!("property{index}"),
                    Value::String("invalid number".into()),
                )
            })
            .collect();
        let json = serde_json::json!({"$token":"project", "$distinct_id":"user", "$add":values})
            .to_string();
        assert!(matches!(
            profile.prepare(&form(&json), "text/plain", false),
            Preparation::Deferred(Problem::Limit)
        ));
    }
    #[test]
    fn source_only_secret_sanitization_work_is_preflighted() {
        let profile = Profile::builtin(
            "vendor/mixpanel-engage",
            include_str!("../rulepacks/vendor/mixpanel-engage.json"),
        )
        .unwrap();
        let sensitive: serde_json::Map<String, Value> = (0..3000)
            .map(|index| {
                (
                    format!("key{index}"),
                    Value::String(format!("unique{index}{}", "x".repeat(500))),
                )
            })
            .collect();
        let invalid: serde_json::Map<String, Value> = (0..500)
            .map(|index| {
                (
                    format!("counter{index}"),
                    Value::String("invalid number".into()),
                )
            })
            .collect();
        let json = serde_json::json!({"$token":"project", "$distinct_id":"user", "$add":invalid, "extra_passwords":sensitive}).to_string();
        assert!(matches!(
            profile.prepare(&form(&json), "text/plain", false),
            Preparation::Deferred(Problem::Limit)
        ));
    }
}
