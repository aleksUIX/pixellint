use std::env;
use std::fs;
use std::process::ExitCode;

use pixellint_core::{
    ArtifactKind, Engine, ExpansionState, Severity, ValidationOptions, ValidationRequest,
};

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
}

fn main() -> ExitCode {
    let engine = Engine::default();
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [command] if command == "list-rulepacks" => {
            for rulepack in engine.list_rulepacks() {
                println!(
                    "{}\t{}\t{}",
                    rulepack.id, rulepack.display_name, rulepack.description
                );
            }
            ExitCode::SUCCESS
        }
        [command, kind, input, rest @ ..] if command == "validate" => {
            let artifact_kind = match parse_artifact_kind(kind) {
                Ok(kind) => kind,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::FAILURE;
                }
            };

            let artifact = match read_artifact(input) {
                Ok(artifact) => artifact,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::FAILURE;
                }
            };

            let options = match parse_cli_options(rest) {
                Ok(options) => options,
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::FAILURE;
                }
            };

            let request = ValidationRequest {
                artifact_kind,
                artifact,
                claimed_vendor: None,
                expansion_state: options.expansion_state,
            };

            match engine.validate(&request, &options.validation) {
                Ok(summary) => {
                    if emit_summary(&summary, options.output_format).is_err() {
                        return ExitCode::FAILURE;
                    }

                    if has_errors(&summary) {
                        ExitCode::FAILURE
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            print_usage();
            ExitCode::FAILURE
        }
    }
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
        other => Err(format!("unknown artifact kind: {other}")),
    }
}

fn read_artifact(input: &str) -> Result<String, String> {
    if let Some(path) = input.strip_prefix('@') {
        fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
    } else {
        Ok(input.to_string())
    }
}

fn parse_cli_options(args: &[String]) -> Result<CliOptions, String> {
    let mut options = ValidationOptions::default();
    let mut output_format = OutputFormat::Text;
    let mut expansion_state = ExpansionState::Unknown;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                output_format = OutputFormat::Json;
                index += 1;
            }
            "--state" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --state".to_string())?;
                expansion_state = parse_expansion_state(value)?;
                index += 2;
            }
            "--rulepack" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --rulepack".to_string())?;
                options.only_rulepacks.push(value.clone());
                index += 2;
            }
            "--except" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --except".to_string())?;
                options.except_rulepacks.push(value.clone());
                index += 2;
            }
            other => {
                return Err(format!("unknown validate argument: {other}"));
            }
        }
    }

    Ok(CliOptions {
        validation: options,
        output_format,
        expansion_state,
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
    summary: &pixellint_core::ValidationSummary,
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

fn print_summary(summary: &pixellint_core::ValidationSummary) {
    for report in &summary.reports {
        println!("rulepack: {}", report.plugin_id);
        if report.violations.is_empty() {
            println!("ok");
            continue;
        }

        for violation in &report.violations {
            println!(
                "{:?}\t{}\t{}",
                violation.severity, violation.code, violation.message
            );
            if let Some(fix_hint) = &violation.fix_hint {
                println!("fix\t{}", fix_hint);
            }
        }
    }
}

fn has_errors(summary: &pixellint_core::ValidationSummary) -> bool {
    summary
        .reports
        .iter()
        .flat_map(|report| report.violations.iter())
        .any(|violation| violation.severity == Severity::Error)
}

fn print_usage() {
    eprintln!("pixellint-cli list-rulepacks");
    eprintln!(
        "pixellint-cli validate <url|html|js|gtm|request|vast|postback|unknown> <inline-or-@path> [--json] [--state <unknown|template|fired>] [--rulepack <id>]... [--except <id>]..."
    );
}
