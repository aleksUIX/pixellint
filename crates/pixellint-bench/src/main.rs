//! In-process load benchmark for pixellint-core.
//!
//! Times Engine::validate on the golden corpus and D1-shaped synthetic
//! artifacts, then Engine::validate_many on large extracted-URL batches.
//! That is the number that matters for speeding the engine up. Process spawn
//! is optional (`--mode cli` / `--mode cli-many`).

mod case;
mod corpus;
mod load;
mod stats;
mod synth;

use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use pixellint_core::{Engine, ValidationOptions};
use serde::Serialize;

use crate::case::Case;
use crate::corpus::load_corpus;
use crate::load::{LoadCase, LoadOptions, load_cases};
use crate::stats::{
    SampleInput, SampleRow, print_corpus_summary, print_load_table, print_single_table,
    row_from_samples,
};
use crate::synth::synth_cases;

const USAGE_EXIT: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Corpus,
    Synth,
    Load,
    Cli,
    CliMany,
}

#[derive(Debug, Clone)]
struct Args {
    modes: Vec<Mode>,
    quick: bool,
    heavy: bool,
    warmup: usize,
    per_fixture: Option<usize>,
    batch_size: Option<usize>,
    filter: Option<String>,
    save: bool,
    json: bool,
    verbose: bool,
    write_fixtures: Option<PathBuf>,
    cli: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct Report {
    timestamp: u64,
    version: String,
    engine_construct_ms: f64,
    rulepacks: usize,
    corpus_cases: usize,
    modes: Vec<String>,
    results: Vec<SampleRow>,
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            print_usage();
            return ExitCode::from(USAGE_EXIT);
        }
    };

    let corpus = load_corpus();
    let synth = synth_cases();
    let batch_size = args
        .batch_size
        .unwrap_or(if args.quick { 500 } else { 10_000 });
    let load_options = if args.quick {
        LoadOptions::quick(batch_size)
    } else {
        LoadOptions::full(batch_size, args.heavy)
    };
    let loads = if wants(&args.modes, Mode::Load) || wants(&args.modes, Mode::CliMany) {
        load_cases(&corpus, &load_options)
    } else {
        Vec::new()
    };

    if let Some(dir) = &args.write_fixtures {
        if let Err(error) = write_fixtures(dir, &synth, &loads) {
            eprintln!("{error}");
            return ExitCode::from(USAGE_EXIT);
        }
        println!("wrote fixtures to {}", dir.display());
    }

    let warmup = if args.quick { 1 } else { args.warmup };
    let base_iters = args.per_fixture.unwrap_or(if args.quick { 5 } else { 100 });

    println!(
        "Pixellint bench · {} · corpus {} · synth {} · load {}",
        env!("CARGO_PKG_VERSION"),
        corpus.len(),
        synth.len(),
        loads.len()
    );

    let construct_start = Instant::now();
    let engine = Engine::default();
    let engine_construct_ms = construct_start.elapsed().as_secs_f64() * 1000.0;
    let rulepacks = engine.list_rulepacks().len();
    println!("engine construct: {engine_construct_ms:.1} ms ({rulepacks} rulepacks)");

    let options = ValidationOptions::default();
    let mut results = Vec::new();

    if wants(&args.modes, Mode::Corpus) {
        let cases = apply_filter(&corpus, args.filter.as_deref());
        println!(
            "\n== corpus (Engine::validate, {} fixtures, base {base_iters} runs) ==",
            cases.len()
        );
        let rows = bench_singles(&engine, &options, &cases, warmup, base_iters);
        if args.verbose {
            print_single_table(&rows);
        } else {
            print_corpus_summary(&rows);
        }
        results.extend(rows);
    }

    if wants(&args.modes, Mode::Synth) {
        let cases = apply_filter(&synth, args.filter.as_deref());
        println!(
            "\n== synth / D1-shaped (Engine::validate, {} fixtures) ==",
            cases.len()
        );
        let rows = bench_singles(&engine, &options, &cases, warmup, base_iters);
        print_single_table(&rows);
        results.extend(rows);
    }

    if wants(&args.modes, Mode::Load) {
        let cases: Vec<&LoadCase> = loads
            .iter()
            .filter(|case| match &args.filter {
                Some(filter) => case.name.contains(filter),
                None => true,
            })
            .collect();
        println!("\n== load (Engine::validate_many) ==");
        let mut rows = Vec::new();
        for case in &cases {
            let iters = load_iterations(case.artifacts, base_iters);
            let row = bench_load(&engine, &options, case, warmup, iters);
            println!(
                "  {}  {} items ({} unique)  mean {:.2} ms  {:.0} arts/s",
                case.name, case.artifacts, case.unique, row.mean_ms, row.artifacts_per_sec
            );
            rows.push(row);
        }

        if let Some(playground) = cases
            .iter()
            .find(|case| case.name.starts_with("d1-playground-"))
        {
            let iters = load_iterations(playground.artifacts, base_iters);
            let row = bench_loop_validate(&engine, &options, playground, warmup, iters);
            println!(
                "  {}  {} items (loop, no dedupe wrap)  mean {:.2} ms  {:.0} arts/s",
                row.name, playground.artifacts, row.mean_ms, row.artifacts_per_sec
            );
            rows.push(row);
        }

        print_load_table(&rows);
        results.extend(rows);
    }

    if wants(&args.modes, Mode::Cli) || wants(&args.modes, Mode::CliMany) {
        match ensure_cli(args.cli.as_deref()) {
            Ok(cli) => {
                if wants(&args.modes, Mode::Cli) {
                    let mut cases = apply_filter(&synth, args.filter.as_deref());
                    if cases.is_empty() {
                        cases = apply_filter(&corpus, args.filter.as_deref());
                        cases.truncate(20);
                    }
                    println!("\n== cli per-process ({} fixtures) ==", cases.len());
                    let rows =
                        bench_cli(&cli, &cases, warmup.min(2), if args.quick { 3 } else { 20 });
                    print_single_table(&rows);
                    results.extend(rows);
                }
                if wants(&args.modes, Mode::CliMany) {
                    println!("\n== cli validate-many ==");
                    let mut rows = Vec::new();
                    for case in &loads {
                        if let Some(filter) = &args.filter
                            && !case.name.contains(filter)
                        {
                            continue;
                        }
                        if case.artifacts > 2_000 && args.quick {
                            continue;
                        }
                        let iters = if args.quick {
                            1
                        } else {
                            load_iterations(case.artifacts, 10)
                        };
                        let row = bench_cli_many(&cli, case, iters);
                        println!(
                            "  {}  {} items  mean {:.2} ms  {:.0} arts/s",
                            case.name, case.artifacts, row.mean_ms, row.artifacts_per_sec
                        );
                        rows.push(row);
                    }
                    print_load_table(&rows);
                    results.extend(rows);
                }
            }
            Err(message) => {
                eprintln!("{message}");
                return ExitCode::from(USAGE_EXIT);
            }
        }
    }

    let report = Report {
        timestamp: unix_now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        engine_construct_ms,
        rulepacks,
        corpus_cases: corpus.len(),
        modes: args
            .modes
            .iter()
            .map(|mode| mode_name(*mode).to_string())
            .collect(),
        results,
    };

    if args.json {
        match serde_json::to_string_pretty(&report) {
            Ok(payload) => println!("{payload}"),
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(USAGE_EXIT);
            }
        }
    }

    if args.save {
        match save_report(&report) {
            Ok(path) => println!("saved {}", path.display()),
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(USAGE_EXIT);
            }
        }
    }

    ExitCode::SUCCESS
}

