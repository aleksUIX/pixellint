# pixellint

Validator for pixels, postbacks, and tracking URLs, as a WASM-backed npm
package. Same engine, same rule ids, and same evidence levels as the
[`pixellint` CLI](https://crates.io/crates/pixellint).

```bash
npm install pixellint
```

```js
import { validate, isOk } from "pixellint";

const summary = validate("https://www.facebook.com/tr?ev=Purchase");

isOk(summary); // false
summary.reports.flatMap((report) => report.violations).map((v) => v.code);
// ["vendor.meta.param.id.missing"]
```

Every finding carries a stable `code`, a `severity`, a `fix_hint`, the byte
range it applies to, and the document it came from:

```js
const [finding] = summary.reports.flatMap((report) => report.violations);

finding.severity;         // "error"
finding.source.level;     // "official_vendor"
finding.source.reference; // "https://developers.facebook.com/docs/meta-pixel/get-started"
finding.targets[0];       // { component: "whole_url", start: 0, end: 46, ... }
```

## What it checks

- URL conformance, transport, credentials, fragments, and ad-tech macro handling
- IAB consent signals: TCF `gdpr` and `gdpr_consent`, the deprecated US Privacy
  string, and GPP `gpp` and `gpp_sid`
- Vendor parameter contracts for thirty-five endpoint families, each cited to the
  vendor's own documentation
- Endpoint attribution for 90 vendors, so an unrecognized pixel still gets a name.

## API

| Function | Returns |
| --- | --- |
| `validate(artifact, options?)` | The full validation summary |
| `isOk(summary)` | `false` when any error-severity finding is present |
| `rulepacks()` | Every rulepack with its evidence level |
| `vendors()` | The vendor endpoint directory |
| `vendorForHost(host)` | The vendor that serves a host, or `null` |
| `version()` | The `pixellint-core` version this build wraps |

`options` takes `kind` (`url` by default, plus `vast`, `postback`, `request`,
`html`, `js`, `gtm`, `unknown`), `state` (`unknown`, `template`, `fired`), and
`vendor` for a caller's claimed vendor.

Templates keep their macros: pass `{ state: "template" }` and unexpanded macros
stop being findings.

## Links

- [Playground](https://pixellint.org)
- [Rule inventory](https://github.com/aleksUIX/pixellint/blob/main/docs/STANDARDS.md)
- [Writing a rulepack](https://github.com/aleksUIX/pixellint/blob/main/docs/RULEPACK_SCHEMA.md)

Apache-2.0. Not affiliated with any vendor named in its rulepacks.
