use std::io::{self, BufRead, BufReader, Write};

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2024-11-05";
const INSTRUCTIONS: &str = "Pixellint validates pixels, postbacks, VAST tracking URLs, conversion API request bodies, and related measurement artifacts. The core rulepack applies spec-backed URL and macro checks to every artifact; vendor rulepacks add parameter contracts and only run when the artifact targets that vendor's endpoints. Use list_rulepacks to discover what is available, then call validate_artifact with an artifact kind and artifact payload. Every finding carries a stable code, a severity, an evidence level, and the documentation it came from.";

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let engine = Engine::default();
    let mut raw_message = String::new();

    loop {
        raw_message.clear();
        if reader.read_line(&mut raw_message)? == 0 {
            break;
        }

        if raw_message.trim().is_empty() {
            continue;
        }

        let action = handle_message(raw_message.trim(), &engine);
        if let Some(response) = action.response {
            write_message(&mut writer, &response)?;
        }
        if action.should_exit {
            break;
        }
    }

    Ok(())
}

struct Action {
    response: Option<Value>,
    should_exit: bool,
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct ValidateArtifactArgs {
    #[serde(alias = "artifactKind")]
    artifact_kind: String,
    artifact: String,
    #[serde(default, alias = "claimedVendor")]
    claimed_vendor: Option<String>,
    #[serde(default, alias = "expansionState")]
    expansion_state: Option<String>,
    #[serde(default)]
    rulepacks: Vec<String>,
    #[serde(default, alias = "exceptRulepacks")]
    except_rulepacks: Vec<String>,
}

fn handle_message(raw_message: &str, engine: &Engine) -> Action {
    let request = match serde_json::from_str::<RpcRequest>(raw_message) {
        Ok(request) => request,
        Err(error) => {
            return Action {
                response: Some(error_response(
                    Value::Null,
                    -32700,
                    &format!("parse error: {error}"),
                )),
                should_exit: false,
            };
        }
    };

    let id = request.id.clone().unwrap_or(Value::Null);
    let expects_response = request.id.is_some();

    match request.method.as_str() {
        "initialize" => Action {
            response: expects_response.then(|| success_response(id, initialize_result())),
            should_exit: false,
        },
        "notifications/initialized" => Action {
            response: None,
            should_exit: false,
        },
        "ping" => Action {
            response: expects_response.then(|| success_response(id, json!({}))),
            should_exit: false,
        },
        "tools/list" => Action {
            response: expects_response.then(|| success_response(id, tools_list_result(engine))),
            should_exit: false,
        },
        "tools/call" => Action {
            response: expects_response.then(|| handle_tool_call(id, request.params, engine)),
            should_exit: false,
        },
        "shutdown" => Action {
            response: expects_response.then(|| success_response(id, json!({}))),
            should_exit: false,
        },
        "exit" => Action {
            response: None,
            should_exit: true,
        },
        other => Action {
            response: expects_response
                .then(|| error_response(id, -32601, &format!("method not found: {other}"))),
            should_exit: false,
        },
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "pixellint",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": INSTRUCTIONS,
    })
}

fn tools_list_result(engine: &Engine) -> Value {
    let rulepack_ids: Vec<String> = engine
        .list_rulepacks()
        .into_iter()
        .map(|rulepack| rulepack.id)
        .collect();

    json!({
        "tools": [
            {
                "name": "list_rulepacks",
                "description": "List available Pixellint rulepacks that can be toggled per validation run.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                },
            },
            {
                "name": "list_vendors",
                "description": "List the vendor endpoint directory: which vendors Pixellint can attribute an endpoint to, and which of them have a rulepack. Use it to find out who owns a pixel host.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "description": "Optional category filter, for example social, programmatic, analytics, consent."
                        },
                        "host": {
                            "type": "string",
                            "description": "Optional host to attribute instead of listing everything."
                        }
                    },
                    "additionalProperties": false,
                },
            },
            {
                "name": "validate_artifact",
                "description": "Validate a measurement artifact such as a pixel URL, server postback, VAST tracking URL, conversion API JSON body, GTM template, HTML snippet, or JavaScript snippet.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "artifact_kind": {
                            "type": "string",
                            "enum": ["url", "html", "js", "gtm", "request", "vast", "postback", "json", "unknown"],
                            "description": "Artifact type to validate. Use vast for VAST tracking URLs, postback for server-side conversion or attribution endpoints, and json for a conversion API request body."
                        },
                        "artifact": {
                            "type": "string",
                            "description": "Inline artifact content to validate."
                        },
                        "claimed_vendor": {
                            "type": "string",
                            "description": "Optional vendor hint supplied by the caller."
                        },
                        "expansion_state": {
                            "type": "string",
                            "enum": ["unknown", "template", "fired"],
                            "description": "Whether the artifact is still a URL template with macros, an already fired URL, or unknown."
                        },
                        "rulepacks": {
                            "type": "array",
                            "items": { "type": "string", "enum": rulepack_ids },
                            "description": "Optional allowlist of rulepack IDs to run. Leave empty to let Pixellint select packs by endpoint."
                        },
                        "except_rulepacks": {
                            "type": "array",
                            "items": { "type": "string", "enum": rulepack_ids },
                            "description": "Optional denylist of rulepack IDs to skip."
                        }
                    },
                    "required": ["artifact_kind", "artifact"],
                    "additionalProperties": false,
                },
            }
        ]
    })
}

