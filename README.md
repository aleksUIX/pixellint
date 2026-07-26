# Pixellint

[![Rust](https://github.com/aleksUIX/pixellint/actions/workflows/rust.yml/badge.svg)](https://github.com/aleksUIX/pixellint/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/pixellint.svg)](https://crates.io/crates/pixellint)
[![docs.rs](https://img.shields.io/docsrs/pixellint-core)](https://docs.rs/pixellint-core)
[![license](https://img.shields.io/crates/l/pixellint.svg)](LICENSE)

Pixellint is a spec-first validator for pixels, postbacks, conversion API
payloads, and other measurement artifacts. It runs in a terminal, in CI, and in agents, and every
finding carries a stable id, a severity, an evidence level, and the document it
came from.

Broken pixels cost attribution, QA time, and campaign money. The tooling that
exists is fragmented: vendor-specific helpers, enterprise tag auditors, and
schema linters that know nothing about ad tech. Pixellint is the open
validation layer underneath all of that.

```bash
$ pixellint validate url 'https://www.facebook.com/tr?ev=Purchse&em=buyer@example.com'
rulepack: core
  ok
rulepack: vendor/meta (vendor: meta)
  error   vendor.meta.param.id.missing   `id` is required on Meta Pixel requests but is not present. It is the numeric Pixel ID from Events Manager.
    fix: Set `id` to the numeric Pixel ID shown in Events Manager.
    docs: https://developers.facebook.com/docs/meta-pixel/get-started
  warning vendor.meta.param.ev.invalid   `ev` is `Purchse`, which is not one of the documented values: PageView, AddPaymentInfo, ...
    fix: Use a standard event name, or confirm the custom event is registered in Events Manager.
    docs: https://developers.facebook.com/docs/meta-pixel/reference
  error   vendor.meta.pii.unhashed_email A parameter carries what looks like a raw email address. Meta requires customer information to be normalized and SHA-256 hashed before it is sent.
    fix: Normalize the value, hash it with SHA-256, and send the hex digest instead of the plain address.
    docs: https://developers.facebook.com/docs/meta-pixel/advanced/advanced-matching

2 error(s), 1 warning(s), 0 info message(s) across 2 rulepack(s).
```

Server-side events are checked in the body, one event at a time:

```bash
$ pixellint validate json '{"data":[{"event_name":"Purchase","event_time":1770000000000,
    "action_source":"website","user_data":{"em":"buyer@example.com"}}]}'
rulepack: vendor/meta-conversions-api (vendor: meta)
  error   vendor.meta-conversions-api.body.event_time.invalid `event_time` must be at most 10 digits, but `1770000000000` has 13. Meta documents it as a Unix timestamp in seconds...
    fix: Send seconds, not milliseconds: divide a JavaScript `Date.now()` by 1000 and floor it.
  error   vendor.meta-conversions-api.body.purchase_requires_value_and_currency A `Purchase` event is missing `custom_data.value` or `custom_data.currency`.
  error   vendor.meta-conversions-api.body.website_requires_source_url The event is marked `action_source: website` but carries no `event_source_url`.
```

## Install

```bash
cargo install pixellint          # CLI
cargo install pixellint-mcp      # MCP server
npm install pixellint            # library, WASM-backed
```

Or paste a URL into the playground at [pixellint.org](https://pixellint.org),
which runs the same engine in your browser and sends nothing anywhere.

## Use it

### QA

Validate an inline artifact, a file with `@path`, or stdin with `-`:

```bash
pixellint validate url 'https://px.ads.linkedin.com/collect?pid=123456&fmt=gif'
pixellint validate postback @conversion-endpoint.txt --json
curl -s "$TAG_URL" | pixellint validate vast -
```

If the artifact is still a template with unexpanded macros, say so, and macro
rules adjust:

```bash
pixellint validate url 'https://example.com/pixel?cb=[CACHEBUSTING]' --state template
```

### CI

`pixellint validate` exits `0` when the artifact is clean or only produced
warnings, `1` when any error-severity finding is present, and `2` on a usage or
input problem. That makes it usable as a gate:

```yaml
- name: Validate tracking artifacts
  run: |
    cargo install pixellint
    pixellint validate url @fixtures/conversion-pixel.txt
```

### Agents

```bash
cargo install pixellint-mcp
```

`pixellint-mcp` speaks MCP over stdio and exposes three tools:

- `list_rulepacks`
- `list_vendors`, optionally filtered by `category` or attributing a single `host`
- `validate_artifact`, taking `artifact_kind`, `artifact`, and optional
  `claimed_vendor`, `expansion_state`, `rulepacks`, `except_rulepacks`

Responses include the structured findings, the detected vendors, and a
severity summary, so an agent can act on the result without parsing prose.

### Node and the browser

```js
import { validate, isOk } from "pixellint";

const summary = validate("https://www.facebook.com/tr?ev=Purchase");
isOk(summary); // false, the pixel id is missing
```

### Library

```rust
use pixellint_core::{ArtifactKind, Engine, ExpansionState, ValidationOptions, ValidationRequest};

let engine = Engine::default();
let request = ValidationRequest {
    artifact_kind: ArtifactKind::Url,
    artifact: "https://www.facebook.com/tr?id=1234567890123456&ev=PageView".to_string(),
    claimed_vendor: None,
    expansion_state: ExpansionState::Unknown,
};

let summary = engine.validate(&request, &ValidationOptions::default())?;
assert!(summary.is_ok());
```

## Rulepacks

`core` runs on every URL-like artifact. Vendor packs run only when the artifact
targets their endpoints, so you get vendor checks without asking for them, and
nothing fires on an endpoint it does not understand. A conversion API body has
no endpoint to go by, so those packs claim it by the shape of the payload.

| Rulepack | Covers | Evidence |
| --- | --- | --- |
| `core` | URL validity, transport, credentials, fragments, ad-tech macro handling, IAB consent signals | normative |
| `vendor/meta` | `facebook.com/tr` pixel requests | official vendor |
| `vendor/meta-conversions-api` | Graph API events edge, URL and JSON event payload | official vendor |
| `vendor/google-analytics` | GA4 Measurement Protocol, URL and JSON event payload | official vendor |
| `vendor/google-analytics-collect` | The `/g/collect` transport the Google tag uses in the browser | ecosystem reference |
| `vendor/google-tag-manager` | `gtm.js`, `gtag/js`, and `ns.html` loader requests | official vendor |
| `vendor/google-ads-conversion` | Google Ads conversion and view-through pixels | official vendor |
| `vendor/floodlight` | Campaign Manager Floodlight activity tags | official vendor |
| `vendor/adobe-analytics` | Adobe Analytics data collection beacons | official vendor |
| `vendor/pinterest` | Pinterest tag requests and the noscript fallback | official vendor |
| `vendor/snapchat` | Snap Conversions API v3, URL and JSON event payload | official vendor |
| `vendor/tiktok` | TikTok Pixel loader and collection requests | ecosystem reference |
| `vendor/linkedin` | LinkedIn conversion image pixels | ecosystem reference |
| `vendor/linkedin-conversions-api` | LinkedIn conversion events, single and batched | official vendor |
| `vendor/microsoft-uet` | Microsoft Advertising Universal Event Tracking | ecosystem reference |
| `vendor/reddit` | Reddit Pixel conversion requests | ecosystem reference |

### Vendor directory

Rulepacks cover fifteen endpoint families in depth. The vendor directory covers
the rest by attribution: 89 vendors and 217 hosts, so an unrecognized pixel
still gets a name.

```bash
$ pixellint validate url 'https://trc.taboola.com/actions?a=1'
rulepack: directory (vendor: taboola)
  info  directory.no_rulepack_coverage  This endpoint belongs to Taboola (native). No Pixellint rulepack covers it, so only the core checks ran.
```

Directory entries make one claim, that a host belongs to a vendor. They carry
no parameter contracts, the finding is always `info`, and attribution never
changes an exit code. See [docs/VENDOR_DIRECTORY.md](docs/VENDOR_DIRECTORY.md).

```bash
pixellint list-vendors
pixellint list-vendors --json
```

Select packs per run:

```bash
pixellint validate url "$ARTIFACT" --rulepack core
pixellint validate url "$ARTIFACT" --except vendor/meta
pixellint list-rulepacks --json
```

Full rule inventory with citations: [docs/STANDARDS.md](docs/STANDARDS.md).

### Evidence levels

Findings say where their authority comes from, and you can hold packs to
different standards accordingly:

- `normative`: a formal standard such as the WHATWG URL Standard or an RFC
- `official_vendor`: a parameter contract the vendor publishes, with the URL
- `official_template`: vendor-published templates or SDK behavior
- `ecosystem_reference`: consistent real-world behavior the vendor generates in
  its own UI but does not document
- `heuristic`: Pixellint's judgment, labeled as such

The manifest loader refuses to compile a pack that claims `official_vendor`
without citing documentation.

## Custom rulepacks

Rulepacks are data, not code. First-party vendor packs are written in the same
JSON format you can write for an internal endpoint:

```json
{
  "id": "custom/acme",
  "display_name": "Acme internal pixel",
  "description": "Contract for the internal conversion endpoint.",
  "vendor": "acme",
  "source_level": "heuristic",
  "match": { "hosts": ["px.acme.example"], "path_prefixes": ["/collect"] },
  "params": [
    { "name": "aid", "requirement": "required", "format": { "kind": "integer" } },
    { "name": "ev", "requirement": "required", "format": { "kind": "enum", "values": ["view", "purchase"] } }
  ]
}
```

```bash
pixellint validate url 'https://px.acme.example/collect?ev=purchase' --rulepack-file acme.json
```

The format is documented in [docs/RULEPACK_SCHEMA.md](docs/RULEPACK_SCHEMA.md).

## What Pixellint does not do

- It does not extract artifacts from documents. Callers such as Vastlint parse
  VAST, HTML, or GTM containers and hand Pixellint the URLs they found. The
  planned document-level result model is in
  [docs/MULTI_ARTIFACT_SCHEMA.md](docs/MULTI_ARTIFACT_SCHEMA.md).
- It does not fire requests or check whether an endpoint responds.
- It does not auto-fix. Findings carry fix hints; applying them is on you.

## Evidence

Every rule is backed by tests: unit tests in `crates/pixellint-core/src/`, a
golden corpus in `fixtures/` with one directory per rulepack, and integration
tests that drive the real CLI binary and the real MCP stdio transport.

```bash
cargo test --workspace
```

## Docs

- [Standards and rule inventory](docs/STANDARDS.md)
- [Vendor directory](docs/VENDOR_DIRECTORY.md)
- [npm package](npm/)
- [Rulepack manifest schema](docs/RULEPACK_SCHEMA.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Multi-artifact output model](docs/MULTI_ARTIFACT_SCHEMA.md)

## License

Apache-2.0. Pixellint is not affiliated with or endorsed by any vendor named
in its rulepacks; see [NOTICE](NOTICE).
