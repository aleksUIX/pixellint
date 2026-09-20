use serde::Serialize;

use crate::case::{human_bytes, tier_of};

#[derive(Debug, Clone, Serialize)]
pub struct SampleRow {
    pub name: String,
    pub kind: String,
    pub tier: String,
    pub bytes: usize,
    pub iterations: usize,
    pub artifacts_per_run: usize,
    pub unique_per_run: usize,
    pub wall_seconds: f64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub ops_per_sec: f64,
    pub artifacts_per_sec: f64,
}

pub fn percentile(sorted_samples: &[f64], p: f64) -> f64 {
    if sorted_samples.is_empty() {
        return 0.0;
    }
    let k = (sorted_samples.len() - 1) as f64 * p;
    let lo = k.floor() as usize;
    let hi = (lo + 1).min(sorted_samples.len() - 1);
    let frac = k - lo as f64;
    sorted_samples[lo] + (sorted_samples[hi] - sorted_samples[lo]) * frac
}

pub struct SampleInput {
    pub name: String,
    pub kind: String,
    pub bytes: usize,
    pub iterations: usize,
    pub artifacts_per_run: usize,
    pub unique_per_run: usize,
    pub wall_seconds: f64,
    pub samples: Vec<f64>,
}

pub fn row_from_samples(input: SampleInput) -> SampleRow {
    let SampleInput {
        name,
        kind,
        bytes,
        iterations,
        artifacts_per_run,
        unique_per_run,
        wall_seconds,
        mut samples,
    } = input;
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let total_artifacts = artifacts_per_run * iterations;
    SampleRow {
        name,
        kind,
        tier: tier_of(bytes).to_string(),
        bytes,
        iterations,
        artifacts_per_run,
        unique_per_run,
        wall_seconds,
        mean_ms: mean * 1000.0,
        p50_ms: percentile(&samples, 0.50) * 1000.0,
        p95_ms: percentile(&samples, 0.95) * 1000.0,
        p99_ms: percentile(&samples, 0.99) * 1000.0,
        min_ms: samples.first().copied().unwrap_or(0.0) * 1000.0,
        max_ms: samples.last().copied().unwrap_or(0.0) * 1000.0,
        ops_per_sec: iterations as f64 / wall_seconds,
        artifacts_per_sec: total_artifacts as f64 / wall_seconds,
    }
}

pub fn print_single_table(rows: &[SampleRow]) {
    print_fixture_rows(rows);
    print_tier_rollups(rows);
}

pub fn print_corpus_summary(rows: &[SampleRow]) {
    print_pack_table(rows);
    print_slowest(rows, 10);
    print_tier_rollups(rows);
}

fn print_fixture_rows(rows: &[SampleRow]) {
    let header = format!(
        "{:<42} {:<8} {:>9} {:>6} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "fixture", "kind", "size", "n", "mean", "p50", "p95", "p99", "arts/s"
    );
    println!();
    println!("{header}");
    println!("{}", "-".repeat(header.len()));
    for row in rows {
        println!(
            "{:<42} {:<8} {:>9} {:>6} {:>7.3}ms {:>7.3}ms {:>7.3}ms {:>7.3}ms {:>10.0}",
            fit(&row.name, 42),
            row.kind,
            human_bytes(row.bytes),
            row.iterations,
            row.mean_ms,
            row.p50_ms,
            row.p95_ms,
            row.p99_ms,
            row.artifacts_per_sec
        );
    }
    println!("{}", "-".repeat(header.len()));
}

fn pack_name(name: &str) -> &str {
    name.split('/').next().unwrap_or(name)
}