fn handle_tool_call(id: Value, params: Option<Value>, engine: &Engine) -> Value {
    let Some(params) = params else {
        return error_response(id, -32602, "missing tools/call params");
    };

    let tool_call = match serde_json::from_value::<ToolCallParams>(params) {
        Ok(tool_call) => tool_call,
        Err(error) => {
            return error_response(id, -32602, &format!("invalid tools/call params: {error}"));
        }
    };

    match tool_call.name.as_str() {
        "list_rulepacks" => success_response(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": engine
                        .list_rulepacks()
                        .iter()
                        .map(|rulepack| match &rulepack.vendor {
                            Some(vendor) => format!("{} ({vendor}): {}", rulepack.id, rulepack.description),
                            None => format!("{}: {}", rulepack.id, rulepack.description),
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }],
                "structuredContent": {
                    "rulepacks": engine.list_rulepacks(),
                }
            }),
        ),
        "list_vendors" => handle_list_vendors(id, tool_call.arguments, engine),
        "validate_artifact" => handle_validate_artifact(id, tool_call.arguments, engine),
        other => error_response(id, -32602, &format!("unknown tool: {other}")),
    }
}

#[derive(Debug, Default, Deserialize)]
struct ListVendorsArgs {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    host: Option<String>,
}

fn handle_list_vendors(id: Value, arguments: Value, engine: &Engine) -> Value {
    let args = if arguments.is_null() {
        ListVendorsArgs::default()
    } else {
        match serde_json::from_value::<ListVendorsArgs>(arguments) {
            Ok(args) => args,
            Err(error) => {
                return error_response(
                    id,
                    -32602,
                    &format!("invalid list_vendors arguments: {error}"),
                );
            }
        }
    };

    let directory = engine.directory();

    if let Some(host) = args.host.as_deref() {
        return match directory.lookup_host(host) {
            Some(entry) => success_response(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("{host} belongs to {} ({}).", entry.display_name, entry.category),
                    }],
                    "structuredContent": { "host": host, "vendor": entry },
                }),
            ),
            None => success_response(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("{host} is not in the vendor directory."),
                    }],
                    "structuredContent": { "host": host, "vendor": Value::Null },
                }),
            ),
        };
    }

    let vendors: Vec<&pixellint_core::VendorEntry> = directory
        .entries()
        .iter()
        .filter(|entry| {
            args.category
                .as_deref()
                .is_none_or(|category| entry.category.eq_ignore_ascii_case(category))
        })
        .collect();

    success_response(
        id,
        json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "{} vendor(s) in the directory, {} covered by a rulepack.",
                    vendors.len(),
                    vendors.iter().filter(|entry| entry.rulepack.is_some()).count(),
                ),
            }],
            "structuredContent": { "vendors": vendors },
        }),
    )
}

