# Changelog

All notable changes to Pixellint are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## 0.8.0 - 2026-07-26

### Added

- `vendor/meta` requires `value` and `currency` on `Purchase`. Meta documents
  both as required, and a Purchase without them reports no revenue. The
  requirement is the vendor's; the wire spelling of custom data is not
  documented, so the rule carries ecosystem evidence
- `vendor/google-analytics` contracts the Measurement Protocol request body at
  two levels: the envelope once, and each event in `events` on its own. It
  catches `timestamp_micros` in milliseconds rather than microseconds, a `value`
  with no `currency`, and the ecommerce fields Google documents as required for
  `purchase`, `refund`, `add_to_cart`, and `begin_checkout`
- A manifest's `body` may be a list of specs, so one pack can contract both the
  envelope and the elements inside it

### Fixed

- The `clean-purchase` fixture was not clean: it fired a Meta `Purchase` with no
  value or currency, which the new rule reports

## 0.7.0 - 2026-07-26

### Added

- JSON request bodies are a first-class artifact. A new `json` artifact kind,
  and an `unknown` artifact that opens like a document is read as one
- Rulepack manifests can contract a JSON body with `body`, addressing fields by
  path with `[]` for "every element". Contracts are written against one element
  of a batch and evaluated per element, so three broken events report three
  findings, each pointing at its own bytes
- `match.json_paths` claims a payload by its shape, since a bare body carries no
  host. Entries can require a path, exclude values that belong to another
  vendor, or accept any of several alternatives
- `vendor/meta-conversions-api` contracts the documented server event:
  `event_name`, `event_time` in seconds rather than milliseconds, the
  `action_source` enum, the hashed customer information fields, and the rules
  that Purchase needs value and currency, that website events need a source URL,
  and that `client_ip_address` must not arrive hashed
- `vendor/snapchat` contracts the Snap Conversions API v3 payload
- `vendor/linkedin-conversions-api`, covering conversion events sent singly or
  batched under `elements`, including the millisecond timestamp LinkedIn
  requires where Meta requires seconds
- `core.json.parse_error`, which reports a body that does not parse and names
  the byte where it stops

### Changed

- Findings about body fields carry `.body.` in their code rather than `.param.`,
  because an endpoint may accept the same field in the query string and in the
  payload under different rules
- A finding about a missing field points at the container it belongs in and
  names the exact path

## 0.6.0 - 2026-07-26

### Added

- `pixellint-wasm`: wasm-bindgen bindings over the same engine, exposing
  validation, the rulepack list, and the vendor directory
- `pixellint` npm package, WASM-backed, with TypeScript types and a
  dependency-free smoke test suite
- Playground at [pixellint.org](https://pixellint.org), served from `site/` in
  this repository. It runs entirely in the browser and sends nothing anywhere
- CI builds the WASM crate and runs the npm package tests against a fresh build,
  so the committed artifacts stay honest

## 0.5.0 - 2026-07-26

### Added

- `path_pattern` in rulepack manifests: a regular expression with named captures
  run against the path, turning path segments into contractable parameters
- `vendor/google-ads-conversion`: Google Ads conversion and view-through
  conversion image pixels, whose conversion ID rides on the path
- `vendor/adobe-analytics`: Adobe Analytics beacons, whose report suite rides on
  the path after `/b/ss/`

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
