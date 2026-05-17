# Pixellint

Pixellint is a spec-first validator for pixels, tracking snippets, and measurement artifacts. The market gap is not debugging itself; teams already use vendor helpers, paid tag-audit platforms, and event-governance tools. The gap is the open validation layer that can run in code, CI, editors, and AI agents.

This repo starts with a Rust core that validates cardinal rules grounded in published specs and official vendor contracts. The product stack is: `pixellint-core` first, then `pixellint-cli` and `pixellint-mcp` on top of the same engine. GTM templates are useful inputs, but not the source of truth.

## Why This Exists

- Demand is real: broken pixels cost attribution, QA time, and campaign money.
- Current solutions are fragmented: vendor-specific helpers, enterprise auditors, and schema tools.
- The missing layer is a neutral lint engine for conformance and deterministic fixes.

## Source Of Truth

1. Formal specs and standards from the relevant standards body.
2. Official vendor docs, APIs, and SDK behavior.
3. Official vendor templates and tests.
4. GTM and other template ecosystems as structured hints, not canonical law.
5. Observed traffic and community examples only as heuristic input.

Hard standards implemented today in the `core` rulepack: [docs/STANDARDS.md](docs/STANDARDS.md)
Planned multi-pixel output model: [docs/MULTI_ARTIFACT_SCHEMA.md](docs/MULTI_ARTIFACT_SCHEMA.md)

## Product Shape

- Rust core for fast, deterministic validation.
- Core cardinal rulepack for broad spec-level checks.
- Vendor rulepacks so users choose Google, Meta, TikTok, LinkedIn, GTM, VAST, or custom targets explicitly.
- Rulepacks are toggleable per run, so CI or MCP clients can opt into only the packs they trust or need.
- Thin surfaces on top: CLI and MCP now, editor or browser integrations later.

## Use Today

### QA

Run Pixellint locally against an inline artifact or a file path. Use `--json` when you want stable machine-readable output.

```bash
cargo run --quiet -p pixellint-cli -- validate url 'https://user:pass@example.com/pixel?id=1#frag' --json
```

If the artifact is still a URL template with ad macros, tell Pixellint to validate it as a template rather than a fired request:

```bash
cargo run --quiet -p pixellint-cli -- validate url 'https://example.com/pixel?cb=[CACHEBUSTING]' --state template --json
```

You can also validate a saved artifact by prefixing the path with `@`:

```bash
cargo run --quiet -p pixellint-cli -- validate postback @fixtures/core/clean-postback.txt --json
```

### CI

`pixellint-cli validate` exits non-zero when any error-severity violation is present, which makes it suitable for gating builds in CI.

```yaml
- name: Validate tracking artifact
  run: cargo run --quiet -p pixellint-cli -- validate url @fixtures/core/clean-pixel.txt --json
```

### Agent Workflows

Start the stdio MCP server:

```bash
cargo run --quiet -p pixellint-mcp
```

It exposes two tools today:

- `list_rulepacks`
- `validate_artifact`

`validate_artifact` accepts `artifact_kind`, `artifact`, optional `claimed_vendor`, optional `expansion_state`, optional `rulepacks`, and optional `except_rulepacks`.

Today the available executable rulepack set is only `core`, which provides deterministic baseline checks for URL-like artifacts.

When a finding points at a specific URL component or query parameter, the structured output may include `targets` metadata with component, name, value, and byte offsets.

## Core Evidence

The current `core` rulepack is backed by two test layers:

- rule-level unit tests in `crates/pixellint-core/src/lib.rs`
- a golden fixture corpus in `fixtures/core/`, enforced by `crates/pixellint-core/tests/core_rulepack_golden.rs`

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the plugin and performance shape.
