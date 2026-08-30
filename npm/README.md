# pixellint

Validator for pixels, postbacks, conversion API payloads, and tracking URLs,
as a WASM-backed npm package. Same engine, same rule ids, and same evidence
levels as the [`pixellint` CLI](https://crates.io/crates/pixellint). Paste a
URL or a CAPI JSON body at [pixellint.org](https://pixellint.org).

```bash
npm install pixellint
```

```js
import { validate, isOk } from "pixellint";

const pixel = validate("https://www.facebook.com/tr?ev=Purchase");
isOk(pixel); // false: missing Pixel ID

const capi = validate(
  JSON.stringify({
    data: [{ event_name: "Purchase", event_time: 1770000000000, action_source: "website" }],
  }),
  { kind: "json" },
);
isOk(capi); // false: event_time is milliseconds, Meta wants seconds
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
- Vendor parameter contracts for sixty endpoint families, including Meta
  Conversions API, TikTok Events API, Reddit CAPI, and the browser pixels
- Endpoint attribution for 97 vendors, so an unrecognized pixel still gets a name.

## API

| Function | Returns |
| --- | --- |
| `validate(artifact, options?)` | The full validation summary |
| `isOk(summary)` | `false` when any error-severity finding is present |
| `rulepacks()` | Every rulepack with its evidence level |
| `vendors()` | The vendor endpoint directory |
| `vendorForHost(host)` | The vendor that serves a host, or `null` |
| `version()` | The `pixellint-core` version this build wraps |

`options` takes `kind` (`url` by default, plus `json`, `vast`, `postback`, `request`,
`html`, `js`, `gtm`, `unknown`), `state` (`unknown`, `template`, `fired`), and
`vendor` for a caller's claimed vendor.

Templates keep their macros: pass `{ state: "template" }` and unexpanded macros
stop being findings.

## Links

- [Playground](https://pixellint.org)
- [Conversion API validator](https://pixellint.org/docs/conversion-api-validator/)
- [Pixel not firing](https://pixellint.org/docs/pixel-not-firing/)
- [Rule inventory](https://github.com/aleksUIX/pixellint/blob/main/docs/STANDARDS.md)
- [Writing a rulepack](https://github.com/aleksUIX/pixellint/blob/main/docs/RULEPACK_SCHEMA.md)

Apache-2.0. Not affiliated with any vendor named in its rulepacks.
