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

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    fn date_now() -> f64;
}

fn current_unix_seconds() -> i64 {
    (date_now() / 1000.0).floor() as i64
}

fn to_js<T: serde::Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn to_js_object<T: serde::Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true);
    value
        .serialize(&serializer)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn parse_artifact_kind(value: &str) -> Result<ArtifactKind, JsValue> {
    match value {
        "url" => Ok(ArtifactKind::Url),
        "html" | "js" | "gtm" => Err(JsValue::from_str(&format!(
            "{value} is not a validation kind. Extract tracking URLs from the snippet, then validate as url. Pixellint does not parse HTML, JavaScript, or GTM containers."
        ))),
        "request" => Ok(ArtifactKind::NetworkRequest),
        "vast" => Ok(ArtifactKind::VastTracker),
        "postback" => Ok(ArtifactKind::ServerPostback),
        "json" => Ok(ArtifactKind::JsonPayload),
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
        .validate_at(
            &request,
            &ValidationOptions::default(),
            current_unix_seconds(),
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;

    to_js(&summary)
}

/// Validates extracted artifacts as one document. The caller already pulled
/// tracking URLs out. A JSON array of URL strings is a list of `url` artifacts.
#[wasm_bindgen]
pub fn validate_many(document_json: &str) -> Result<JsValue, JsValue> {
    let request = pixellint_core::document_request_from_json(document_json)
        .map_err(|error| JsValue::from_str(&error))?;
    let report = Engine::default()
        .validate_many_at(
            &request,
            &ValidationOptions::default(),
            current_unix_seconds(),
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_js(&report)
}

fn har_import_options(header_policy: &str) -> Result<pixellint_core::HarImportOptions, JsValue> {
    let header_policy = match header_policy {
        "unknown" => pixellint_core::HarHeaderPolicy::Unknown,
        "complete" => pixellint_core::HarHeaderPolicy::Complete,
        "chrome_sanitized" => pixellint_core::HarHeaderPolicy::ChromeSanitized,
        _ => {
            return Err(JsValue::from_str(
                "HAR header policy must be unknown, complete or chrome_sanitized",
            ));
        }
    };
    Ok(pixellint_core::HarImportOptions { header_policy })
}

/// Extracts local HAR 1.2 requests and records unavailable capture fields.
#[wasm_bindgen]
pub fn import_har(har_json: &str, header_policy: &str) -> Result<JsValue, JsValue> {
    let imported = pixellint_core::import_har(har_json, &har_import_options(header_policy)?)
        .map_err(|error| JsValue::from_str(&error))?;
    to_js_object(&imported)
}

/// Validates offline at each capture timestamp, or one caller-supplied override.
#[wasm_bindgen]
pub fn validate_har(
    har_json: &str,
    header_policy: &str,
    reference_time: Option<f64>,
) -> Result<JsValue, JsValue> {
    let imported = pixellint_core::import_har(har_json, &har_import_options(header_policy)?)
        .map_err(|error| JsValue::from_str(&error))?;
    let engine = Engine::default();
    let report = match reference_time {
        Some(time)
            if time.is_finite() && time.fract() == 0.0 && time.abs() <= 9_007_199_254_740_991.0 =>
        {
            engine.validate_har_at(&imported, &ValidationOptions::default(), time as i64)
        }
        Some(_) => {
            return Err(JsValue::from_str(
                "HAR reference time must be a safe integer number of Unix seconds",
            ));
        }
        None => engine.validate_har(&imported, &ValidationOptions::default()),
    }
    .map_err(|error| JsValue::from_str(&error))?;
    to_js_object(&report)
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
