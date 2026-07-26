//! WASM bindings for `pixellint-core`.
//!
//! Exposes the same engine the CLI and MCP server use to JavaScript, so a
//! browser page or a Node script validates artifacts with identical rules and
//! identical rule ids.
//!
//! Build with:
//! ```sh
//! wasm-pack build crates/pixellint-wasm --target web --out-dir <out>
//! ```

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest, VendorDirectory,
};
use wasm_bindgen::prelude::*;

fn to_js<T: serde::Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn parse_artifact_kind(value: &str) -> Result<ArtifactKind, JsValue> {
    match value {
        "url" => Ok(ArtifactKind::Url),
        "html" => Ok(ArtifactKind::HtmlSnippet),
        "js" => Ok(ArtifactKind::JavaScriptSnippet),
        "gtm" => Ok(ArtifactKind::GtmTemplate),
        "request" => Ok(ArtifactKind::NetworkRequest),
        "vast" => Ok(ArtifactKind::VastTracker),
        "postback" => Ok(ArtifactKind::ServerPostback),
        "unknown" => Ok(ArtifactKind::Unknown),
        other => Err(JsValue::from_str(&format!(
            "unknown artifact kind: {other}"
        ))),
    }
}

fn parse_expansion_state(value: Option<String>) -> Result<ExpansionState, JsValue> {
    match value.as_deref().unwrap_or("unknown") {
        "unknown" => Ok(ExpansionState::Unknown),
        "template" => Ok(ExpansionState::Template),
        "fired" => Ok(ExpansionState::Fired),
        other => Err(JsValue::from_str(&format!(
            "unknown expansion state: {other}"
        ))),
    }
}

/// Validates one artifact and returns the full [`ValidationSummary`] as a plain
/// JS object.
#[wasm_bindgen]
pub fn validate(
    artifact_kind: &str,
    artifact: &str,
    expansion_state: Option<String>,
    claimed_vendor: Option<String>,
) -> Result<JsValue, JsValue> {
    let request = ValidationRequest {
        artifact_kind: parse_artifact_kind(artifact_kind)?,
        artifact: artifact.to_string(),
        claimed_vendor,
        expansion_state: parse_expansion_state(expansion_state)?,
    };

    let summary = Engine::default()
        .validate(&request, &ValidationOptions::default())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;

    to_js(&summary)
}

/// Validates a URL artifact with default options, the common case.
#[wasm_bindgen]
pub fn validate_url(artifact: &str) -> Result<JsValue, JsValue> {
    validate("url", artifact, None, None)
}

/// Every rulepack the engine ships, with its evidence level.
#[wasm_bindgen]
pub fn rulepacks() -> Result<JsValue, JsValue> {
    to_js(&Engine::default().list_rulepacks())
}

/// The vendor endpoint directory.
#[wasm_bindgen]
pub fn vendors() -> Result<JsValue, JsValue> {
    to_js(VendorDirectory::builtin().entries())
}

/// Attributes a host to a vendor, or returns `null` when the host is unknown.
#[wasm_bindgen]
pub fn vendor_for_host(host: &str) -> Result<JsValue, JsValue> {
    match VendorDirectory::builtin().lookup_host(host) {
        Some(entry) => to_js(entry),
        None => Ok(JsValue::NULL),
    }
}

/// The `pixellint-core` version this build wraps.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
