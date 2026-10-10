use serde_json::{Value, json};
use std::process::Command;
fn input() -> String {
    json!({"document":{"artifacts":[{"artifact":"https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=a"},{"artifact":"https://unified.adsafeprotected.com/vevent/complete/1111111/66666666?xsId=b"}]},"sessions":[{"session_id":"ad-1","artifact_indexes":[0,1]}]}).to_string()
}
#[test]
fn explicit_sessions_exit_and_json_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_pixellint"))
        .args([
            "validate-sessions",
            &input(),
            "--json",
            "--at",
            "1791590400",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["relationship_summary"]["errors"], 1);
    assert_eq!(report["document"]["summary"]["errors"], 0);
    assert_eq!(report["summary"]["errors"], 1);
    let output = Command::new(env!("CARGO_BIN_EXE_pixellint"))
        .args([
            "validate-sessions",
            &input(),
            "--rulepack",
            "core",
            "--at",
            "1791590400",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("PackNotSelected")
    );
    let mut malformed: Value = serde_json::from_str(&input()).unwrap();
    malformed["sessions"][0]["artifact_indexes"] = json!([2]);
    let output = Command::new(env!("CARGO_BIN_EXE_pixellint"))
        .args(["validate-sessions", &malformed.to_string()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
