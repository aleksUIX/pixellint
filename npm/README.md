# pixellint

Validator for pixels, postbacks, conversion API payloads, and tracking URLs,
as a WASM-backed npm package for Node.js 18 or newer. Same engine, same rule ids, and same evidence
levels as the [`pixellint` CLI](https://crates.io/crates/pixellint). Paste a
URL or a CAPI JSON body at [pixellint.org](https://pixellint.org).

```bash
npm install pixellint
```

```js
import { validate, isOk, validateMany } from "pixellint";

const pixel = validate("https://www.facebook.com/tr?ev=Purchase");
isOk(pixel); // false: missing Pixel ID

const capi = validate(
  JSON.stringify({
    data: [{ event_name: "Purchase", event_time: 1770000000000, action_source: "website" }],
  }),
  { kind: "json" },
);
isOk(capi); // false: event_time is milliseconds, Meta wants seconds

const document = validateMany([
  "https://example.com/pixel?id=1#frag",
  "https://example.com/pixel?id=1#frag",
]);
document.summary.unique_artifacts; // 1
```

Every finding carries a stable `code`, a `severity`, a `fix_hint`, the byte
range it applies to, and the document it came from:

```js
const [finding] = pixel.reports.flatMap((report) => report.violations);

finding.severity;         // "error"
finding.source.level;     // "official_vendor"
finding.source.reference; // "https://developers.facebook.com/docs/meta-pixel/get-started"
finding.targets[0];       // { component: "whole_url", start: 0, end: 46, ... }
```

## What it checks

- URL conformance, transport, credentials, fragments, and ad-tech macro handling
- IAB consent signals: TCF `gdpr` and `gdpr_consent`, the deprecated US Privacy
  string, and GPP `gpp` and `gpp_sid`
- 164 vendor rulepacks across 87 vendors, including Meta
  Conversions API, TikTok Events API, Reddit CAPI, and the browser pixels
- Endpoint attribution for 134 vendor rows, so an unrecognized pixel still gets a name.

## API

| Function | Returns |
| --- | --- |
| `validate(artifact, options?)` | The full validation summary |
| `validateMany(document)` | Document report for extracted artifacts |
| `validateSessions(request, options?)` | Explicit ad-session findings, document report and relationship coverage |
| `isOk(summary)` | `false` when any error-severity finding is present |
| `rulepacks()` | Every rulepack with its evidence level |
| `vendors()` | The vendor endpoint directory |
| `vendorForHost(host)` | The vendor that serves a host, or `null` |
| `version()` | The `pixellint-core` version this build wraps |

`options` takes `kind` (`url` by default, plus `json`, `vast`, `postback`, `request`,
`unknown`), `state` (`unknown`, `template`, `fired`), and
`vendor` for a caller's claimed vendor. `html`, `js`, and `gtm` throw: extract
URLs first.

Use `{ kind: "request" }` with a serialized complete capture containing string
`url`, string `method`, `headers` as an object or name/value list, and optional
raw string `body` or `body_base64` containing captured binary bytes. Identity
and gzip decoding share the native engine's bounded inspection profile.
The capture binds payload checks to the destination and adds method and header
contracts. See [the HTTP capture schema](https://github.com/aleksUIX/pixellint/blob/main/docs/HTTP_REQUEST_SCHEMA.md)
for representation limits and repeated fields.

`validateSessions` accepts the [session schema](https://github.com/aleksUIX/pixellint/blob/main/docs/SESSION_SCHEMA.md)
as an object or raw JSON string. The caller supplies actual session membership
using original zero-based artifact indexes. Options include `at` as a safe
integer Unix clock, `rulepacks` and `exceptRulepacks`. The initial IAS UVP
profile checks external-ID consistency and uniqueness. Inspect both
`report.summary.errors` and `report.coverage` when deciding whether the observed
relationships were fully evaluated.

`vast` takes a tracking URL extracted from VAST and enables the IAB VAST
macro-name checks. It does not accept a VAST XML document.

Pass `{ state: "template" }` to allow unexpanded macros in a template.
Macro syntax, position, and VAST macro-name checks still apply.

## Browser integration

This npm wrapper loads the Node-target WASM build. Its ESM entry uses
`node:module`, so it cannot be imported directly in a browser. Build
`pixellint-wasm` for a browser target and provide a browser loader.

## Links

- [Playground](https://pixellint.org)
- [Conversion API validator](https://pixellint.org/docs/conversion-api-validator/)
- [Pixel not firing](https://pixellint.org/docs/pixel-not-firing/)
- [Rule inventory](https://github.com/aleksUIX/pixellint/blob/main/docs/STANDARDS.md)
- [Writing a rulepack](https://github.com/aleksUIX/pixellint/blob/main/docs/RULEPACK_SCHEMA.md)

Apache-2.0. Not affiliated with any vendor named in its rulepacks.

## HAR requests use recorded clocks and explicit availability.

```js
import { importHar, validateHar } from "pixellint";
const imported = importHar(harText, { headerPolicy: "chrome_sanitized" });
const report = validateHar(harText, { headerPolicy: "chrome_sanitized" });
```

HAR input stays offline. Default replay uses each capture's startedDateTime;
`at` overrides it with safe integer Unix seconds. Missing/invalid capture clocks
require an explicit override. The default `unknown` header policy treats absent
Authorization/Cookie as unavailable. `chrome_sanitized` records their omission
as redaction; `complete` establishes absent headers and enables missing-field
findings. Observable methods, headers and bodies retain their checks.

Import and replay results preserve original URLs, headers and bodies and can
contain credentials. They are private capture data, not sanitized output.
See [the HAR request schema](https://github.com/aleksUIX/pixellint/blob/main/docs/HAR_REQUEST_SCHEMA.md)
for body encoding, byte limits, deduplication and provenance details.
