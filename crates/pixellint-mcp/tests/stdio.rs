//! End-to-end checks over the MCP stdio transport.
//!
//! MCP stdio is newline-delimited JSON-RPC. These tests drive the real binary
//! the same way a client does, so a transport regression fails here rather than
//! in someone's editor.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn exchange(requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pixellint-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn pixellint-mcp");

    {
        let stdin = child.stdin.as_mut().expect("stdin");
        for request in requests {
            writeln!(stdin, "{request}").expect("write request");
        }
    }

    let output = child.wait_with_output().expect("wait for pixellint-mcp");
    assert!(
        output.status.success(),
        "server exited with {:?}",
        output.status
    );

    String::from_utf8(output.stdout)
        .expect("utf-8 stdout")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse response"))
        .collect()
}

#[test]
fn initialize_reports_tools_capability_and_server_identity() {
    let responses = exchange(&[json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2024-11-05" }
    })]);

    assert_eq!(responses.len(), 1);
    let result = &responses[0]["result"];
    assert_eq!(result["serverInfo"]["name"], "pixellint");
    assert!(result["capabilities"]["tools"].is_object());
    assert!(
        result["instructions"]
            .as_str()
            .expect("instructions")
            .contains("rulepack")
    );
}

#[test]
fn notifications_get_no_response() {
    let responses = exchange(&[
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
    ]);

    assert_eq!(responses.len(), 1, "a notification must not be answered");
    assert_eq!(responses[0]["id"], 1);
}

#[test]
fn tools_list_advertises_every_tool_with_live_rulepack_ids() {
    let responses = exchange(&[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })]);
    let tools = responses[0]["result"]["tools"].as_array().expect("tools");

    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(
        names,
        vec!["list_rulepacks", "list_vendors", "validate_artifact"]
    );

    let rulepacks = tools[2]["inputSchema"]["properties"]["rulepacks"]["items"]["enum"]
        .as_array()
        .expect("rulepack enum");
    assert!(rulepacks.iter().any(|id| id == "core"));
    assert!(rulepacks.iter().any(|id| id == "vendor/meta"));
}

#[test]
fn validate_artifact_returns_structured_findings() {
    let responses = exchange(&[json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "validate_artifact",
            "arguments": {
                "artifact_kind": "url",
                "artifact": "https://www.facebook.com/tr?ev=PageView"
            }
        }
    })]);

    let content = &responses[0]["result"]["structuredContent"];
    assert_eq!(content["ok"], false);
    assert_eq!(content["summary"]["errors"], 1);
    assert_eq!(content["detected_vendors"], json!(["meta"]));

    let codes: Vec<&str> = content["reports"]
        .as_array()
        .expect("reports")
        .iter()
        .flat_map(|report| report["violations"].as_array().expect("violations"))
        .map(|violation| violation["code"].as_str().expect("code"))
        .collect();
    assert_eq!(codes, vec!["vendor.meta.param.id.missing"]);
}

#[test]
fn a_clean_artifact_reports_no_findings() {
    let responses = exchange(&[json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "validate_artifact",
            "arguments": {
                "artifact_kind": "url",
                "artifact": "https://www.facebook.com/tr?id=1234567890123456&ev=PageView&noscript=1"
            }
        }
    })]);

    let content = &responses[0]["result"]["structuredContent"];
    assert_eq!(content["ok"], true);
    assert_eq!(content["summary"]["errors"], 0);
}

#[test]
fn rulepack_selection_and_bad_input_are_reported_without_crashing() {
    let responses = exchange(&[
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "validate_artifact",
                "arguments": {
                    "artifact_kind": "url",
                    "artifact": "https://www.facebook.com/tr?ev=PageView",
                    "rulepacks": ["core"]
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "validate_artifact",
                "arguments": { "artifact_kind": "banana", "artifact": "x" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "no_such_tool", "arguments": {} }
        }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "no/such/method" }),
    ]);

    let plugins: Vec<&str> = responses[0]["result"]["structuredContent"]["reports"]
        .as_array()
        .expect("reports")
        .iter()
        .map(|report| report["plugin_id"].as_str().expect("plugin id"))
        .collect();
    assert_eq!(plugins, vec!["core"]);

    assert_eq!(responses[1]["error"]["code"], -32602);
    assert_eq!(responses[2]["error"]["code"], -32602);
    assert_eq!(responses[3]["error"]["code"], -32601);
}

#[test]
fn malformed_json_gets_a_parse_error_and_the_server_keeps_going() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pixellint-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn pixellint-mcp");

    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{{not json").expect("write garbage");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"ping"}}"#).expect("write ping");
    }

    let output = child.wait_with_output().expect("wait for pixellint-mcp");
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .expect("utf-8 stdout")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse response"))
        .collect();

    assert_eq!(responses[0]["error"]["code"], -32700);
    assert_eq!(responses[1]["id"], 2);
}

#[test]
fn list_vendors_attributes_a_host_and_filters_by_category() {
    let responses = exchange(&[
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_vendors",
                "arguments": { "host": "pixel.mathtag.com" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "list_vendors",
                "arguments": { "category": "consent" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "list_vendors",
                "arguments": { "host": "pixel.nobody-knows-this.example" }
            }
        }),
    ]);

    assert_eq!(
        responses[0]["result"]["structuredContent"]["vendor"]["vendor"],
        "mediamath"
    );

    let consent = responses[1]["result"]["structuredContent"]["vendors"]
        .as_array()
        .expect("vendors");
    assert!(!consent.is_empty());
    assert!(consent.iter().all(|entry| entry["category"] == "consent"));

    assert!(responses[2]["result"]["structuredContent"]["vendor"].is_null());
}

#[test]
fn validation_reports_a_directory_attribution_when_no_pack_matches() {
    let responses = exchange(&[json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "validate_artifact",
            "arguments": {
                "artifact_kind": "url",
                "artifact": "https://trc.taboola.com/actions?a=1"
            }
        }
    })]);

    let content = &responses[0]["result"]["structuredContent"];
    assert_eq!(content["ok"], true);
    assert_eq!(content["detected_vendors"], json!(["taboola"]));
}