fn wants(modes: &[Mode], mode: Mode) -> bool {
    modes.contains(&mode)
}

fn apply_filter<'a>(cases: &'a [Case], filter: Option<&str>) -> Vec<&'a Case> {
    match filter {
        Some(filter) => cases
            .iter()
            .filter(|case| case.name.contains(filter))
            .collect(),
        None => cases.iter().collect(),
    }
}

fn single_iterations(bytes: usize, base: usize) -> usize {
    if bytes < 2_000 {
        base.max(1)
    } else if bytes < 20_000 {
        (base / 5).max(3)
    } else if bytes < 200_000 {
        (base / 20).max(2)
    } else {
        (base / 50).max(1)
    }
}

fn load_iterations(artifacts: usize, base: usize) -> usize {
    if artifacts <= 200 {
        base.max(1)
    } else if artifacts <= 2_000 {
        (base / 2).max(2)
    } else if artifacts <= 20_000 {
        (base / 10).max(2)
    } else {
        (base / 20).max(1)
    }
}

fn bench_singles(
    engine: &Engine,
    options: &ValidationOptions,
    cases: &[&Case],
    warmup: usize,
    base: usize,
) -> Vec<SampleRow> {
    let mut rows = Vec::with_capacity(cases.len());
    for case in cases {
        let iters = single_iterations(case.bytes(), base);
        let request = case.request();
        for _ in 0..warmup {
            let summary = engine.validate(&request, options).expect("warmup");
            black_box(summary.reports.len());
        }
        let mut samples = Vec::with_capacity(iters);
        let wall_start = Instant::now();
        for _ in 0..iters {
            let start = Instant::now();
            let summary = engine.validate(&request, options).expect("validate");
            black_box(summary.reports.len());
            samples.push(start.elapsed().as_secs_f64());
        }
        let wall = wall_start.elapsed().as_secs_f64();
        rows.push(row_from_samples(SampleInput {
            name: case.name.clone(),
            kind: case.kind_label().to_string(),
            bytes: case.bytes(),
            iterations: iters,
            artifacts_per_run: 1,
            unique_per_run: 1,
            wall_seconds: wall,
            samples,
        }));
    }
    rows
}

