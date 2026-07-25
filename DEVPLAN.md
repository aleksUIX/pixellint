# Pixellint 0.1.0 Devplan

One-day plan to take Pixellint from local prototype to a published open source release.

Target date: 2026-07-25

## Definition Of Done

0.1.0 is release ready when all of these are true:

- `github.com/aleksUIX/pixellint` exists, public, with CI green on `main`.
- `pixellint-core`, `pixellint`, and `pixellint-mcp` are live on crates.io at 0.1.0.
- `cargo install pixellint` gives a working binary on a clean machine.
- Five rulepacks ship: `core`, `vendor/google`, `vendor/meta`, `vendor/tiktok`, `vendor/linkedin`.
- Vendor packs are declarative manifests, loadable by users, not hardcoded Rust.
- Every vendor rule cites an official vendor doc URL in its `RuleSource`.
- `cargo fmt --all --check`, `cargo test --workspace`, and `cargo clippy --all-targets --all-features -- -D warnings` all pass.
- README, STANDARDS, ROADMAP, CHANGELOG, LICENSE, NOTICE, CONTRIBUTING, SECURITY are accurate and present.

Out of scope today: WASM crate, npm package, pixellint.org playground, MCP over HTTP, autofix, Homebrew, Docker, VS Code, Chrome. Those go in ROADMAP.md as Next and Later.

## Verified Starting State

Checked 2026-07-25:

- Workspace builds, 23 tests pass (22 unit, 1 golden over 20 fixtures).
- `core` rulepack implements 10 rules across input, URL, and macro checks.
- `cargo fmt --all --check` fails on one CLI line. `cargo clippy` emits 2 warnings in `pixellint-mcp` (`collapsible_if`).
- Repo has one `init` commit, no remote, and an uncommitted `docs/ARCHITECTURE.md` diff.
- Crate names `pixellint`, `pixellint-core`, `pixellint-cli`, `pixellint-mcp` are all free on crates.io.
- `gh` is authed as `aleksUIX` with `repo` and `workflow` scopes.
- The local `~/.cargo/credentials.toml` token is dead. All publishing must run in CI with the `CARGO_REGISTRY_TOKEN` secret.
- pixellint.org already serves a coming-soon Worker from `pixellint-infra`. Untouched today.

## Decisions Locked Before Coding

1. **CLI package renames from `pixellint-cli` to `pixellint`**, binary `pixellint`. The name is free and `cargo install pixellint` is the expected install line. Directory stays `crates/pixellint-cli`.
2. **Vendor packs are data, not code.** One `ManifestRulePack` plugin type interprets a manifest. First-party packs are embedded with `include_str!`; user packs load from a path.
3. **Rule ids are namespaced** `vendor.<vendor>.<family>.<rule>`, for example `vendor.meta.pixel.missing_id`. Core ids stay as they are.
4. **Vendor packs only run when their host matchers hit**, via `supports()`. Passing `--rulepack vendor/meta` forces a pack on for claimed-vendor QA.
5. **Every vendor rule needs a citation.** No rule ships with `RuleSource.reference: None` at `OfficialVendor` level. If the doc cannot be found, the rule becomes `Heuristic` or gets dropped.

## Phase 0 · Baseline Hygiene (30 min)

- [ ] `cargo fmt --all`
- [ ] Fix the 2 `collapsible_if` warnings in `crates/pixellint-mcp/src/main.rs`
- [ ] Add `#![deny(warnings)]`-equivalent enforcement via CI, not source attributes
- [ ] Commit the pending `docs/ARCHITECTURE.md` rulepack sections

Gate: fmt, test, and clippy with `-D warnings` all clean.

## Phase 1 · Declarative Rulepack Engine (2.5 h)

Build in `crates/pixellint-core/src/manifest.rs`, exported from `lib.rs`.

- [ ] Manifest schema as serde types, JSON on disk:
  - pack identity: `id`, `display_name`, `version`, `description`, `source_level`, `vendor`
  - `match`: host exact list, host suffix list, path prefix or exact list, artifact kinds
  - `params`: per-parameter contract with `name`, `required`, `forbidden`, `deprecated`, `format` (one of `non_empty`, `integer`, `enum`, `regex`, `url`, `email_hash`), `allowed_values`, `severity`, `message`, `fix_hint`, `doc`
  - `rules`: pack-level assertions such as required-one-of groups and mutually exclusive groups
  - `macros`: expected macro tokens per state, plus macros that must never appear fired
- [ ] `ManifestRulePack` implementing `ValidatorPlugin`, mapping every contract violation into the existing `Violation` shape with `targets` byte offsets reused from the core URL parser
- [ ] Manifest validation at load time: unknown fields rejected, bad regex rejected, missing doc URL on an `OfficialVendor` rule rejected
- [ ] `Engine::register_manifest(&str)` and `Engine::register_manifest_path(&Path)` for user packs
- [ ] Generalize `tests/core_rulepack_golden.rs` into a shared fixture harness so each vendor pack gets the same manifest-driven golden coverage
- [ ] Unit tests for the loader itself: malformed manifest, unknown format, duplicate pack id, matcher precedence

Gate: a throwaway test manifest validates a fake vendor URL end to end through `Engine::validate`.

