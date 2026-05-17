# Architecture

## Decisions

- Core first: the Rust library is the source of truth.
- CLI second: the human and CI entrypoint.
- MCP now: the agent entrypoint.
- VS Code and Chrome later: useful surfaces, but not the core product.

## Validation Layers

- Normative: formal specs and official vendor contracts.
- Reference: official templates, SDKs, and vendor tests.
- Heuristic: ecosystem behavior, GTM community templates, observed traffic.

## Fast Path

- Users should name the vendor or rulepack when they can.
- Parse once into normalized artifact types.
- Precompile URL matchers, regexes, and manifests.
- Keep plugins pure and deterministic.
- Avoid runtime scripting in the hot path.

## Current State

- `core` is the only implemented rulepack today.
- `pixellint-cli` exposes local QA and CI flows, including JSON output for machine-readable validation results.
- `pixellint-mcp` exposes the same engine over stdio for agent workflows.

## Plugin Model

- `core`: broad rules from formal specs.
- `vendor/*`: Google, Meta, TikTok, LinkedIn, GTM, VAST, and custom packs.
- External plugins should prefer rulepack manifests and compiled matcher bundles over arbitrary dynamic Rust code. That keeps startup, safety, and compatibility under control.
- All rulepacks should be toggleable. The default flow can auto-run compatible packs, while CLI, CI, and MCP clients should be able to include or exclude specific packs per invocation.
