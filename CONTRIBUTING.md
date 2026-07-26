# Contributing to Pixellint

Thanks for helping make measurement plumbing less painful.

## Dev setup

Rust 1.88+ and cargo. The whole workspace builds with:

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

All three must pass; CI enforces them.

## Repo layout

- `crates/pixellint-core`: the validation engine, the `core` rulepack, and the
  declarative manifest loader
- `crates/pixellint-core/rulepacks/`: first-party vendor manifests, compiled in
  with `include_str!`
- `crates/pixellint-cli`: the `pixellint` binary
- `crates/pixellint-mcp`: MCP server over stdio
- `crates/pixellint-wasm`: wasm-bindgen bindings
- `npm/`: the Node package, wrapping a committed WASM build in `npm/wasm/`
- `site/`: the pixellint.org playground, wrapping a committed WASM build in
  `site/wasm/`
- `fixtures/`: golden corpus, one directory per rulepack

## The committed WASM builds

`npm/wasm/` and `site/wasm/` are build output kept in the repository so the
package and the site work without a Rust toolchain. Regenerate both when
`pixellint-core` changes:

```bash
wasm-pack build crates/pixellint-wasm --target nodejs --out-dir ../../npm/wasm --out-name pixellint
wasm-pack build crates/pixellint-wasm --target web --out-dir ../../site/wasm --out-name pixellint
rm -f npm/wasm/package.json npm/wasm/README.md npm/wasm/.gitignore
rm -f site/wasm/package.json site/wasm/README.md site/wasm/.gitignore
node npm/test.mjs
```

CI rebuilds them from source and runs the package tests against the fresh
output, and both the release and the site deploy build fresh rather than
trusting what is committed.

## Adding a vendor rulepack

Vendor packs are data. You do not need to write Rust.

1. Read the vendor's documentation first and keep the URL. A rule with no
   citation does not land.
2. Add `crates/pixellint-core/rulepacks/vendor/<name>.json`. Start from an
   existing pack; the schema is documented in
   [docs/RULEPACK_SCHEMA.md](docs/RULEPACK_SCHEMA.md).
3. Register it in `BUILTIN_VENDOR_MANIFESTS` in
   `crates/pixellint-core/src/lib.rs`.
4. Add `fixtures/vendor-<name>/` with at least two clean artifacts and one
   artifact per rule you added, plus a `manifest.json` listing the expected
   findings. `every_builtin_rulepack_has_a_fixture_directory` fails without it.
5. Document the pack in [docs/STANDARDS.md](docs/STANDARDS.md), including its
   evidence level.

Test with:

```bash
cargo run -p pixellint -- validate url '<a real artifact>' --json
```

## Evidence levels are not decoration

Every rule declares where it came from:

- `normative`: a formal standard (WHATWG, RFC, W3C)
- `official_vendor`: a documented vendor contract, with the doc URL
- `official_template`: vendor-published templates or SDK behavior
- `ecosystem_reference`: consistent real-world behavior the vendor has not
  documented
- `heuristic`: Pixellint's own judgment

The loader rejects an `official_vendor` rule that cites no documentation URL.
If you cannot find the doc, drop the level rather than inventing the citation.
Do not upgrade a level to make a finding sound stronger.

## Severity guidance

- `error`: the artifact will not work as intended, or the vendor documents the
  requirement as mandatory
- `warning`: it works but is likely wrong, deprecated, or lossy
- `info`: context the caller asked for, such as a claimed vendor that does not
  match the endpoint

When a documented requirement is ambiguous, prefer `warning` over `error`.

## Custom rulepacks

You do not have to upstream a pack to use one. Any manifest works with
`--rulepack-file`, and `Engine::register_manifest_path` does the same from
Rust. Pixellint aims to cover common endpoint families first-party and leave
the long tail to custom packs.

## Commit and PR style

Small commits, present-tense subject lines, and a body that explains why.
Every behavior change needs a test.