## Phase 2 · Four Vendor Packs (3 h)

For each pack: research the official doc, author the manifest, write 5 or more fixtures (at least 2 clean, 3 failing), register in the embedded pack list, extend STANDARDS.md with the rule table and citations.

Research rule: read the current official doc before authoring. Do not encode remembered parameter names. Record the doc URL in every rule's `doc` field.

- [ ] `vendor/google`: Google Ads conversion and remarketing endpoints plus Floodlight activity URLs. Cover conversion id and label presence, Floodlight `src`/`type`/`cat` triple, cachebuster parameter, `ord` semantics for counting methods.
- [ ] `vendor/meta`: `facebook.com/tr` pixel. Cover `id` presence and numeric format, `ev` presence and standard-event enum, `noscript` handling, `dl` and `rl` expectations, disallowed PII in query values.
- [ ] `vendor/tiktok`: TikTok pixel and events endpoints. Cover pixel or `sdkid` presence, event name enum, required event id for dedup where documented.
- [ ] `vendor/linkedin`: `px.ads.linkedin.com/collect`. Cover partner id presence and numeric format, `conversionId`, `fmt` value set.

Cut line if time runs short: drop `vendor/linkedin` first, then `vendor/tiktok`. Google and Meta carry the launch story. A dropped pack moves to ROADMAP Next, it does not ship half-built.

Gate: `pixellint validate url '<real-world sample>'` produces vendor findings with citations, and every pack has green golden fixtures.

## Phase 3 · Surfaces (1 h)

- [ ] CLI: `--version`, `--help`, keep `--json` stable, document exit codes (0 clean or warnings only, 1 errors present, 2 usage error)
- [ ] CLI: `list-rulepacks` prints the vendor packs with vendor and match summary
- [ ] CLI: accept `--rulepack vendor/meta` repeated, already wired, verify against the new pack ids
- [ ] MCP: `list_rulepacks` returns the new packs, `validate_artifact` input schema documents pack ids, add `detected_vendor` to the response summary
- [ ] MCP: verify the stdio handshake end to end with a scripted request or a real client
- [ ] Auto-detection: host matchers select vendor packs without flags, and `claimed_vendor` mismatch produces an info finding

Gate: one artifact validated identically through CLI JSON and MCP.

## Phase 4 · Repo And Docs (1 h)

- [ ] `LICENSE` (Apache-2.0) and `NOTICE`, matching the rtblint pattern
- [ ] `CONTRIBUTING.md`, `SECURITY.md`
- [ ] `CHANGELOG.md` with a 0.1.0 entry
- [ ] `ROADMAP.md` in Shipped / Next / Later / Non-goals form. Next holds npm and WASM, the pixellint.org playground, MCP over HTTP, autofix, and any dropped vendor pack.
- [ ] README rewrite: what it is, install, three usage surfaces, rulepack table, custom rulepack authoring, evidence and testing story
- [ ] STANDARDS.md updated with all vendor rule tables and citations
- [ ] ARCHITECTURE.md `Current State` section updated to reflect manifests being real
- [ ] Cargo metadata for all three published crates: `description`, `keywords`, `categories`, `readme`, `homepage`, `documentation`

## Phase 5 · CI And Release (1 h)

- [ ] `.github/workflows/rust.yml`: fmt, test, clippy `-D warnings` on push and PR
- [ ] `.github/workflows/release.yml`: `v*` tag triggers verify, then publish in dependency order `pixellint-core`, `pixellint`, `pixellint-mcp`
- [ ] Publish job guards: skip only when the exact version already exists on crates.io, and fail loudly on any other error. Do not treat a nonzero exit as already-published. This is the exact failure that silently lost five vastlint releases.
- [ ] Wait between crate publishes so the index catches up before the dependent crate publishes
- [ ] `gh repo create aleksUIX/pixellint --public --source . --push`
- [ ] `gh secret set CARGO_REGISTRY_TOKEN`
- [ ] Tag `v0.1.0`, push, watch the run

## Phase 6 · Post-Release Verification (30 min)

- [ ] All three crates resolve on crates.io at 0.1.0
- [ ] `cargo install pixellint` in a clean temp dir, then validate a real pixel URL
- [ ] docs.rs build succeeds for `pixellint-core`
- [ ] MCP server runs from the installed binary path against a real client
- [ ] README install and usage lines match what actually shipped

## Risks

- **Vendor doc drift.** Parameter contracts change and undocumented params are common. Mitigation: cite the doc per rule, prefer `Warning` over `Error` where the doc is not explicit, and keep the strictness bar high only for documented requirements.
- **Scope pressure.** Manifest engine plus four packs plus a release pipeline is the aggressive read of one day. The cut line in Phase 2 protects the release date.
- **crates.io publish order.** `pixellint` and `pixellint-mcp` depend on `pixellint-core` by version. The core publish must land in the index before the dependents run.
- **Edition 2024 and rust-version 1.94.** CI stable must be at or above 1.94. Verify on the first CI run rather than at tag time.
- **False confidence from thin rules.** Ten generic URL rules plus four thin vendor packs must not be described as broad coverage. README and STANDARDS state exactly what is enforced and what is not.
