//! Pixellint command line interface.
//!
//! Exit codes are part of the contract:
//! `0` clean or warnings only, `1` at least one error-severity finding,
//! `2` usage, input, or configuration problem.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, RuleSourceLevel, Severity, ValidationOptions,
    ValidationRequest, ValidationSummary,
};

const USAGE_EXIT: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    validation: ValidationOptions,
    output_format: OutputFormat,
    expansion_state: ExpansionState,
    claimed_vendor: Option<String>,
    rulepack_files: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [] => {
            print_usage();
            ExitCode::from(USAGE_EXIT)
        }
        [command] if is_help(command) => {
            print_usage();
            ExitCode::SUCCESS
        }
        [command] if is_version(command) => {
            println!("pixellint {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [command, rest @ ..] if command == "list-rulepacks" => run_list_rulepacks(rest),
        [command, rest @ ..] if command == "list-vendors" => run_list_vendors(rest),
        [command, rest @ ..] if command == "validate" => run_validate(rest),
        [command, ..] => {
            eprintln!("unknown command: {command}");
            print_usage();
            ExitCode::from(USAGE_EXIT)
        }
    }
}

fn run_list_rulepacks(args: &[String]) -> ExitCode {
    let options = match parse_cli_options(args) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };

    let engine = match build_engine(&options) {
        Ok(engine) => engine,
        Err(message) => return usage_error(&message),
    };

    if options.output_format == OutputFormat::Json {
        match serde_json::to_string_pretty(&engine.list_rulepacks()) {
            Ok(payload) => println!("{payload}"),
            Err(error) => return usage_error(&error.to_string()),
        }

        return ExitCode::SUCCESS;
    }

    for rulepack in engine.list_rulepacks() {
        println!(
            "{}\t{}\t{}\t{}",
            rulepack.id,
            rulepack.display_name,
            source_level_label(rulepack.source_level),
            rulepack.description
        );
    }

    ExitCode::SUCCESS
}

fn run_list_vendors(args: &[String]) -> ExitCode {
    let options = match parse_cli_options(args) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };

    let engine = match build_engine(&options) {
        Ok(engine) => engine,
        Err(message) => return usage_error(&message),
    };

    let directory = engine.directory();

    if options.output_format == OutputFormat::Json {
        match serde_json::to_string_pretty(directory.entries()) {
            Ok(payload) => println!("{payload}"),
            Err(error) => return usage_error(&error.to_string()),
        }

        return ExitCode::SUCCESS;
    }

    for entry in directory.entries() {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            entry.vendor,
            entry.display_name,
            entry.category,
            entry.rulepack.as_deref().unwrap_or("-"),
            entry.hosts.join(",")
        );
    }

    eprintln!(
        "\n{} vendors, {} hosts, {} with a rulepack.",
        directory.len(),
        directory.host_count(),
        directory
            .entries()
            .iter()
            .filter(|entry| entry.rulepack.is_some())
            .count()
    );

    ExitCode::SUCCESS
}

fn run_validate(args: &[String]) -> ExitCode {
    let [kind, input, rest @ ..] = args else {
        print_usage();
        return ExitCode::from(USAGE_EXIT);
    };

    let artifact_kind = match parse_artifact_kind(kind) {
        Ok(artifact_kind) => artifact_kind,
        Err(message) => return usage_error(&message),
    };

    let artifact = match read_artifact(input) {
        Ok(artifact) => artifact,
        Err(message) => return usage_error(&message),
    };

    let options = match parse_cli_options(rest) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };

    let engine = match build_engine(&options) {
        Ok(engine) => engine,
        Err(message) => return usage_error(&message),
    };

    let request = ValidationRequest {
        artifact_kind,
        artifact,
        claimed_vendor: options.claimed_vendor.clone(),
        expansion_state: options.expansion_state,
    };

    match engine.validate(&request, &options.validation) {
        Ok(summary) => {
            if let Err(error) = emit_summary(&summary, options.output_format) {
                return usage_error(&error.to_string());
            }

            if has_errors(&summary) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => usage_error(&error.to_string()),
    }
}

/// Builds the engine with the built-in packs plus any user-supplied manifests.
fn build_engine(options: &CliOptions) -> Result<Engine, String> {
    let mut engine = Engine::default();

    for path in &options.rulepack_files {
        engine
            .register_manifest_path(path)
            .map_err(|error| error.to_string())?;
    }

    Ok(engine)
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(USAGE_EXIT)
}

fn is_help(value: &str) -> bool {
    matches!(value, "help" | "--help" | "-h")
}

fn is_version(value: &str) -> bool {
    matches!(value, "version" | "--version" | "-V")
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
        "unknown" => Ok(ArtifactKind::Unknown),
        other => Err(format!(
            "unknown artifact kind: {other} (expected url, html, js, gtm, request, vast, postback, or unknown)"
        )),
    }
}

