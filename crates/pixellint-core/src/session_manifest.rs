//! Opted-in relationships between caller-declared ad sessions.
use crate::{Engine, RuleSourceLevel, Severity};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, error::Error, fmt, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuleKind {
    ConsistentParameter,
    UniqueParameterAcrossSessions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRule {
    pub code: String,
    pub kind: SessionRuleKind,
    pub param: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub fix_hint: Option<String>,
    #[serde(default)]
    pub placeholder_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRulesManifest {
    pub owner_plugin_id: String,
    pub source_level: RuleSourceLevel,
    pub docs: String,
    pub rules: Vec<SessionRule>,
}

pub(crate) struct CompiledSessionProfile {
    pub(crate) manifest: SessionRulesManifest,
    pub(crate) placeholders: Vec<HashSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionManifestError(pub String);
impl fmt::Display for SessionManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for SessionManifestError {}

impl SessionRulesManifest {
    pub fn from_json(raw: &str) -> Result<Self, SessionManifestError> {
        if raw.len() > crate::session::MAX_TEXT_BYTES {
            return Err(SessionManifestError(
                "session manifest exceeds local text limit".into(),
            ));
        }
        let value: Self =
            serde_json::from_str(raw).map_err(|e| SessionManifestError(e.to_string()))?;
        value.check()?;
        Ok(value)
    }
    fn check(&self) -> Result<(), SessionManifestError> {
        let fail = |s: &str| SessionManifestError(s.into());
        if self.owner_plugin_id.trim().is_empty()
            || self.docs.trim().is_empty()
            || self.rules.is_empty()
        {
            return Err(fail(
                "session profile requires owner, source docs and rules",
            ));
        }
        let mut text = self
            .owner_plugin_id
            .len()
            .checked_add(self.docs.len())
            .ok_or_else(|| fail("session manifest text overflow"))?;
        for rule in &self.rules {
            for bytes in [
                rule.code.len(),
                rule.param.len(),
                rule.message.len(),
                rule.fix_hint.as_ref().map_or(0, String::len),
            ]
            .into_iter()
            .chain(rule.placeholder_values.iter().map(String::len))
            {
                text = text
                    .checked_add(bytes)
                    .ok_or_else(|| fail("session manifest text overflow"))?;
                if text > crate::session::MAX_TEXT_BYTES {
                    return Err(fail("session manifest exceeds local text limit"));
                }
            }
        }
        let prefix = format!("{}.session.", self.owner_plugin_id.replace('/', "."));
        let mut codes = std::collections::HashSet::new();
        for r in &self.rules {
            if !r.code.starts_with(&prefix)
                || r.code.len() == prefix.len()
                || !codes.insert(&r.code)
            {
                return Err(fail(
                    "session rule code must be unique and within owner namespace",
                ));
            }
            if r.param.trim().is_empty() || r.message.trim().is_empty() {
                return Err(fail("session rule requires parameter and message"));
            }
        }
        Ok(())
    }
}

impl Engine {
    pub fn register_session_manifest(
        &mut self,
        profile: SessionRulesManifest,
    ) -> Result<(), SessionManifestError> {
        profile.check()?;
        if !self.plugins.contains_key(&profile.owner_plugin_id) {
            return Err(SessionManifestError(format!(
                "session owner not found: {}",
                profile.owner_plugin_id
            )));
        }
        let placeholders = profile
            .rules
            .iter()
            .map(|r| r.placeholder_values.iter().cloned().collect())
            .collect();
        self.session_profiles.insert(
            profile.owner_plugin_id.clone(),
            CompiledSessionProfile {
                manifest: profile,
                placeholders,
            },
        );
        Ok(())
    }
    pub fn register_session_manifest_json(
        &mut self,
        raw: &str,
    ) -> Result<(), SessionManifestError> {
        self.register_session_manifest(SessionRulesManifest::from_json(raw)?)
    }
    pub fn register_session_manifest_path(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), SessionManifestError> {
        let raw = std::fs::read_to_string(path).map_err(|e| SessionManifestError(e.to_string()))?;
        self.register_session_manifest_json(&raw)
    }
}
