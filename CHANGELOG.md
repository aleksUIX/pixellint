# Changelog

All notable changes to Pixellint are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.0 - 2026-07-26

Consent and privacy signals.

### Added

- Eleven `core` rules for the IAB consent signals, checked against the specs
  that define them: TCF v2 `gdpr` and `gdpr_consent` coherence and format, US
  Privacy string format plus its January 2024 deprecation, GPP string and
  section id format, and the specs' single-occurrence requirement
- Signals are read from Floodlight-style path parameters as well as the query
  string
- Meta Limited Data Use parameters `dpo`, `dpoco`, and `dpost`, with a rule for
  Meta's requirement that a country is sent with a state
- `required_when_value` rule kind for manifests: a requirement that applies only
  when another parameter carries a given value

### Notes

- Values carrying an unexpanded macro and empty values never trigger a privacy
  finding. Both are normal in templates that an ad server fills at serve time

## 0.3.0 - 2026-07-26

### Added

- Vendor endpoint directory: 89 vendors across 217 hosts, covering social,
  search, programmatic, identity, verification, measurement, analytics,
  martech, affiliate, mobile attribution, and consent platforms. An endpoint no
  rulepack covers now reports `directory.no_rulepack_coverage` at info severity
  with the vendor that owns it
- `pixellint list-vendors`, with `--json`
- `list_vendors` MCP tool, filterable by category or resolving a single host
- `Engine::set_directory` and `VendorDirectory::from_path`, so callers can
  supply or disable attribution
- `directory` is togglable like a rulepack through `--rulepack` and `--except`

### Notes

- Directory entries make one claim, that a host belongs to a vendor. They carry
  no parameter contracts, and attribution never changes an exit code

## 0.2.0 - 2026-07-26

Coverage release. Twelve rulepacks, up from six.

### Added

- `vendor/google-tag-manager`: container and tag loader requests, covering
  `gtm.js`, `gtag/js`, and the `ns.html` noscript iframe
- `vendor/google-analytics-collect`: the browser `/g/collect` transport the
  Google tag actually uses, including a check that catches Universal Analytics
  hits still pointed at a dead property
- `vendor/pinterest`: Pinterest tag requests with the documented event set
- `vendor/microsoft-uet`: Microsoft Advertising Universal Event Tracking
- `vendor/reddit`: Reddit Pixel conversion requests
- `vendor/meta-conversions-api`: the Graph API events edge, including a warning
  when `test_event_code` reaches live traffic and a raw-email guard
- `vendor/snapchat`: Snapchat Conversions API events endpoint

### Changed

- The CLI rulepack listing test now derives its expectations from the built-in
  pack list, so adding a pack cannot leave a stale assertion behind

## 0.1.0 - 2026-07-25

First release.

### Added

- `pixellint-core`: validation engine with rulepack plugins, stable rule ids,
  typed severities, evidence levels, documentation citations, and byte-offset
  targets on findings
- `core` rulepack with ten spec-backed and baseline rules covering URL
  validity, transport, credentials, fragments, empty input, and ad-tech macro
  handling
- Declarative rulepack manifests, with load-time validation of matchers,
  parameter contracts, regular expressions, rule codes, cross-references, and
  documentation citations
- First-party vendor packs: `vendor/meta`, `vendor/google-analytics`,
  `vendor/floodlight`, `vendor/tiktok`, `vendor/linkedin`
- Custom rulepack loading from disk via `--rulepack-file` and
  `Engine::register_manifest_path`
- `pixellint` CLI: `validate` and `list-rulepacks`, JSON output, inline, file,
  and stdin input, rulepack selection, vendor hints, and documented exit codes
- `pixellint-mcp`: MCP server over stdio exposing `list_rulepacks` and
  `validate_artifact`, with live rulepack ids in the tool schema and detected
  vendors in every response
- Golden fixture corpus with one directory per rulepack, plus integration tests
  that drive the real CLI binary and the real MCP transport
