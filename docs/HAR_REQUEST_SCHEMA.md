# HAR request replay preserves capture availability.

`pixellint import-har @session.har` extracts local HAR 1.2 requests.
`pixellint validate-har @session.har --json` validates them offline against
Pixellint's existing endpoint, HTTP and payload contracts. The commands never
send captured requests. Responses, cookies stored separately from request
headers, initiators and browser state do not become fabricated request fields.

```sh
pixellint import-har @session.har --har-headers chrome_sanitized
pixellint validate-har @session.har --json --har-headers chrome_sanitized
pixellint validate-har @session.har --json --har-headers complete --at 1791547200
```

Input can also be an inline JSON string or `-` for stdin. Validation flags,
custom packs and directory files work as on `validate-many`. Validation exits
with 0 for no errors, 1 for error findings, and 2 for malformed input or usage.
Import emits JSON with `document`, `entries` and `header_policy`; it exits with
0 after extraction or 2 for malformed input. Import does not run vendor rules.

## Omitted fields stay distinguishable from missing request fields.

[Chrome's export documentation](https://developer.chrome.com/docs/devtools/network/reference#save-all-network-requests-to-a-har-file)
explains that its default sanitized export excludes Authorization, Cookie and
Set-Cookie headers. HAR 1.2 has no standard flag establishing that an omitted
request header was absent on the original request. Pick a capture policy:

- `unknown` is the default. Absent Authorization and Cookie remain unknown.
  Populated headers and other absent headers retain their observed contract
  checks. This records uncertainty about those two fields, not a claim that
  all sensitive fields were removed.
- `chrome_sanitized` records omitted Authorization and Cookie as redacted.
  A credential header that is present remains observed and gets value checks.
  Use this when the export procedure is known.
- `complete` records absent headers as absent on the request. Use this when
  capture completeness is established. Missing credential findings then run.

A missing or null `request.headers` collection makes absent header names
unavailable under every policy. Explicitly supplied HTTP envelopes may declare
partially observed headers as well. Method, endpoint, populated header values
and available body checks stay active. Presence rules involving an unavailable
alternative credential carrier are deferred, while invalid observed carrier
values still produce findings. An informational `core.request.capture_incomplete`
finding makes this limitation visible even when a caller selects only vendor packs.

## Raw request fields keep their representation.

The importer preserves `request.method`, the complete original `request.url`,
the ordered header name/value list and raw `postData.text`. It does not merge
or rebuild `queryString`, infer Content-Type from `postData.mimeType`, recreate
form encoding from `postData.params`, or construct Cookie from `request.cookies`.
Repeated query values, percent encodings, literal plus signs and header lines
therefore remain as captured. Posted-data metadata stays in each entry's
`post_data` field, including encoding extensions and parameter/file metadata.

Under the [HAR 1.2 request schema](https://webperfwg.org/specs/HAR/Overview.html#request),
`bodySize: 0` with no postData establishes an absent body. A positive, -1,
null or omitted size with no postData records an unavailable body.
The importer does not infer absence from GET or POST. Raw text is available
when its UTF-8 byte count agrees with a known nonnegative bodySize and its
representation is supported. Body text, including whitespace, is used for
vendor byte limits.

These representations remain unavailable for body-dependent contracts:

- postData with params and no raw text, or with both text and params;
- a declared request text encoding (`encoding` or `_encoding`), including base64;
- a known bodySize that disagrees with the UTF-8 text length;
- a non-UTF-8 Content-Type charset, except US-ASCII with ASCII-only text;
- raw request text exceeding the local 4 MiB inspection limit.

The importer preserves these fields and reports the reason. HAR 1.2 defines
base64 for response content, not request postData. An encoded response does
not change request availability. `postData._redacted: true` is a Pixellint
capture annotation, not a HAR 1.2 field. It explicitly marks the body redacted
and prevents body inspection. Existing compressed and unsupported multipart
rules continue to report their own decoding limitations. No sensitive-value
placeholder text is guessed as redaction automatically.

## Replay uses recorded clocks and entry provenance.

The result retains the original `startedDateTime`, parsed Unix seconds,
`pageref`, body-size declaration, body availability and capture context for
every entry. The occurrence path is `/log/entries/<index>/request`, with
occurrence ID `har-entry-<index>`. Entries stay in original order.

Default replay validates each request at its own explicit-zone capture time.
Subsecond fractions are floored to Unix seconds, including pre-epoch dates.
Equivalent requests at the same reference clock are deduplicated through the
existing document wrapper and retain all occurrences. The same request at
different clocks can have different timestamp findings, so those validations
remain distinct. `--at` overrides all capture clocks and can coalesce them.

A missing, malformed or timezone-free startedDateTime stays unavailable in
import metadata. Default replay returns an input error requesting an explicit
reference-time override. It never silently validates a historical request
against today's clock. The `captures` list records original clock metadata
even when an override is supplied. `reference_time_override` records the actual
override, or null when recorded clocks were used. Extracted requests use the
fired expansion state; explicit CLI `--state` can override it.

## Libraries use the same adapter and engine.

```javascript
import { importHar, validateHar } from "pixellint";
const imported = importHar(harText, { headerPolicy: "chrome_sanitized" });
const result = validateHar(harText, { headerPolicy: "chrome_sanitized" });
const repeatedAtOneClock = validateHar(harText, { at: 1791547200 });
```

The npm functions accept a raw HAR JSON string or an object. Prefer the raw
string when duplicate JSON keys must be rejected before an object parser can
overwrite them. `at` is a safe integer number of Unix seconds. WASM exposes
`import_har` and `validate_har` with the same import and replay semantics.

The MCP `validate_har` tool accepts inline `har`, optional `header_policy`,
optional integer `at`, and existing `rulepacks`/`except_rulepacks` selections.
It returns the same document reports and per-entry availability metadata.
Missing capture clocks need an explicit `at` override.

Rust exposes `import_har`, `HarImportOptions`, `HarHeaderPolicy`,
`Engine::validate_har` and `Engine::validate_har_at`. Extraction produces a
`DocumentRequest` reusable by existing document callers. To retain recorded
per-entry clocks, use the HAR replay helper. Ordinary `validate_many` uses its
own reference clock. No automatic HAR detection is added to single-artifact
validation, and existing plain `HttpRequest` captures retain their semantics.

The additive HTTP envelope's optional `capture` object supports:

```json
{
  "url": "https://example.org/events",
  "method": "POST",
  "headers": [{ "name": "Content-Type", "value": "application/json" }],
  "capture": {
    "unavailable_headers": ["authorization", "cookie"],
    "body": "unavailable"
  }
}
```

`headers_unavailable` marks incomplete header coverage. `unavailable_headers`
marks names whose absent values are unknown. `redacted_headers` explicitly
marks names whose values cannot be inspected. `body` accepts `available`,
`absent`, `unavailable` or `redacted`. Available requires raw body text; absent
requires no raw body field. Unavailable/redacted text may remain in the raw
capture for provenance but is not decoded. Header names are case insensitive;
duplicate or contradictory availability declarations are rejected. Rust's
`CapturedHttpRequest` serializes this envelope without changing `HttpRequest`.

## Resource and evidence limits stay explicit.

Import is bounded to 64 MiB of input, 50,000 entries and 4 MiB of inspectable
text per request. These are local resource limits, not HAR or vendor limits.
The adapter validates fields needed for request extraction, not the full HAR
response, timings or page schema. The engine's existing JSON depth and entity
decoding bounds remain in force.

Reports retain original URLs, headers and bodies, so import and replay output
can contain credentials and personal data. They stay local unless the caller
exports them. `chrome_sanitized` records measured capture availability; it does
not sanitize URLs, posted bodies or arbitrary application fields. Finding-level
credential redaction follows the existing HTTP validator, and captured raw
artifacts are preserved for debugging. Local validation cannot prove that a
request was sent, a token is accepted, or a server accepted the payload.
