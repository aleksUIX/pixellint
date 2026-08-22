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

- `core` is hand-written Rust and covers spec-level URL, macro, and IAB consent checks.
- Vendor packs are declarative manifests compiled into `pixellint-core` and interpreted by one `ManifestRulePack` plugin. Thirty-five ship today, including conversion API JSON bodies where the vendor documents them.
- A host directory attributes the long tail. It names the vendor and, when a first-party pack exists for that vendor, points at it. It never invents parameter rules.
- User packs load from disk through `--rulepack-file` or `Engine::register_manifest_path`, using the same schema as the first-party packs.
- The `pixellint` CLI exposes local QA and CI flows with JSON output and documented exit codes.
- `pixellint-mcp` exposes the same engine over newline-delimited JSON-RPC stdio (`validate_artifact`, `list_rulepacks`, `list_vendors`).
- `pixellint-wasm` and the npm package wrap the same engine for Node and the browser playground.

## Dependencies

The core carries three runtime dependencies, and each one earns its place:
`url` for WHATWG-conformant parsing, `serde`/`serde_json` for the manifest and
result schemas, and `regex` so declarative packs can express value formats
without shipping code. Everything else is std.

## Plugin Model

- `core`: broad rules from formal specs.
- `vendor/*`: Google, Meta, TikTok, LinkedIn, GTM, VAST, and custom packs.
- External plugins should prefer rulepack manifests and compiled matcher bundles over arbitrary dynamic Rust code. That keeps startup, safety, and compatibility under control.
- All rulepacks should be toggleable. The default flow can auto-run compatible packs, while CLI, CI, and MCP clients should be able to include or exclude specific packs per invocation.

## Rulepack Ownership

- `core` stays first-party and generic.
- Pixellint should ship first-party vendor rulepacks for the high-volume endpoint families that cover most real QA and trafficking workflows.
- The long tail should remain user-extensible through custom rulepacks rather than being hardcoded into the product.
- Internal and external rulepacks should use the same rulepack model. First-party packs are not a separate architecture; they are maintained instances of the same system users extend.

## Coverage Strategy

- Plan around endpoint families, not vendor logos or template catalogs.
- A single vendor may require multiple rulepacks when its wire-level surfaces differ materially, for example Google Ads, Floodlight, and generic VAST trackers.
- The practical goal is first-party coverage for the common families that get Pixellint to roughly p90 usefulness, while leaving niche, regional, private, or customer-specific integrations to custom packs.
- Coverage expansion should follow observed traffic and real demand, not a scrape of every public vendor library.

## Rulepack Format

- Rulepacks should be declarative before they are programmable.
- Prefer manifests that define host and path matchers, parameter contracts, macro expectations, value validators, and fixture sets.
- Avoid arbitrary user code in the hot path until the declarative model is exhausted.
- The same declarative schema should power bundled Pixellint rulepacks and user-supplied rulepacks.

## Product Shape

- `core` runs by default as the neutral baseline.
- First-party `vendor/*` rulepacks are the product-quality packs Pixellint stands behind.
- User-defined `custom/*` rulepacks cover the long tail without forcing Pixellint to own every vendor forever.
- Explicit rulepack selection should remain available even if later surfaces add auto-detect or compatibility hints.