fn handle_validate_artifact(id: Value, arguments: Value, engine: &Engine) -> Value {
    let args = match serde_json::from_value::<ValidateArtifactArgs>(arguments) {
        Ok(args) => args,
        Err(error) => {
            return error_response(
                id,
                -32602,
                &format!("invalid validate_artifact arguments: {error}"),
            );
        }
    };

    let artifact_kind = match parse_artifact_kind(&args.artifact_kind) {
        Ok(artifact_kind) => artifact_kind,
        Err(message) => return error_response(id, -32602, &message),
    };
    let expansion_state = match parse_expansion_state(args.expansion_state.as_deref()) {
        Ok(expansion_state) => expansion_state,
        Err(message) => return error_response(id, -32602, &message),
    };

    let request = ValidationRequest {
        artifact_kind,
        artifact: args.artifact,
        claimed_vendor: args.claimed_vendor.clone(),
        expansion_state,
    };
    let options = ValidationOptions {
        only_rulepacks: args.rulepacks,
        except_rulepacks: args.except_rulepacks,
    };

    match engine.validate(&request, &options) {
        Ok(summary) => {
            let counts = violation_counts(&summary);
            success_response(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Validation completed for {} with {} error(s), {} warning(s), and {} info message(s).",
                            args.artifact_kind,
                            counts.errors,
                            counts.warnings,
                            counts.infos,
                        )
                    }],
                    "structuredContent": {
                        "artifact_kind": args.artifact_kind,
                        "claimed_vendor": args.claimed_vendor,
                        "detected_vendors": detected_vendors(&summary),
                        "expansion_state": expansion_state_label(expansion_state),
                        "ok": summary.is_ok(),
                        "summary": {
                            "errors": counts.errors,
                            "warnings": counts.warnings,
                            "infos": counts.infos,
                        },
                        "reports": summary.reports,
                    }
                }),
            )
        }
        Err(error) => success_response(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": error.to_string(),
                }],
                "structuredContent": {
                    "error": error.to_string(),
                },
                "isError": true,
            }),
        ),
    }
}

/// Vendors whose packs claimed the artifact, in report order and deduplicated.
fn detected_vendors(summary: &pixellint_core::ValidationSummary) -> Vec<String> {
    let mut vendors: Vec<String> = Vec::new();

    for vendor in summary
        .reports
        .iter()
        .filter_map(|report| report.detected_vendor.clone())
    {
        if !vendors.contains(&vendor) {
            vendors.push(vendor);
        }
    }

    vendors
}

fn parse_artifact_kind(value: &str) -> Result<ArtifactKind, String> {
    match value {
        "url" => Ok(ArtifactKind::Url),
        "html" => Ok(ArtifactKind::HtmlSnippet),
        "js" => Ok(ArtifactKind::JavaScriptSnippet),
        "gtm" => Ok(ArtifactKind::GtmTemplate),
        "request" => Ok(ArtifactKind::NetworkRequest),
        "vast" => Ok(ArtifactKind::VastTracker),
        "postback" => Ok(ArtifactKind::ServerPostback),
        "json" => Ok(ArtifactKind::JsonPayload),
        "unknown" => Ok(ArtifactKind::Unknown),
        other => Err(format!("unknown artifact kind: {other}")),
    }
}

fn parse_expansion_state(value: Option<&str>) -> Result<ExpansionState, String> {
    match value.unwrap_or("unknown") {
        "unknown" => Ok(ExpansionState::Unknown),
        "template" => Ok(ExpansionState::Template),
        "fired" => Ok(ExpansionState::Fired),
        other => Err(format!(
            "unknown expansion_state: {other} (expected unknown, template, or fired)"
        )),
    }
}

fn expansion_state_label(value: ExpansionState) -> &'static str {
    match value {
        ExpansionState::Unknown => "unknown",
        ExpansionState::Template => "template",
        ExpansionState::Fired => "fired",
    }
}

#[derive(Debug, Clone, Copy)]
struct ViolationCounts {
    errors: usize,
    warnings: usize,
    infos: usize,
}

fn violation_counts(summary: &pixellint_core::ValidationSummary) -> ViolationCounts {
    let mut counts = ViolationCounts {
        errors: 0,
        warnings: 0,
        infos: 0,
    };

    for violation in summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
    {
        match violation.severity {
            Severity::Error => counts.errors += 1,
            Severity::Warning => counts.warnings += 1,
            Severity::Info => counts.infos += 1,
        }
    }

    counts
}

fn write_message<W: Write>(writer: &mut W, response: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(response).map_err(to_io_error)?;
    writer.write_all(&payload)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

fn to_io_error(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