fn bench_load(
    engine: &Engine,
    options: &ValidationOptions,
    case: &LoadCase,
    warmup: usize,
    iters: usize,
) -> SampleRow {
    for _ in 0..warmup {
        let report = engine
            .validate_many(&case.request, options)
            .expect("warmup");
        black_box(report.summary.unique_artifacts);
    }
    let mut samples = Vec::with_capacity(iters);
    let wall_start = Instant::now();
    for _ in 0..iters {
        let start = Instant::now();
        let report = engine
            .validate_many(&case.request, options)
            .expect("validate-many");
        black_box(report.summary.unique_artifacts);
        samples.push(start.elapsed().as_secs_f64());
    }
    let wall = wall_start.elapsed().as_secs_f64();
    row_from_samples(SampleInput {
        name: case.name.clone(),
        kind: "many".to_string(),
        bytes: case.bytes,
        iterations: iters,
        artifacts_per_run: case.artifacts,
        unique_per_run: case.unique,
        wall_seconds: wall,
        samples,
    })
}

fn bench_loop_validate(
    engine: &Engine,
    options: &ValidationOptions,
    case: &LoadCase,
    warmup: usize,
    iters: usize,
) -> SampleRow {
    let requests: Vec<_> = case
        .request
        .artifacts
        .iter()
        .map(|item| pixellint_core::ValidationRequest {
            artifact_kind: item.artifact_kind,
            artifact: item.artifact.clone(),
            claimed_vendor: item.claimed_vendor.clone(),
            expansion_state: item.expansion_state,
        })
        .collect();

    let run = || {
        let mut n = 0usize;
        for request in &requests {
            let summary = engine.validate(request, options).expect("validate");
            n += summary.reports.len();
        }
        n
    };

    for _ in 0..warmup {
        black_box(run());
    }
    let mut samples = Vec::with_capacity(iters);
    let wall_start = Instant::now();
    for _ in 0..iters {
        let start = Instant::now();
        black_box(run());
        samples.push(start.elapsed().as_secs_f64());
    }
    let wall = wall_start.elapsed().as_secs_f64();
    row_from_samples(SampleInput {
        name: format!("{}-loop", case.name),
        kind: "loop".to_string(),
        bytes: case.bytes,
        iterations: iters,
        artifacts_per_run: case.artifacts,
        unique_per_run: case.artifacts,
        wall_seconds: wall,
        samples,
    })
}

