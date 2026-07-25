# Roadmap

What Pixellint does today and where it is heading. Not a promise of dates.

## Shipped

- Rust core with a plugin engine, stable rule ids, typed severities, evidence
  levels, documentation citations, and byte-offset targets on findings
- `core` rulepack: URL validity and host presence, transport scheme, embedded
  credentials, ignored fragments, insecure transport, and generic ad-tech macro
  handling (unexpanded macros in fired URLs, unsafe macro placement, mixed
  macro syntax)
- Declarative rulepack manifests: host and path matchers, parameter contracts,
  value formats, cross-parameter rules, and load-time validation including a
  citation requirement for vendor-documented rules
- Five first-party vendor packs: Meta Pixel, GA4 Measurement Protocol,
  Campaign Manager Floodlight, TikTok Pixel, LinkedIn image pixel
- Custom rulepacks from disk, the same format the first-party packs use
- CLI with JSON output, stdin and file input, rulepack selection, and CI-ready
  exit codes
- MCP server over stdio with live rulepack ids in its tool schema

## Next

- More endpoint families: Google Ads conversion pixels, the `g/collect` gtag
  transport, Snap, Reddit, Pinterest, X
- Consent and privacy signal checks where vendors document them: `gdpr`,
  `gdpr_consent`, `npa`, `us_privacy`, GPP
- Document-level results: the wrapper described in
  [docs/MULTI_ARTIFACT_SCHEMA.md](docs/MULTI_ARTIFACT_SCHEMA.md), so callers
  that extract many artifacts get one coherent report
- Deterministic autofix for the mechanical findings, behind an explicit flag
- WASM bindings and an npm package, then a browser playground
- Duplicate and conflict detection across a set of artifacts

## Later

- Vendor packs contributed and maintained by the vendors themselves
- GitHub Action, pre-commit hook, Homebrew tap, Docker image, prebuilt binaries
- Editor integration that lints tracking URLs in place
- Network mode: replay a HAR or captured request stream and lint what fired
- Python and Go bindings over the Rust core

## Non-goals

- Firing requests to check whether an endpoint responds
- Enforcing a vendor's business policy beyond what it documents
- Redistributing vendor documentation text; rulepacks carry structured
  metadata and links, not prose
- Owning every vendor forever. First-party packs cover the common families;
  the long tail belongs in custom packs
