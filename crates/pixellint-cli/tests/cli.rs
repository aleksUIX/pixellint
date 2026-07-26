//! CLI contract tests: exit codes, output shape, and custom rulepack loading.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn pixellint() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pixellint"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> (i32, String, String) {
    let output = pixellint().args(args).output().expect("run pixellint");

    (
        output.status.code().expect("exit code"),
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        String::from_utf8(output.stderr).expect("utf-8 stderr"),
    )
}

#[test]
fn clean_artifact_exits_zero() {
    let (code, stdout, _) = run(&[
        "validate",
        "url",
        "https://www.facebook.com/tr?id=1234567890123456&ev=PageView",
    ]);

    assert_eq!(code, 0, "{stdout}");
    assert!(
        stdout.contains("rulepack: vendor/meta (vendor: meta)"),
        "{stdout}"
    );
}

#[test]
fn warnings_alone_still_exit_zero() {
    let (code, stdout, _) = run(&[
        "validate",
        "url",
        "https://www.facebook.com/tr?id=1234567890123456&ev=Purchse",
    ]);

    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("vendor.meta.param.ev.invalid"), "{stdout}");
}

#[test]
fn error_severity_findings_exit_one() {
    let (code, stdout, _) = run(&["validate", "url", "https://www.facebook.com/tr?ev=PageView"]);

    assert_eq!(code, 1, "{stdout}");
    assert!(stdout.contains("vendor.meta.param.id.missing"), "{stdout}");
}

#[test]
fn usage_problems_exit_two() {
    for args in [
        vec!["validate", "banana", "https://example.com/pixel"],
        vec!["validate", "url", "https://example.com/pixel", "--nope"],
        vec!["validate", "url", "https://example.com/pixel", "--state"],
        vec!["validate", "url", "@/definitely/missing/fixture.txt"],
        vec!["frobnicate"],
        vec![],
    ] {
        let (code, _, stderr) = run(&args);
        assert_eq!(code, 2, "args {args:?} stderr {stderr}");
    }
}

#[test]
fn json_output_is_parseable_and_reports_targets() {
    let (code, stdout, _) = run(&[
        "validate",
        "url",
        "https://www.facebook.com/tr?id=nope&ev=PageView",
        "--json",
    ]);

    assert_eq!(code, 1);
    let summary: serde_json::Value = serde_json::from_str(&stdout).expect("parse json");
    let violation = summary["reports"]
        .as_array()
        .expect("reports")
        .iter()
        .flat_map(|report| report["violations"].as_array().expect("violations"))
        .find(|violation| violation["code"] == "vendor.meta.param.id.invalid")
        .expect("meta id finding");

    assert_eq!(violation["severity"], "error");
    assert_eq!(violation["targets"][0]["name"], "id");
    assert_eq!(violation["source"]["level"], "official_vendor");
}

