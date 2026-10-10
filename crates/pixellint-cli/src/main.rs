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
    ArtifactKind, DocumentReport, Engine, ExpansionState, HarHeaderPolicy, HarImportOptions,
    RuleSourceLevel, Severity, ValidationOptions, ValidationRequest, ValidationSummary,
    VendorDirectory, document_request_from_json, import_har,
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
    directory_files: Vec<String>,
    reference_time: Option<i64>,
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
        [command, rest @ ..] if command == "validate-many" => run_validate_many(rest),
        [command, rest @ ..] if command == "validate-sessions" => run_validate_sessions(rest),
        [command, rest @ ..] if command == "import-har" => run_har(rest, false),
        [command, rest @ ..] if command == "validate-har" => run_har(rest, true),
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

    match options.reference_time.map_or_else(
        || engine.validate(&request, &options.validation),
        |time| engine.validate_at(&request, &options.validation, time),
    ) {
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

fn run_validate_many(args: &[String]) -> ExitCode {
    let [input, rest @ ..] = args else {
        print_usage();
        return ExitCode::from(USAGE_EXIT);
    };

    let raw = match read_artifact(input) {
        Ok(raw) => raw,
        Err(message) => return usage_error(&message),
    };

    let request = match document_request_from_json(&raw) {
        Ok(request) => request,
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

    match options.reference_time.map_or_else(
        || engine.validate_many(&request, &options.validation),
        |time| engine.validate_many_at(&request, &options.validation, time),
    ) {
        Ok(report) => {
            if let Err(error) = emit_document(&report, options.output_format) {
                return usage_error(&error.to_string());
            }

            if report.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => usage_error(&error.to_string()),
    }
}

fn run_validate_sessions(args: &[String]) -> ExitCode {
    let [input, rest @ ..] = args else {
        return usage_error("validate-sessions requires explicit grouped input");
    };
    let raw = match read_artifact(input) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let request = match pixellint_core::session_request_from_json(&raw) {
        Ok(v) => v,
        Err(e) => return usage_error(&e.to_string()),
    };
    let options = match parse_cli_options(rest) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let engine = match build_engine(&options) {
        Ok(v) => v,
        Err(e) => return usage_error(&e),
    };
    let result = match options.reference_time {
        Some(at) => engine.validate_sessions_at(&request, &options.validation, at),
        None => engine.validate_sessions(&request, &options.validation),
    };
    match result {
        Err(e) => usage_error(&e.to_string()),
        Ok(report) => {
            if options.output_format == OutputFormat::Json {
                match serde_json::to_string_pretty(&report) {
                    Ok(s) => println!("{s}"),
                    Err(e) => return usage_error(&e.to_string()),
                }
            } else {
                print_document(&report.document);
                for f in &report.findings {
                    println!(
                        "{}\t{}\t{}\tsessions: {}",
                        severity_label(f.severity),
                        f.code,
                        f.message,
                        f.session_ids.join(", ")
                    );
                    for t in &f.targets {
                        println!("  original row {} ({})", t.artifact_index, t.artifact_id);
                    }
                }
                println!(
                    "Session checks: {} evaluated, {} partial, {} not evaluated; {} ungrouped rows",
                    report.coverage.checks_evaluated,
                    report.coverage.checks_partially_evaluated,
                    report.coverage.checks_not_evaluated,
                    report.coverage.ungrouped_artifact_indexes.len()
                );
                for c in &report.checks {
                    for skip in &c.skipped {
                        println!(
                            "  {} row {} skipped: {:?}",
                            c.code, skip.artifact_index, skip.reason
                        );
                    }
                }
            }
            if report.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
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

    for path in &options.directory_files {
        let extra = VendorDirectory::from_path(path).map_err(|error| error.to_string())?;
        engine
            .merge_directory(extra)
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

fn snippet_kind_rejected(kind: &str) -> String {
    format!(
        "{kind} is not a validation kind. Extract tracking URLs from the snippet, then pixellint validate url. Pixellint does not parse HTML, JavaScript, or GTM containers."
    )
}

fn run_har(args: &[String], validate: bool) -> ExitCode {
    let [input, rest @ ..] = args else {
        print_usage();
        return ExitCode::from(USAGE_EXIT);
    };
    let raw = match read_artifact(input) {
        Ok(raw) => raw,
        Err(message) => return usage_error(&message),
    };
    let mut import_options = HarImportOptions::default();
    let mut validation_args = Vec::new();
    let mut args = rest.iter();
    while let Some(argument) = args.next() {
        if argument == "--har-headers" {
            import_options.header_policy = match args.next().map(String::as_str) {
                Some("unknown") => HarHeaderPolicy::Unknown,
                Some("complete") => HarHeaderPolicy::Complete,
                Some("chrome_sanitized") => HarHeaderPolicy::ChromeSanitized,
                _ => {
                    return usage_error(
                        "--har-headers requires unknown, complete or chrome_sanitized",
                    );
                }
            };
        } else {
            validation_args.push(argument.clone());
        }
    }
    if !validate && validation_args.iter().any(|arg| arg != "--json") {
        return usage_error(
            "import-har accepts --har-headers and --json; validation options belong to validate-har",
        );
    }
    let mut imported = match import_har(&raw, &import_options) {
        Ok(imported) => imported,
        Err(message) => return usage_error(&message),
    };
    if !validate {
        match serde_json::to_string_pretty(&imported) {
            Ok(payload) => println!("{payload}"),
            Err(error) => return usage_error(&error.to_string()),
        }
        return ExitCode::SUCCESS;
    }
    let options = match parse_cli_options(&validation_args) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };
    for artifact in &mut imported.document.artifacts {
        artifact.claimed_vendor = options.claimed_vendor.clone();
        if validation_args.iter().any(|argument| argument == "--state") {
            artifact.expansion_state = options.expansion_state;
        }
    }
    let engine = match build_engine(&options) {
        Ok(engine) => engine,
        Err(message) => return usage_error(&message),
    };
    let result = match options.reference_time {
        Some(time) => engine.validate_har_at(&imported, &options.validation, time),
        None => engine.validate_har(&imported, &options.validation),
    };
    match result {
        Ok(report) => {
            let emitted = if options.output_format == OutputFormat::Json {
                serde_json::to_string_pretty(&report).map(|payload| println!("{payload}"))
            } else {
                emit_document(&report.document, OutputFormat::Text)
            };
            if let Err(error) = emitted {
                return usage_error(&error.to_string());
            }
            if report.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(message) => usage_error(&message),
    }
}

fn parse_artifact_kind(value: &str) -> Result<ArtifactKind, String> {
    match value {
        "url" => Ok(ArtifactKind::Url),
        "html" | "js" | "gtm" => Err(snippet_kind_rejected(value)),
        "request" => Ok(ArtifactKind::NetworkRequest),
        "vast" => Ok(ArtifactKind::VastTracker),
        "postback" => Ok(ArtifactKind::ServerPostback),
        "json" => Ok(ArtifactKind::JsonPayload),
        "unknown" => Ok(ArtifactKind::Unknown),
        other => Err(format!(
            "unknown artifact kind: {other} (expected url, request, vast, postback, json, or unknown)"
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
    let mut directory_files = Vec::new();
    let mut reference_time = None;
    let mut index = 0;

    while index < args.len() {
        let argument = args[index].as_str();

        match argument {
            "--json" => {
                output_format = OutputFormat::Json;
                index += 1;
            }
            "--state" | "--rulepack" | "--except" | "--vendor" | "--rulepack-file"
            | "--directory-file" | "--at" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for {argument}"))?;

                match argument {
                    "--state" => expansion_state = parse_expansion_state(value)?,
                    "--rulepack" => validation.only_rulepacks.push(value.clone()),
                    "--except" => validation.except_rulepacks.push(value.clone()),
                    "--vendor" => claimed_vendor = Some(value.clone()),
                    "--rulepack-file" => rulepack_files.push(value.clone()),
                    "--directory-file" => directory_files.push(value.clone()),
                    "--at" => {
                        reference_time =
                            Some(value.parse::<i64>().map_err(|_| {
                                "--at requires Unix seconds as an integer".to_string()
                            })?)
                    }
                    _ => unreachable!(),
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
        directory_files,
        reference_time,
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

fn emit_document(
    report: &DocumentReport,
    output_format: OutputFormat,
) -> Result<(), serde_json::Error> {
    match output_format {
        OutputFormat::Text => {
            print_document(report);
            Ok(())
        }
        OutputFormat::Json => {
            let payload = serde_json::to_string_pretty(report)?;
            println!("{payload}");
            Ok(())
        }
    }
}

fn print_reports(reports: &[pixellint_core::ValidationReport]) -> (usize, usize, usize) {
    let mut errors = 0;
    let mut warnings = 0;
    let mut infos = 0;

    for report in reports {
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

    (errors, warnings, infos)
}

fn print_summary(summary: &ValidationSummary) {
    let (errors, warnings, infos) = print_reports(&summary.reports);
    println!(
        "\n{errors} error(s), {warnings} warning(s), {infos} info message(s) across {} rulepack(s).",
        summary.reports.len()
    );
}

fn print_document(report: &DocumentReport) {
    println!(
        "document: {} ({} artifact(s), {} unique)",
        report.document_kind,
        report.summary.artifacts_total.unwrap_or(0),
        report.summary.unique_artifacts.unwrap_or(0)
    );
    if let Some(extractor) = &report.extractor {
        match &extractor.version {
            Some(version) => println!("extractor: {} {version}", extractor.id),
            None => println!("extractor: {}", extractor.id),
        }
    }

    for artifact in &report.artifacts {
        println!();
        println!(
            "{} ({} occurrence(s)) {}",
            artifact.artifact_id,
            artifact.occurrences.len(),
            artifact.normalized_artifact
        );
        print_reports(&artifact.reports);
    }

    println!(
        "\n{} error(s), {} warning(s), {} info message(s) across {} unique artifact(s).",
        report.summary.errors,
        report.summary.warnings,
        report.summary.infos,
        report.artifacts.len()
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
  pixellint validate-many <document> [options]
  pixellint validate-sessions <session-request> [options]
  pixellint import-har <har> [--har-headers <policy>]
  pixellint validate-har <har> [options] [--har-headers <policy>]
  pixellint list-rulepacks [--json] [--rulepack-file <path>]...
  pixellint list-vendors [--json] [--directory-file <path>]...
  pixellint help
  pixellint version

KINDS
  url, request, vast, postback, json, unknown

ARTIFACT
  inline value, @path to read a file, or - to read stdin
  request captures: JSON with url, method, headers, and optional raw body or body_base64 wire bytes

DOCUMENT
  JSON object from MULTI_ARTIFACT_SCHEMA.md, or a JSON array of URL strings.
  Extract tracking URLs first. Pixellint does not parse VAST, HTML, or GTM.

OPTIONS
  --json                  Machine-readable output
  --state <state>         unknown (default), template, or fired
  --vendor <slug>         Vendor the caller believes the artifact belongs to
  --rulepack <id>         Run only these rulepacks (repeatable). `directory`
                          selects endpoint attribution
  --except <id>           Skip these rulepacks (repeatable)
  --rulepack-file <path>  Load a custom rulepack manifest (repeatable)
  --directory-file <path>  Merge extra vendor directory entries (repeatable)
  --at <unix-seconds>     Validate timestamp windows against this reference time
                          Overrides capture clocks in validate-har
  --har-headers <policy>  HAR only: unknown (default), complete, chrome_sanitized
                          HAR import and validation are offline; requests are not sent

EXIT CODES
  0  clean, or warnings and info only
  1  at least one error-severity finding
  2  usage, input, or configuration problem",
        version = env!("CARGO_PKG_VERSION")
    );
}
