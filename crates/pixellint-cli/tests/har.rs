//! Offline HAR CLI contracts, including capture uncertainty and explicit clocks.
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn archive(url: &str, time: Value) -> String {
    json!({"log":{"version":"1.2","entries":[{"startedDateTime":time,
        "request":{"url":url,"method":"GET","headers":[],"bodySize":0}}]}})
    .to_string()
}

fn run(command: &str, raw: &str, options: &[&str]) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pixellint"))
        .arg(command)
        .arg("-")
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(raw.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn import_outputs_capture_metadata_and_a_compatible_extracted_document() {
    let raw = archive(
        "https://www.facebook.com/tr?id=1234567890123456&ev=PageView",
        json!("2026-10-09T12:00:00Z"),
    );
    let (code, stdout, stderr) = run("import-har", &raw, &["--har-headers", "chrome_sanitized"]);
    assert_eq!(code, 0, "{stderr}");
    let imported: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(imported["document"]["document_kind"], "har");
    assert_eq!(
        imported["document"]["artifacts"][0]["artifact_kind"],
        "request"
    );
    assert_eq!(
        imported["entries"][0]["capture"]["redacted_headers"],
        json!(["authorization", "cookie"])
    );
    assert_eq!(
        imported["entries"][0]["reference_time_unix_seconds"],
        1_791_547_200
    );
    assert_eq!(imported["entries"][0]["body_availability"], "absent");
    assert_eq!(imported["header_policy"], "chrome_sanitized");
}

#[test]
fn validation_keeps_document_reports_provenance_and_error_exit_codes() {
    for (url, exit) in [
        (
            "https://www.facebook.com/tr?id=1234567890123456&ev=PageView",
            0,
        ),
        ("https://www.facebook.com/tr?ev=PageView", 1),
    ] {
        let raw = archive(url, json!("2026-10-09T12:00:00Z"));
        let (code, stdout, stderr) = run("validate-har", &raw, &["--json"]);
        assert_eq!(code, exit, "{stderr}: {stdout}");
        let report: Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["summary"]["artifacts_total"], 1);
        assert_eq!(
            report["artifacts"][0]["occurrences"][0]["path"],
            "/log/entries/0/request"
        );
        assert!(
            report["artifacts"][0]["reports"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["plugin_id"] == "vendor/meta")
        );
        assert_eq!(report["captures"][0]["entry_index"], 0);
    }
}

#[test]
fn unknown_capture_clock_requires_explicit_at_instead_of_using_today() {
    let raw = archive(
        "https://www.facebook.com/tr?id=1234567890123456&ev=PageView",
        Value::Null,
    );
    let (code, _, stderr) = run("validate-har", &raw, &["--json"]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("explicit reference-time override"),
        "{stderr}"
    );
    let (code, stdout, stderr) = run("validate-har", &raw, &["--json", "--at", "1791547200"]);
    assert_eq!(code, 0, "{stderr}: {stdout}");
    let report: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        report["captures"][0]["reference_time_unix_seconds"],
        Value::Null
    );
}

#[test]
fn malformed_archives_and_policy_usage_fail_with_exit_two() {
    for (command, raw, options) in [
        ("validate-har", "{}", vec!["--json"]),
        (
            "import-har",
            r#"{"log":{"version":"1.2","entries":[]}}"#,
            vec!["--har-headers", "guess"],
        ),
        (
            "import-har",
            r#"{"log":{"version":"1.2","entries":[]}}"#,
            vec!["--at", "10"],
        ),
    ] {
        let (code, stdout, stderr) = run(command, raw, &options);
        assert_eq!(code, 2, "{stdout}: {stderr}");
        assert!(stdout.is_empty());
    }
}