#[test]
fn artifacts_can_be_read_from_a_file_or_stdin() {
    let fixture = repo_root().join("fixtures/core/clean-pixel.txt");
    let (code, _, stderr) = run(&["validate", "url", &format!("@{}", fixture.display())]);
    assert_eq!(code, 0, "{stderr}");

    let artifact = fs::read_to_string(&fixture).expect("read fixture");
    let mut child = pixellint()
        .args(["validate", "url", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn pixellint");

    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(artifact.as_bytes())
        .expect("write artifact");

    let output = child.wait_with_output().expect("wait for pixellint");
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn rulepack_selection_is_respected() {
    let (_, stdout, _) = run(&[
        "validate",
        "url",
        "https://www.facebook.com/tr?id=1234567890123456&ev=PageView",
        "--rulepack",
        "core",
    ]);
    assert!(stdout.contains("rulepack: core"), "{stdout}");
    assert!(!stdout.contains("rulepack: vendor/meta"), "{stdout}");

    // Excluding the pack stops it running, but the directory still says whose
    // endpoint this is, so assert on the report header rather than the string.
    let (_, stdout, _) = run(&[
        "validate",
        "url",
        "https://www.facebook.com/tr?id=1234567890123456&ev=PageView",
        "--except",
        "vendor/meta",
    ]);
    assert!(!stdout.contains("rulepack: vendor/meta"), "{stdout}");
    assert!(
        stdout.contains("rulepack: directory (vendor: meta)"),
        "{stdout}"
    );

    let (code, _, stderr) = run(&[
        "validate",
        "url",
        "https://example.com/pixel",
        "--rulepack",
        "vendor/does-not-exist",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("rulepack not found"), "{stderr}");
}

#[test]
fn custom_rulepack_files_load_and_run() {
    let manifest = repo_root().join("target/test-custom-rulepack.json");
    fs::write(
        &manifest,
        r#"{
  "id": "custom/acme",
  "display_name": "Acme internal pixel",
  "description": "Internal endpoint contract for the CLI test.",
  "vendor": "acme",
  "source_level": "heuristic",
  "match": { "hosts": ["px.acme.test"] },
  "params": [{ "name": "aid", "requirement": "required" }]
}
"#,
    )
    .expect("write custom manifest");

    let manifest = manifest.display().to_string();
    let (code, stdout, stderr) = run(&[
        "validate",
        "url",
        "https://px.acme.test/collect?other=1",
        "--rulepack-file",
        &manifest,
    ]);

    assert_eq!(code, 1, "{stderr}");
    assert!(stdout.contains("custom.acme.param.aid.missing"), "{stdout}");

    let (code, stdout, _) = run(&["list-rulepacks", "--rulepack-file", &manifest]);
    assert_eq!(code, 0);
    assert!(stdout.contains("custom/acme"), "{stdout}");

    let (code, _, stderr) = run(&[
        "validate",
        "url",
        "https://px.acme.test/collect",
        "--rulepack-file",
        "/definitely/missing/pack.json",
    ]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("could not read rulepack manifest"),
        "{stderr}"
    );
}

#[test]
fn help_and_version_exit_zero() {
    for flag in ["help", "--help", "-h"] {
        let (code, _, stderr) = run(&[flag]);
        assert_eq!(code, 0);
        assert!(stderr.contains("USAGE"), "{stderr}");
    }

    for flag in ["version", "--version", "-V"] {
        let (code, stdout, _) = run(&[flag]);
        assert_eq!(code, 0);
        assert!(stdout.starts_with("pixellint "), "{stdout}");
    }
}

#[test]
fn list_rulepacks_reports_every_builtin_pack() {
    let (code, stdout, _) = run(&["list-rulepacks"]);
    assert_eq!(code, 0);

    assert!(stdout.contains("core"), "{stdout}");
    for (id, _) in pixellint_core::BUILTIN_VENDOR_MANIFESTS {
        assert!(stdout.contains(id), "missing {id} in {stdout}");
    }

    let (code, stdout, _) = run(&["list-rulepacks", "--json"]);
    assert_eq!(code, 0);
    let packs: serde_json::Value = serde_json::from_str(&stdout).expect("parse json");
    assert_eq!(
        packs.as_array().expect("array").len(),
        pixellint_core::BUILTIN_VENDOR_MANIFESTS.len() + 1
    );
}

#[test]
fn list_vendors_reports_the_directory() {
    let (code, stdout, stderr) = run(&["list-vendors"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("taboola"), "{stdout}");
    assert!(stdout.contains("vendor/meta"), "{stdout}");
    assert!(stderr.contains("vendors"), "{stderr}");

    let (code, stdout, _) = run(&["list-vendors", "--json"]);
    assert_eq!(code, 0);
    let vendors: serde_json::Value = serde_json::from_str(&stdout).expect("parse json");
    assert_eq!(
        vendors.as_array().expect("array").len(),
        pixellint_core::VendorDirectory::builtin().len()
    );
}

#[test]
fn unknown_endpoints_are_attributed_to_their_vendor() {
    let (code, stdout, _) = run(&["validate", "url", "https://trc.taboola.com/actions?a=1"]);

    assert_eq!(code, 0, "attribution must never fail an artifact");
    assert!(
        stdout.contains("rulepack: directory (vendor: taboola)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("directory.no_rulepack_coverage"),
        "{stdout}"
    );

    let (_, stdout, _) = run(&[
        "validate",
        "url",
        "https://trc.taboola.com/actions?a=1",
        "--except",
        "directory",
    ]);
    assert!(!stdout.contains("directory"), "{stdout}");
}
