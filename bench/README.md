# Pixellint benchmark

Measures `pixellint-core` in-process: that is the number to move when you
speed the engine up. Process spawn is optional and is dominated by startup
on typical pixel URLs.

Playground D1 rows are not copied into the repo (they can still carry
campaign identifiers). The synth set follows the same shapes: Meta / GA4 /
Floodlight / LinkedIn / TikTok / CM360 VAST trackers, CAPI JSON, unknown
hosts, templates, fat query strings, and 1,000-event batches. Load documents
mimic a Vastlint extract and a playground dump.

## Quick start

```bash
# smoke: golden corpus + synth + small validate-many batches
cargo run -p pixellint-bench --release -- --quick

# default: ~100k single validates, then validate-many up to 10,000 unique
cargo run -p pixellint-bench --release

# huge unique batch (100,000 artifacts) on top of the default load set
cargo run -p pixellint-bench --release -- --mode load --heavy --batch-size 50000
```

`bench/run.sh` is the same as the `cargo run` line.

The runner builds `Engine::default()` once, prints construct time, then times
`Engine::validate` and `Engine::validate_many`. `--save` writes JSON under
`bench/results/` (gitignored).

## Modes

| Mode | What it times |
|------|----------------|
| `corpus` | Every golden fixture in `fixtures/` (934 today) |
| `synth` | D1-shaped singles plus stress payloads (fat URLs, CAPI x1000) |
| `load` | `validate_many` on VAST-shaped, playground-shaped, duplicate-heavy, and unique batches. Also a loop of `validate` on the playground dump so wrapper cost is visible. |
| `cli` | One `pixellint` process per synth fixture |
| `cli-many` | One process per load document (`validate-many`) |

Default modes: `corpus,synth,load`. Repeat `--mode` or pass a comma list.

## Options

| Flag | Meaning |
|------|---------|
| `--quick` | 5 runs on small fixtures, 500 unique in the large load doc |
| `--heavy` | add a 100,000 unique-artifact `validate_many` |
| `--per-fixture N` | base timed runs per single fixture (large payloads scale down) |
| `--batch-size N` | unique artifacts in the large load document (default 10,000) |
| `--warmup N` | untimed runs (default 3; `--quick` uses 1) |
| `--filter STR` | only fixtures whose name contains STR |
| `--write-fixtures DIR` | dump synth `.txt` and load `.json` documents |
| `--save` | write `bench/results/bench-<unix>.json` |
| `--json` | print that report to stdout |
| `--verbose` | print every golden fixture instead of per-pack + slowest 10 |
| `--cli PATH` | `pixellint` binary for `cli` / `cli-many` |

Large synth fixtures automatically take fewer timed runs than a 100-byte pixel.

## Reading the numbers

- **engine construct** is pack compile + matcher build. Paid once per process.
- Corpus and synth tables are per artifact. `arts/s` is validations per second.
- Load table is per document. `arts/s` counts every extracted URL, `unique/s`
  counts after `validate_many` dedupe. `dup-10000-200-unique` should show
  a large gap between those two if dedupe is doing its job.
- `d1-playground-N-loop` is the same dump without the document wrapper. If
  it is close to `d1-playground-N`, `validate_many` overhead is small.
- `--mode cli` includes process start. On small pixels that is most of the
  sample. Use it to sanity-check the CLI, not to tune the engine.

## Comparing two builds

```bash
cargo build --release -p pixellint-bench
cp target/release/pixellint-bench /tmp/pixellint-bench-before
# ...change the engine, rebuild...
/tmp/pixellint-bench-before --quick --save
cargo run -p pixellint-bench --release -- --quick --save
```

CI does not run the full bench. `cargo test -p pixellint-bench` walks the
corpus, runs every synth fixture once, and `validate_many`s a tiny document
so the harness cannot rot. `--verbose` prints every golden fixture row;
the default corpus view is per-pack plus the slowest ten.