/// Reads the artifact inline, from `@path`, or from stdin when the input is `-`.
fn read_artifact(input: &str) -> Result<String, String> {
    if input == "-" {
        let mut artifact = String::new();
        return io::stdin()
            .read_to_string(&mut artifact)
            .map(|_| artifact)
            .map_err(|error| format!("failed to read stdin: {error}"));
    }

    if let Some(path) = input.strip_prefix('@') {
        return fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"));
    }

    Ok(input.to_string())
}

fn parse_cli_options(args: &[String]) -> Result<CliOptions, String> {
    let mut validation = ValidationOptions::default();
    let mut output_format = OutputFormat::Text;
    let mut expansion_state = ExpansionState::Unknown;
    let mut claimed_vendor = None;
    let mut rulepack_files = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let argument = args[index].as_str();

        match argument {
            "--json" => {
                output_format = OutputFormat::Json;
                index += 1;
            }
            "--state" | "--rulepack" | "--except" | "--vendor" | "--rulepack-file" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for {argument}"))?;

                match argument {
                    "--state" => expansion_state = parse_expansion_state(value)?,
                    "--rulepack" => validation.only_rulepacks.push(value.clone()),
                    "--except" => validation.except_rulepacks.push(value.clone()),
                    "--vendor" => claimed_vendor = Some(value.clone()),
                    _ => rulepack_files.push(value.clone()),
                }

                index += 2;
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
    }

    Ok(CliOptions {
        validation,
        output_format,
        expansion_state,
        claimed_vendor,
        rulepack_files,
    })
}

fn parse_expansion_state(value: &str) -> Result<ExpansionState, String> {
    match value {
        "unknown" => Ok(ExpansionState::Unknown),
        "template" => Ok(ExpansionState::Template),
        "fired" => Ok(ExpansionState::Fired),
        other => Err(format!(
            "unknown expansion state: {other} (expected unknown, template, or fired)"
        )),
    }
}

fn emit_summary(
    summary: &ValidationSummary,
    output_format: OutputFormat,
) -> Result<(), serde_json::Error> {
    match output_format {
        OutputFormat::Text => {
            print_summary(summary);
            Ok(())
        }
        OutputFormat::Json => {
            let payload = serde_json::to_string_pretty(summary)?;
            println!("{payload}");
            Ok(())
        }
    }
}

fn print_summary(summary: &ValidationSummary) {
    let mut errors = 0;
    let mut warnings = 0;
    let mut infos = 0;

    for report in &summary.reports {
        match &report.detected_vendor {
            Some(vendor) => println!("rulepack: {} (vendor: {vendor})", report.plugin_id),
            None => println!("rulepack: {}", report.plugin_id),
        }

        if report.violations.is_empty() {
            println!("  ok");
            continue;
        }

        for violation in &report.violations {
            match violation.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }

            println!(
                "  {}\t{}\t{}",
                severity_label(violation.severity),
                violation.code,
                violation.message
            );

            if let Some(fix_hint) = &violation.fix_hint {
                println!("    fix: {fix_hint}");
            }

            if let Some(reference) = &violation.source.reference {
                println!("    docs: {reference}");
            }
        }
    }

    println!(
        "\n{errors} error(s), {warnings} warning(s), {infos} info message(s) across {} rulepack(s).",
        summary.reports.len()
    );
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn source_level_label(level: RuleSourceLevel) -> &'static str {
    match level {
        RuleSourceLevel::Normative => "normative",
        RuleSourceLevel::OfficialVendor => "official-vendor",
        RuleSourceLevel::OfficialTemplate => "official-template",
        RuleSourceLevel::EcosystemReference => "ecosystem-reference",
        RuleSourceLevel::Heuristic => "heuristic",
    }
}

fn has_errors(summary: &ValidationSummary) -> bool {
    summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
        .any(|violation| violation.severity == Severity::Error)
}

fn print_usage() {
    eprintln!(
        "\
pixellint {version}
Spec-first validator for pixels, postbacks, and other measurement artifacts.

USAGE
  pixellint validate <kind> <artifact> [options]
  pixellint list-rulepacks [--json] [--rulepack-file <path>]...
  pixellint list-vendors [--json]
  pixellint help
  pixellint version

KINDS
  url, html, js, gtm, request, vast, postback, unknown

ARTIFACT
  inline value, @path to read a file, or - to read stdin

OPTIONS
  --json                  Machine-readable output
  --state <state>         unknown (default), template, or fired
  --vendor <slug>         Vendor the caller believes the artifact belongs to
  --rulepack <id>         Run only these rulepacks (repeatable). `directory`
                          selects endpoint attribution
  --except <id>           Skip these rulepacks (repeatable)
  --rulepack-file <path>  Load a custom rulepack manifest (repeatable)

EXIT CODES
  0  clean, or warnings and info only
  1  at least one error-severity finding
  2  usage, input, or configuration problem",
        version = env!("CARGO_PKG_VERSION")
    );
}