fn bench_cli(cli: &Path, cases: &[&Case], warmup: usize, iters: usize) -> Vec<SampleRow> {
    let mut rows = Vec::new();
    let tmp = std::env::temp_dir().join(format!("pixellint-bench-cli-{}", unix_now()));
    fs::create_dir_all(&tmp).expect("temp dir");
    for case in cases {
        let path = tmp.join(format!("{}.txt", case.name.replace('/', "_")));
        fs::write(&path, &case.artifact).expect("write fixture");
        let cmd = [
            cli.to_string_lossy().into_owned(),
            "validate".to_string(),
            case.kind_label().to_string(),
            format!("@{}", path.display()),
            "--json".to_string(),
        ];
        for _ in 0..warmup {
            let _ = Command::new(&cmd[0])
                .args(&cmd[1..])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let mut samples = Vec::with_capacity(iters);
        let wall_start = Instant::now();
        for _ in 0..iters {
            let start = Instant::now();
            let _ = Command::new(&cmd[0])
                .args(&cmd[1..])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            samples.push(start.elapsed().as_secs_f64());
        }
        let wall = wall_start.elapsed().as_secs_f64();
        rows.push(row_from_samples(SampleInput {
            name: case.name.clone(),
            kind: "cli".to_string(),
            bytes: case.bytes(),
            iterations: iters,
            artifacts_per_run: 1,
            unique_per_run: 1,
            wall_seconds: wall,
            samples,
        }));
    }
    let _ = fs::remove_dir_all(&tmp);
    rows
}

fn bench_cli_many(cli: &Path, case: &LoadCase, iters: usize) -> SampleRow {
    let tmp = std::env::temp_dir().join(format!("pixellint-bench-many-{}.json", unix_now()));
    fs::write(&tmp, case.dump_json()).expect("write document");
    let mut samples = Vec::with_capacity(iters);
    let wall_start = Instant::now();
    for _ in 0..iters {
        let start = Instant::now();
        let _ = Command::new(cli)
            .args(["validate-many", &format!("@{}", tmp.display()), "--json"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        samples.push(start.elapsed().as_secs_f64());
    }
    let wall = wall_start.elapsed().as_secs_f64();
    let _ = fs::remove_file(&tmp);
    row_from_samples(SampleInput {
        name: format!("{}-cli", case.name),
        kind: "cli-many".to_string(),
        bytes: case.bytes,
        iterations: iters,
        artifacts_per_run: case.artifacts,
        unique_per_run: case.unique,
        wall_seconds: wall,
        samples,
    })
}

fn ensure_cli(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        return Err(format!("--cli path not found: {}", path.display()));
    }
    let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/pixellint");
    if default.exists() {
        return Ok(default);
    }
    println!("release CLI not found, building cargo build --release -p pixellint ...");
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", "pixellint"])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .status()
        .map_err(|error| format!("failed to spawn cargo: {error}"))?;
    if !status.success() {
        return Err("cargo build --release -p pixellint failed".to_string());
    }
    Ok(default)
}

fn write_fixtures(dir: &Path, synth: &[Case], loads: &[LoadCase]) -> Result<(), String> {
    let synth_dir = dir.join("synth");
    let load_dir = dir.join("load");
    fs::create_dir_all(&synth_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&load_dir).map_err(|error| error.to_string())?;
    for case in synth {
        fs::write(synth_dir.join(format!("{}.txt", case.name)), &case.artifact)
            .map_err(|error| error.to_string())?;
    }
    for case in loads {
        fs::write(
            load_dir.join(format!("{}.json", case.name)),
            case.dump_json(),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn save_report(report: &Report) -> Result<PathBuf, String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/results");
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(format!("bench-{}.json", report.timestamp));
    fs::write(
        &path,
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(path)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Corpus => "corpus",
        Mode::Synth => "synth",
        Mode::Load => "load",
        Mode::Cli => "cli",
        Mode::CliMany => "cli-many",
    }
}

fn parse_mode(value: &str) -> Result<Mode, String> {
    match value {
        "corpus" => Ok(Mode::Corpus),
        "synth" => Ok(Mode::Synth),
        "load" => Ok(Mode::Load),
        "cli" => Ok(Mode::Cli),
        "cli-many" => Ok(Mode::CliMany),
        other => Err(format!(
            "unknown mode {other} (expected corpus, synth, load, cli, cli-many)"
        )),
    }
}

fn parse_args(raw: Vec<String>) -> Result<Args, String> {
    if raw.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        std::process::exit(0);
    }

    let mut modes = Vec::new();
    let mut quick = false;
    let mut heavy = false;
    let mut warmup = 3usize;
    let mut per_fixture = None;
    let mut batch_size = None;
    let mut filter = None;
    let mut save = false;
    let mut json = false;
    let mut verbose = false;
    let mut write_fixtures = None;
    let mut cli = None;
    let mut index = 0;

    while index < raw.len() {
        match raw[index].as_str() {
            "--quick" => {
                quick = true;
                index += 1;
            }
            "--heavy" => {
                heavy = true;
                index += 1;
            }
            "--save" => {
                save = true;
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            "--mode" | "--warmup" | "--per-fixture" | "--batch-size" | "--filter"
            | "--write-fixtures" | "--cli" => {
                let key = raw[index].as_str();
                let value = raw
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for {key}"))?;
                match key {
                    "--mode" => {
                        if value == "all" {
                            modes.extend([
                                Mode::Corpus,
                                Mode::Synth,
                                Mode::Load,
                                Mode::Cli,
                                Mode::CliMany,
                            ]);
                        } else {
                            for part in value.split(',') {
                                modes.push(parse_mode(part.trim())?);
                            }
                        }
                    }
                    "--warmup" => {
                        warmup = value.parse().map_err(|_| "invalid --warmup".to_string())?
                    }
                    "--per-fixture" => {
                        per_fixture = Some(
                            value
                                .parse()
                                .map_err(|_| "invalid --per-fixture".to_string())?,
                        );
                    }
                    "--batch-size" => {
                        batch_size = Some(
                            value
                                .parse()
                                .map_err(|_| "invalid --batch-size".to_string())?,
                        );
                    }
                    "--filter" => filter = Some(value.clone()),
                    "--write-fixtures" => write_fixtures = Some(PathBuf::from(value)),
                    "--cli" => cli = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
                index += 2;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if modes.is_empty() {
        modes.extend([Mode::Corpus, Mode::Synth, Mode::Load]);
    }

    Ok(Args {
        modes,
        quick,
        heavy,
        warmup,
        per_fixture,
        batch_size,
        filter,
        save,
        json,
        verbose,
        write_fixtures,
        cli,
    })
}

fn print_usage() {
    print!(
        "\
Pixellint load benchmark

Usage:
  cargo run -p pixellint-bench --release -- [options]
  bench/run.sh [options]

Times pixellint-core in-process. Golden fixtures, D1-shaped synthetic
pixels, and huge validate-many batches. Use this as the baseline before
speeding the engine up.

Options:
  --mode corpus|synth|load|cli|cli-many   repeatable, or comma-separated
  --quick                                 small iteration counts
  --heavy                                 add a 100,000 unique-artifact batch
  --per-fixture N                         base timed runs per single fixture
  --batch-size N                          unique artifacts in the large load doc
  --warmup N                              untimed runs (default 3)
  --filter STR                            only fixtures whose name contains STR
  --write-fixtures DIR                    dump synth and load documents
  --save                                  write JSON under bench/results/
  --json                                  print the JSON report
  --verbose                               print every golden fixture row
  --cli PATH                              pixellint binary for cli modes
  -h, --help
"
    );
}
