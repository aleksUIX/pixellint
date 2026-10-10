//! Relationships over explicitly grouped, original URL rows. No grouping is inferred.
use crate::{
    ArtifactKind, ArtifactOccurrence, DocumentError, DocumentReport, DocumentRequest, Engine,
    EngineError, FindingCounts, PreparedArtifact, RuleSource, SessionRule, SessionRuleKind,
    Severity, ValidationOptions, ValidationRequest, detect_macro_spans,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

pub const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
const MAX_ROWS: usize = 50_000;
const MAX_GROUPS: usize = 10_000;
const MAX_OCCURRENCES: usize = 100_000;
const MAX_SPANS: usize = 100_000;
const MAX_WORK: usize = 2_000_000;
const MAX_OUTPUT_ITEMS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionGroup {
    pub session_id: String,
    pub artifact_indexes: Vec<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub document: DocumentRequest,
    pub sessions: Vec<SessionGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEvaluationStatus {
    Evaluated,
    PartiallyEvaluated,
    NotEvaluated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionCheckReason {
    InsufficientObservations,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSkipReason {
    PackNotSelected,
    UnsupportedArtifact,
    InvalidUrl,
    EndpointMismatch,
    MissingParameter,
    EmptyParameter,
    InstallationPlaceholder,
    UnresolvedMacro,
    ConflictingValues,
    InvalidPercentEncoding,
    InvalidUtf8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTargetComponent {
    QueryParam,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionQuerySpan {
    pub component: SessionTargetComponent,
    pub name: String,
    pub value: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionArtifactTarget {
    pub artifact_index: usize,
    pub artifact_id: String,
    pub session_id: String,
    pub query_spans: Vec<SessionQuerySpan>,
    pub occurrences: Vec<ArtifactOccurrence>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionSkippedArtifact {
    pub artifact_index: usize,
    pub artifact_id: String,
    pub session_id: String,
    pub reason: SessionSkipReason,
    pub occurrences: Vec<ArtifactOccurrence>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionFinding {
    pub plugin_id: String,
    pub detected_vendor: Option<String>,
    pub code: String,
    pub message: String,
    pub severity: Severity,
    pub field: Option<String>,
    pub fix_hint: Option<String>,
    pub source: RuleSource,
    pub session_ids: Vec<String>,
    pub targets: Vec<SessionArtifactTarget>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionCheckCoverage {
    pub plugin_id: String,
    pub code: String,
    pub kind: SessionRuleKind,
    pub session_ids: Vec<String>,
    pub status: SessionEvaluationStatus,
    pub participants_total: usize,
    pub compared_total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<SessionCheckReason>,
    pub skipped: Vec<SessionSkippedArtifact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionCoverage {
    pub status: SessionEvaluationStatus,
    pub sessions_total: usize,
    pub grouped_artifacts: usize,
    pub ungrouped_artifact_indexes: Vec<usize>,
    pub profiles_total: usize,
    pub checks_evaluated: usize,
    pub checks_partially_evaluated: usize,
    pub checks_not_evaluated: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionReport {
    pub document: DocumentReport,
    pub summary: FindingCounts,
    pub relationship_summary: FindingCounts,
    pub findings: Vec<SessionFinding>,
    pub checks: Vec<SessionCheckCoverage>,
    pub coverage: SessionCoverage,
}
impl SessionReport {
    pub fn is_ok(&self) -> bool {
        self.summary.errors == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    Input {
        code: &'static str,
        message: String,
    },
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    Engine(EngineError),
    Document(DocumentError),
}
impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input { code, message } => write!(f, "{code}: {message}"),
            Self::ResourceLimit { resource, limit } => write!(
                f,
                "local session resource limit: {resource} (maximum {limit})"
            ),
            Self::Engine(e) => e.fmt(f),
            Self::Document(e) => e.fmt(f),
        }
    }
}
impl Error for SessionError {}
fn input(code: &'static str, message: impl Into<String>) -> SessionError {
    SessionError::Input {
        code,
        message: message.into(),
    }
}
fn limit(resource: &'static str, value: usize, maximum: usize) -> Result<(), SessionError> {
    if value > maximum {
        Err(SessionError::ResourceLimit {
            resource,
            limit: maximum,
        })
    } else {
        Ok(())
    }
}
fn add(a: usize, b: usize) -> Result<usize, SessionError> {
    a.checked_add(b).ok_or(SessionError::ResourceLimit {
        resource: "projection_overflow",
        limit: usize::MAX,
    })
}
fn mul(a: usize, b: usize) -> Result<usize, SessionError> {
    a.checked_mul(b).ok_or(SessionError::ResourceLimit {
        resource: "projection_overflow",
        limit: usize::MAX,
    })
}
fn sum(values: impl IntoIterator<Item = usize>) -> Result<usize, SessionError> {
    values.into_iter().try_fold(0, add)
}

pub fn session_request_from_json(raw: &str) -> Result<SessionRequest, SessionError> {
    limit("serialized_json_bytes", raw.len(), MAX_TEXT_BYTES)?;
    serde_json::from_str(raw).map_err(|e| input("invalid_json", e.to_string()))
}

/// Present clock fields require integer tokens. Omitted fields use the caller's clock.
pub fn deserialize_session_clock<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<i64>, D::Error> {
    let value = i64::deserialize(d)?;
    if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value) {
        return Err(serde::de::Error::custom(
            "session clock must be safe integer Unix seconds",
        ));
    }
    Ok(Some(value))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionBindingOptions {
    #[serde(default, deserialize_with = "deserialize_session_clock")]
    pub at: Option<i64>,
    #[serde(default)]
    pub rulepacks: Vec<String>,
    #[serde(default)]
    pub except_rulepacks: Vec<String>,
}
pub fn session_options_from_json(raw: &str) -> Result<SessionBindingOptions, SessionError> {
    limit("serialized_options_bytes", raw.len(), MAX_TEXT_BYTES)?;
    serde_json::from_str(raw).map_err(|e| input("invalid_options", e.to_string()))
}

struct Metrics {
    n: usize,
    g: usize,
    o: usize,
    q: usize,
    bq: usize,
    ot: usize,
    lm: usize,
    lg: usize,
}
fn occurrence_bytes(o: &ArtifactOccurrence) -> Result<usize, SessionError> {
    sum([
        o.occurrence_id.as_deref(),
        o.source_kind.as_deref(),
        o.path.as_deref(),
        o.context_label.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::len))
}
fn query_region(raw: &str) -> Option<(usize, &str)> {
    let trimmed = raw.trim();
    let lead = raw.len() - raw.trim_start().len();
    let end = trimmed.find('#').unwrap_or(trimmed.len());
    let start = trimmed[..end].find('?')? + 1;
    Some((lead + start, &trimmed[start..end]))
}
fn url_like(kind: ArtifactKind, raw: &str) -> bool {
    let mut tokens = raw
        .trim_start()
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace();
    let raw_http = tokens.next().is_some()
        && tokens.next().is_some()
        && tokens.next().is_some_and(|v| v.starts_with("HTTP/"));
    if raw_http {
        return false;
    }
    !matches!(
        kind,
        ArtifactKind::JsonPayload
            | ArtifactKind::HtmlSnippet
            | ArtifactKind::JavaScriptSnippet
            | ArtifactKind::GtmTemplate
    ) && !raw.trim_start().starts_with(['{', '['])
}
fn inspect(request: &SessionRequest) -> Result<(Vec<Option<usize>>, Metrics), SessionError> {
    limit("artifact_rows", request.document.artifacts.len(), MAX_ROWS)?;
    limit("groups", request.sessions.len(), MAX_GROUPS)?;
    let mut text = sum([
        request.document.document_kind.len(),
        request
            .document
            .extractor
            .as_ref()
            .map_or(0, |x| x.id.len()),
        request
            .document
            .extractor
            .as_ref()
            .and_then(|x| x.version.as_ref())
            .map_or(0, String::len),
    ])?;
    let mut occurrences = 0;
    for row in &request.document.artifacts {
        text = add(text, row.artifact.len())?;
        text = add(text, row.claimed_vendor.as_ref().map_or(0, String::len))?;
        occurrences = add(occurrences, row.occurrences.len())?;
        for o in &row.occurrences {
            text = add(text, occurrence_bytes(o)?)?;
        }
    }
    limit("occurrences", occurrences, MAX_OCCURRENCES)?;
    let mut membership = vec![None; request.document.artifacts.len()];
    let mut labels = HashSet::new();
    let mut m = Metrics {
        n: 0,
        g: request.sessions.len(),
        o: 0,
        q: 0,
        bq: 0,
        ot: 0,
        lm: 0,
        lg: 0,
    };
    for (group_index, group) in request.sessions.iter().enumerate() {
        if group.session_id.trim().is_empty() {
            return Err(input("invalid_session_id", "session label is blank"));
        }
        limit("session_id_bytes", group.session_id.len(), 4096)?;
        if !labels.insert(&group.session_id) {
            return Err(input("duplicate_session_id", &group.session_id));
        }
        text = add(text, group.session_id.len())?;
        m.lg = add(m.lg, group.session_id.len())?;
        m.n = add(m.n, group.artifact_indexes.len())?;
        limit("memberships", m.n, MAX_ROWS)?;
        for &i in &group.artifact_indexes {
            let Some(slot) = membership.get_mut(i) else {
                return Err(input("artifact_index_out_of_range", i.to_string()));
            };
            if let Some(old) = *slot {
                return Err(input(
                    if old == group_index {
                        "duplicate_artifact_index"
                    } else {
                        "overlapping_artifact_index"
                    },
                    i.to_string(),
                ));
            }
            *slot = Some(group_index);
            let row = &request.document.artifacts[i];
            m.o = add(m.o, row.occurrences.len())?;
            m.lm = add(m.lm, group.session_id.len())?;
            for o in &row.occurrences {
                m.ot = add(m.ot, occurrence_bytes(o)?)?;
            }
            if url_like(row.artifact_kind, &row.artifact)
                && let Some((_, query)) = query_region(&row.artifact)
            {
                m.q = add(m.q, query.split('&').filter(|s| !s.is_empty()).count())?;
                m.bq = add(m.bq, query.len())?;
            }
        }
    }
    limit("aggregate_owned_text_bytes", text, MAX_TEXT_BYTES)?;
    limit("query_spans", m.q, MAX_SPANS)?;
    Ok((membership, m))
}
fn project(engine: &Engine, m: &Metrics, ungrouped: usize) -> Result<(), SessionError> {
    let mut items = add(1, ungrouped)?;
    let mut text = 128;
    let mut work = 0;
    for compiled in engine.session_profiles.values() {
        let profile = &compiled.manifest;
        let owner = engine
            .plugins
            .get(&profile.owner_plugin_id)
            .expect("profile bound to registered owner")
            .plugin
            .metadata();
        for rule in &profile.rules {
            work = add(work, add(m.n, m.g)?)?;
            limit("rule_member_work_units", work, MAX_WORK)?;
            let within = rule.kind == SessionRuleKind::ConsistentParameter;
            let (c, f, lf) = if within {
                (m.g, m.g, m.lg)
            } else {
                (1, m.n, m.lm)
            };
            items = add(
                items,
                sum([m.n, m.q, m.o, c, f, m.g, if within { m.g } else { m.n }])?,
            )?;
            limit("projected_output_items", items, MAX_OUTPUT_ITEMS)?;
            let row_text = sum([
                m.ot,
                m.lm,
                mul(32, m.n)?,
                m.bq,
                mul(m.q, rule.param.len())?,
                mul(64, m.n)?,
                mul(32, m.q)?,
            ])?;
            let check_text = add(mul(c, sum([owner.id.len(), rule.code.len(), 128])?)?, m.lg)?;
            let finding_text = add(
                mul(
                    f,
                    sum([
                        owner.id.len(),
                        owner.vendor.as_ref().map_or(0, String::len),
                        rule.code.len(),
                        rule.message.len(),
                        rule.fix_hint.as_ref().map_or(0, String::len),
                        owner.display_name.len(),
                        profile.docs.len(),
                        rule.param.len(),
                        128,
                    ])?,
                )?,
                lf,
            )?;
            text = add(text, sum([row_text, check_text, finding_text])?)?;
            limit("projected_output_text", text, MAX_TEXT_BYTES)?;
        }
    }
    Ok(())
}

fn decode(raw: &str) -> Result<String, SessionSkipReason> {
    let mut bytes = Vec::with_capacity(raw.len());
    let mut i = 0;
    let data = raw.as_bytes();
    while i < data.len() {
        match data[i] {
            b'%' => {
                if i + 2 >= data.len() {
                    return Err(SessionSkipReason::InvalidPercentEncoding);
                }
                let hi = (data[i + 1] as char)
                    .to_digit(16)
                    .ok_or(SessionSkipReason::InvalidPercentEncoding)?;
                let lo = (data[i + 2] as char)
                    .to_digit(16)
                    .ok_or(SessionSkipReason::InvalidPercentEncoding)?;
                bytes.push((hi * 16 + lo) as u8);
                i += 3;
            }
            b'+' => {
                bytes.push(b' ');
                i += 1;
            }
            byte => {
                bytes.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(bytes).map_err(|_| SessionSkipReason::InvalidUtf8)
}
fn identity(
    region: Option<(usize, &str)>,
    rule: &SessionRule,
    placeholders: &HashSet<String>,
) -> Result<(String, Vec<SessionQuerySpan>), SessionSkipReason> {
    let Some((offset, query)) = region else {
        return Err(SessionSkipReason::MissingParameter);
    };
    let mut spans = Vec::new();
    let mut problems = Vec::new();
    let mut pos = offset;
    for part in query.split('&') {
        if let Some((name, raw)) = part.split_once('=')
            && decode(name).ok().as_deref() == Some(&rule.param)
        {
            let value = if !detect_macro_spans(raw).is_empty() {
                Ok(raw.to_string())
            } else {
                decode(raw)
            };
            match value {
                Err(reason) => problems.push(reason),
                Ok(value) => {
                    if placeholders.contains(&value) {
                        problems.push(SessionSkipReason::InstallationPlaceholder);
                    } else if !detect_macro_spans(&value).is_empty() {
                        problems.push(SessionSkipReason::UnresolvedMacro);
                    } else if value.trim().is_empty() {
                        problems.push(SessionSkipReason::EmptyParameter);
                    }
                    spans.push(SessionQuerySpan {
                        component: SessionTargetComponent::QueryParam,
                        name: rule.param.clone(),
                        value,
                        start: pos + name.len() + 1,
                        end: pos + part.len(),
                    });
                }
            }
        }
        pos += part.len() + 1;
    }
    for reason in [
        SessionSkipReason::InvalidPercentEncoding,
        SessionSkipReason::InvalidUtf8,
        SessionSkipReason::InstallationPlaceholder,
        SessionSkipReason::UnresolvedMacro,
        SessionSkipReason::EmptyParameter,
    ] {
        if problems.contains(&reason) {
            return Err(reason);
        }
    }
    let Some(first) = spans.first() else {
        return Err(SessionSkipReason::MissingParameter);
    };
    if spans.iter().any(|s| s.value != first.value) {
        return Err(SessionSkipReason::ConflictingValues);
    }
    Ok((first.value.clone(), spans))
}
type Resolved = Result<(String, Vec<SessionQuerySpan>), SessionSkipReason>;

impl Engine {
    pub fn validate_sessions(
        &self,
        request: &SessionRequest,
        options: &ValidationOptions,
    ) -> Result<SessionReport, SessionError> {
        self.validate_sessions_at(request, options, crate::timestamp::current_unix_seconds())
    }
    pub fn validate_sessions_at(
        &self,
        request: &SessionRequest,
        options: &ValidationOptions,
        at: i64,
    ) -> Result<SessionReport, SessionError> {
        self.ensure_known_rulepacks(&options.only_rulepacks)
            .map_err(SessionError::Engine)?;
        self.ensure_known_rulepacks(&options.except_rulepacks)
            .map_err(SessionError::Engine)?;
        let (membership, metrics) = inspect(request)?;
        project(
            self,
            &metrics,
            membership.iter().filter(|s| s.is_none()).count(),
        )?;
        let document = self
            .validate_many_at(&request.document, options, at)
            .map_err(SessionError::Document)?;
        let by_artifact: HashMap<_, _> = document
            .artifacts
            .iter()
            .map(|a| {
                (
                    (a.artifact_kind as u8, a.normalized_artifact.as_str()),
                    a.artifact_id.as_str(),
                )
            })
            .collect();
        let ids: Vec<&str> = request
            .document
            .artifacts
            .iter()
            .map(|r| by_artifact[&(r.artifact_kind as u8, r.artifact.trim())])
            .collect();
        let grouped_indices: Vec<_> = membership
            .iter()
            .enumerate()
            .filter_map(|(i, g)| g.map(|_| i))
            .collect();
        let validation: Vec<_> = grouped_indices
            .iter()
            .map(|&i| {
                let r = &request.document.artifacts[i];
                (
                    i,
                    ValidationRequest {
                        artifact_kind: r.artifact_kind,
                        artifact: r.artifact.clone(),
                        claimed_vendor: r.claimed_vendor.clone(),
                        expansion_state: r.expansion_state,
                    },
                )
            })
            .collect();
        let prepared: HashMap<_, _> = validation
            .iter()
            .map(|(i, r)| (*i, PreparedArtifact::from_request_at(r, at)))
            .collect();
        let regions: HashMap<_, _> = grouped_indices
            .iter()
            .map(|&i| (i, query_region(&request.document.artifacts[i].artifact)))
            .collect();
        let mut findings = Vec::new();
        let mut checks = Vec::new();
        for compiled in self.session_profiles.values() {
            let profile = &compiled.manifest;
            let owner = &self.plugins[&profile.owner_plugin_id].plugin;
            let selected = !options.except_rulepacks.contains(&profile.owner_plugin_id)
                && (options.only_rulepacks.is_empty()
                    || options.only_rulepacks.contains(&profile.owner_plugin_id));
            for (rule_index, rule) in profile.rules.iter().enumerate() {
                let resolved: HashMap<usize, Resolved> = grouped_indices
                    .iter()
                    .map(|&i| {
                        let r = &request.document.artifacts[i];
                        let value = if !selected {
                            Err(SessionSkipReason::PackNotSelected)
                        } else if !url_like(r.artifact_kind, &r.artifact) {
                            Err(SessionSkipReason::UnsupportedArtifact)
                        } else if prepared[&i].url().is_none() {
                            Err(SessionSkipReason::InvalidUrl)
                        } else if !owner.supports_prepared(&prepared[&i]) {
                            Err(SessionSkipReason::EndpointMismatch)
                        } else {
                            identity(regions[&i], rule, &compiled.placeholders[rule_index])
                        };
                        (i, value)
                    })
                    .collect();
                let target = |i: usize| SessionArtifactTarget {
                    artifact_index: i,
                    artifact_id: ids[i].into(),
                    session_id: request.sessions[membership[i].unwrap()].session_id.clone(),
                    query_spans: resolved[&i].as_ref().unwrap().1.clone(),
                    occurrences: request.document.artifacts[i].occurrences.clone(),
                };
                let skipped = |i: usize, reason| SessionSkippedArtifact {
                    artifact_index: i,
                    artifact_id: ids[i].into(),
                    session_id: request.sessions[membership[i].unwrap()].session_id.clone(),
                    reason,
                    occurrences: request.document.artifacts[i].occurrences.clone(),
                };
                let finding = |session_ids: Vec<String>, indices: Vec<usize>| SessionFinding {
                    plugin_id: profile.owner_plugin_id.clone(),
                    detected_vendor: owner.metadata().vendor.clone(),
                    code: rule.code.clone(),
                    message: rule.message.clone(),
                    severity: rule.severity,
                    field: Some(rule.param.clone()),
                    fix_hint: rule.fix_hint.clone(),
                    source: RuleSource {
                        level: profile.source_level,
                        name: owner.metadata().display_name.clone(),
                        reference: Some(profile.docs.clone()),
                    },
                    session_ids,
                    targets: indices.into_iter().map(&target).collect(),
                };
                let check = |session_ids: Vec<String>,
                             participants_total,
                             known: usize,
                             enough: bool,
                             skipped: Vec<SessionSkippedArtifact>| {
                    SessionCheckCoverage {
                        plugin_id: profile.owner_plugin_id.clone(),
                        code: rule.code.clone(),
                        kind: rule.kind,
                        session_ids,
                        participants_total,
                        compared_total: known,
                        status: if !enough {
                            SessionEvaluationStatus::NotEvaluated
                        } else if skipped.is_empty() {
                            SessionEvaluationStatus::Evaluated
                        } else {
                            SessionEvaluationStatus::PartiallyEvaluated
                        },
                        reason: (!enough).then_some(SessionCheckReason::InsufficientObservations),
                        skipped,
                    }
                };
                if rule.kind == SessionRuleKind::ConsistentParameter {
                    for group in &request.sessions {
                        let mut known = Vec::new();
                        let mut omitted = Vec::new();
                        let mut values = HashSet::new();
                        let mut indices = group.artifact_indexes.clone();
                        indices.sort_unstable();
                        for i in indices {
                            match &resolved[&i] {
                                Ok((value, _)) => {
                                    values.insert(value);
                                    known.push(i);
                                }
                                Err(reason) => omitted.push(skipped(i, *reason)),
                            }
                        }
                        if values.len() > 1 {
                            findings.push(finding(vec![group.session_id.clone()], known.clone()));
                        }
                        checks.push(check(
                            vec![group.session_id.clone()],
                            group.artifact_indexes.len(),
                            known.len(),
                            known.len() >= 2,
                            omitted,
                        ));
                    }
                } else {
                    let mut by_value: HashMap<&str, usize> = HashMap::new();
                    let mut values: Vec<(&str, Vec<usize>, HashSet<usize>)> = Vec::new();
                    let mut groups = HashSet::new();
                    let mut known = 0;
                    let mut omitted = Vec::new();
                    for &i in &grouped_indices {
                        let g = membership[i].unwrap();
                        match &resolved[&i] {
                            Ok((value, _)) => {
                                known += 1;
                                groups.insert(g);
                                let index = *by_value.entry(value).or_insert_with(|| {
                                    values.push((value, Vec::new(), HashSet::new()));
                                    values.len() - 1
                                });
                                values[index].1.push(i);
                                values[index].2.insert(g);
                            }
                            Err(reason) => omitted.push(skipped(i, *reason)),
                        }
                    }
                    let mut labels = vec![Vec::new(); values.len()];
                    for group in &request.sessions {
                        let mut seen = HashSet::new();
                        for &i in &group.artifact_indexes {
                            if let Ok((value, _)) = &resolved[&i]
                                && seen.insert(value.as_str())
                            {
                                let slot = by_value[value.as_str()];
                                if values[slot].2.len() >= 2 {
                                    labels[slot].push(group.session_id.clone());
                                }
                            }
                        }
                    }
                    for (slot, (_, indices, groups)) in values.into_iter().enumerate() {
                        if groups.len() >= 2 {
                            findings.push(finding(std::mem::take(&mut labels[slot]), indices));
                        }
                    }
                    checks.push(check(
                        request
                            .sessions
                            .iter()
                            .map(|g| g.session_id.clone())
                            .collect(),
                        metrics.n,
                        known,
                        groups.len() >= 2,
                        omitted,
                    ));
                }
            }
        }
        let mut relationship_summary = FindingCounts {
            artifacts_total: None,
            unique_artifacts: None,
            errors: 0,
            warnings: 0,
            infos: 0,
        };
        for f in &findings {
            match f.severity {
                Severity::Error => relationship_summary.errors += 1,
                Severity::Warning => relationship_summary.warnings += 1,
                Severity::Info => relationship_summary.infos += 1,
            }
        }
        let mut summary = document.summary.clone();
        summary.errors += relationship_summary.errors;
        summary.warnings += relationship_summary.warnings;
        summary.infos += relationship_summary.infos;
        let count = |status| checks.iter().filter(|c| c.status == status).count();
        let evaluated = count(SessionEvaluationStatus::Evaluated);
        let partial = count(SessionEvaluationStatus::PartiallyEvaluated);
        let none = count(SessionEvaluationStatus::NotEvaluated);
        let ungrouped: Vec<_> = membership
            .iter()
            .enumerate()
            .filter_map(|(i, g)| g.is_none().then_some(i))
            .collect();
        let status = if evaluated + partial == 0 {
            SessionEvaluationStatus::NotEvaluated
        } else if partial + none > 0 || !ungrouped.is_empty() {
            SessionEvaluationStatus::PartiallyEvaluated
        } else {
            SessionEvaluationStatus::Evaluated
        };
        let coverage = SessionCoverage {
            status,
            sessions_total: metrics.g,
            grouped_artifacts: metrics.n,
            ungrouped_artifact_indexes: ungrouped,
            profiles_total: self.session_profiles.len(),
            checks_evaluated: evaluated,
            checks_partially_evaluated: partial,
            checks_not_evaluated: none,
        };
        Ok(SessionReport {
            document,
            summary,
            relationship_summary,
            findings,
            checks,
            coverage,
        })
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn checked_budget_boundaries() {
        assert!(limit("projected_output_items", MAX_OUTPUT_ITEMS, MAX_OUTPUT_ITEMS).is_ok());
        assert!(
            limit(
                "projected_output_items",
                MAX_OUTPUT_ITEMS + 1,
                MAX_OUTPUT_ITEMS
            )
            .is_err()
        );
        assert!(limit("projected_output_text", MAX_TEXT_BYTES, MAX_TEXT_BYTES).is_ok());
        assert!(limit("projected_output_text", MAX_TEXT_BYTES + 1, MAX_TEXT_BYTES).is_err());
        assert!(add(usize::MAX, 1).is_err());
        assert!(mul(usize::MAX, 2).is_err());
    }
    #[test]
    fn collection_exact_and_one_over_boundaries() {
        let row = crate::DocumentArtifactInput {
            artifact_kind: ArtifactKind::Url,
            artifact: "https://example.com/?x=y".into(),
            claimed_vendor: None,
            expansion_state: crate::ExpansionState::Unknown,
            occurrences: vec![],
        };
        let mut r = SessionRequest {
            document: DocumentRequest {
                document_kind: "list".into(),
                extractor: None,
                artifacts: vec![row.clone(); MAX_ROWS],
            },
            sessions: vec![],
        };
        assert!(inspect(&r).is_ok());
        r.document.artifacts.push(row.clone());
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "artifact_rows",
                ..
            })
        ));
        r.document.artifacts = vec![row.clone()];
        r.sessions = (0..MAX_GROUPS)
            .map(|i| SessionGroup {
                session_id: format!("ad-{i}"),
                artifact_indexes: vec![],
            })
            .collect();
        assert!(inspect(&r).is_ok());
        r.sessions.push(SessionGroup {
            session_id: "extra".into(),
            artifact_indexes: vec![],
        });
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "groups",
                ..
            })
        ));
        r.sessions = vec![SessionGroup {
            session_id: "x".repeat(4096),
            artifact_indexes: vec![],
        }];
        assert!(inspect(&r).is_ok());
        r.sessions[0].session_id.push('x');
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "session_id_bytes",
                ..
            })
        ));
        r.sessions = vec![SessionGroup {
            session_id: "ad".into(),
            artifact_indexes: vec![0],
        }];
        r.document.artifacts[0].artifact =
            format!("https://example.com/?{}", vec!["x=y"; MAX_SPANS].join("&"));
        assert!(inspect(&r).is_ok());
        r.document.artifacts[0].artifact.push_str("&x=y");
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "query_spans",
                ..
            })
        ));
        let occurrence = ArtifactOccurrence {
            occurrence_id: None,
            source_kind: None,
            path: None,
            line: None,
            column: None,
            context_label: None,
        };
        r.document.artifacts[0] = row.clone();
        r.document.artifacts[0].occurrences = vec![occurrence.clone(); MAX_OCCURRENCES];
        assert!(inspect(&r).is_ok());
        r.document.artifacts[0].occurrences.push(occurrence);
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "occurrences",
                ..
            })
        ));
        r.document.artifacts = vec![row.clone(); MAX_ROWS];
        r.sessions[0].artifact_indexes = (0..MAX_ROWS).collect();
        assert!(inspect(&r).is_ok());
        r.sessions[0].artifact_indexes.push(0);
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "memberships",
                ..
            })
        ));
        r.sessions.clear();
        r.document.artifacts = vec![row];
        r.document.artifacts[0].artifact = "x".repeat(MAX_TEXT_BYTES - 4);
        assert!(inspect(&r).is_ok());
        r.document.artifacts[0].artifact.push('x');
        assert!(matches!(
            inspect(&r),
            Err(SessionError::ResourceLimit {
                resource: "aggregate_owned_text_bytes",
                ..
            })
        ));
        let mut raw = " ".repeat(MAX_TEXT_BYTES);
        assert!(matches!(
            session_request_from_json(&raw),
            Err(SessionError::Input {
                code: "invalid_json",
                ..
            })
        ));
        raw.push(' ');
        assert!(matches!(
            session_request_from_json(&raw),
            Err(SessionError::ResourceLimit {
                resource: "serialized_json_bytes",
                ..
            })
        ));
        assert!(limit("rule_member_work_units", MAX_WORK, MAX_WORK).is_ok());
        assert!(limit("rule_member_work_units", MAX_WORK + 1, MAX_WORK).is_err());
    }
}