fn fit(name: &str, width: usize) -> String {
    if name.chars().count() <= width {
        return name.to_string();
    }
    let mut out: String = name.chars().take(width.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn print_pack_table(rows: &[SampleRow]) {
    let mut packs: std::collections::BTreeMap<&str, Vec<&SampleRow>> =
        std::collections::BTreeMap::new();
    for row in rows {
        packs.entry(pack_name(&row.name)).or_default().push(row);
    }

    let header = format!(
        "{:<42} {:>6} {:>9} {:>10} {:>10}",
        "pack", "cases", "mean", "p95", "arts/s"
    );
    println!();
    println!("{header}");
    println!("{}", "-".repeat(header.len()));
    let mut ranked: Vec<_> = packs.into_iter().collect();
    ranked.sort_by(|a, b| {
        let mean_a = weighted_mean(a.1.as_slice());
        let mean_b = weighted_mean(b.1.as_slice());
        mean_b.partial_cmp(&mean_a).unwrap()
    });
    for (pack, pack_rows) in ranked {
        let total_wall: f64 = pack_rows.iter().map(|row| row.wall_seconds).sum();
        let total_arts: usize = pack_rows
            .iter()
            .map(|row| row.artifacts_per_run * row.iterations)
            .sum();
        let p95 = pack_rows
            .iter()
            .map(|row| row.p95_ms)
            .fold(0.0_f64, f64::max);
        println!(
            "{pack:<42} {:>6} {:>7.3}ms {:>8.3}ms {:>10.0}",
            pack_rows.len(),
            weighted_mean(pack_rows.as_slice()),
            p95,
            total_arts as f64 / total_wall
        );
    }
    println!("{}", "-".repeat(header.len()));
}

fn weighted_mean(rows: &[&SampleRow]) -> f64 {
    let total_n: usize = rows.iter().map(|row| row.iterations).sum();
    rows.iter()
        .map(|row| row.mean_ms * row.iterations as f64)
        .sum::<f64>()
        / total_n as f64
}

fn print_slowest(rows: &[SampleRow], count: usize) {
    let mut ranked: Vec<&SampleRow> = rows.iter().collect();
    ranked.sort_by(|a, b| b.mean_ms.partial_cmp(&a.mean_ms).unwrap());
    ranked.truncate(count);
    println!();
    println!("slowest {count} fixtures:");
    print_fixture_rows(&ranked.into_iter().cloned().collect::<Vec<_>>());
}

pub fn print_load_table(rows: &[SampleRow]) {
    let header = format!(
        "{:<42} {:>8} {:>8} {:>6} {:>10} {:>10} {:>12}",
        "document", "items", "unique", "n", "mean", "arts/s", "unique/s"
    );
    println!();
    println!("{header}");
    println!("{}", "-".repeat(header.len()));
    for row in rows {
        let unique_per_sec = (row.unique_per_run * row.iterations) as f64 / row.wall_seconds;
        println!(
            "{:<42} {:>8} {:>8} {:>6} {:>8.2}ms {:>10.0} {:>12.0}",
            row.name,
            row.artifacts_per_run,
            row.unique_per_run,
            row.iterations,
            row.mean_ms,
            row.artifacts_per_sec,
            unique_per_sec
        );
    }
    println!("{}", "-".repeat(header.len()));
}

fn print_tier_rollups(rows: &[SampleRow]) {
    for tier in ["tiny", "small", "medium", "large", "xlarge"] {
        let tier_rows: Vec<&SampleRow> = rows.iter().filter(|row| row.tier == tier).collect();
        if tier_rows.is_empty() {
            continue;
        }
        let total_n: usize = tier_rows.iter().map(|row| row.iterations).sum();
        let total_wall: f64 = tier_rows.iter().map(|row| row.wall_seconds).sum();
        let total_arts: usize = tier_rows
            .iter()
            .map(|row| row.artifacts_per_run * row.iterations)
            .sum();
        let mean = tier_rows
            .iter()
            .map(|row| row.mean_ms * row.iterations as f64)
            .sum::<f64>()
            / total_n as f64;
        println!(
            "{:<42} {:<8} {:>9} {:>6} {:>7.3}ms {:>9} {:>9} {:>9} {:>10.0}",
            format!("tier {tier}"),
            "",
            "",
            total_n,
            mean,
            "",
            "",
            "",
            total_arts as f64 / total_wall
        );
    }

    let total_n: usize = rows.iter().map(|row| row.iterations).sum();
    let total_wall: f64 = rows.iter().map(|row| row.wall_seconds).sum();
    let total_arts: usize = rows
        .iter()
        .map(|row| row.artifacts_per_run * row.iterations)
        .sum();
    println!();
    println!(
        "total: {total_arts} artifacts in {total_wall:.2}s ({:.0} artifacts/s, {total_n} timed runs)",
        total_arts as f64 / total_wall
    );
}
