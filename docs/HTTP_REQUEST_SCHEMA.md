# Complete HTTP captures keep destination checks together.

Pass a JSON capture as a string with artifact kind `request`. The CLI, Rust
engine, npm library, WASM and MCP use the same representation:

```json
{
  "url": "https://plausible.io/api/event",
  "method": "POST",
  "headers": {
    "Content-Type": "application/json; charset=utf-8",
    "User-Agent": "Mozilla/5.0"
  },
  "body": "{\"name\":\"pageview\",\"url\":\"https://example.org/\",\"domain\":\"example.org\"}"
}
```

`url`, `method` and `headers` are required. An empty headers object means the
capture establishes that no headers were supplied. `body` is optional; when
present it must be a raw string, including original whitespace. A parsed JSON
object loses the wire representation needed for byte limits and is rejected.
Unknown envelope fields and duplicate object keys are rejected.

Headers may instead be a list of objects with string `name` and `value` fields.
This preserves repeated header lines. Names are case insensitive, surrounding
header whitespace is trimmed, and repeated values remain arrays in the rule
namespace. MIME essence is lowercased and excludes parameters. Method tokens
retain case because HTTP methods are case sensitive.

```javascript
import { validate } from "pixellint";
const result = validate(JSON.stringify(capture), { kind: "request" });
```

Rust callers may serialize the exported `HttpRequest`, `HttpHeaders` and
`HttpHeader` types into the existing `ValidationRequest.artifact` string, then
use `ArtifactKind::NetworkRequest`. `validate_at` shares one explicit clock
between URL, transport and payload checks, including on WASM.

Matching uses the URL endpoint rather than a standalone body shape. A payload
that resembles another vendor's schema does not select that vendor. Explicitly
forcing a different vendor produces its endpoint mismatch finding. JSON syntax
remains an input check when only vendor packs are selected.

JSON MIME types, including `+json`, decode a native body. JSON-looking raw
bodies with absent or text MIME can still receive body checks, while transport
rules separately decide whether that MIME is allowed. `application/x-ndjson`
decodes nonempty JSON lines into an array. Body byte limits use the original
wire string, including NDJSON whitespace and newlines.

`application/x-www-form-urlencoded` decodes fields with repeated values
preserved. Query values, including declared aliases, take precedence over form
fallbacks. Path captures remain authoritative and cannot be supplied by query
or form fields. Encoded `body.source_param` contracts can validate JSON/Base64
inside form fields. Ordinary native JSON body scopes do not run on the enclosing
form object.

Compressed and multipart bodies produce
`core.request.unsupported_body_encoding` at informational severity. Pixellint
does not claim to have inspected those payloads. Observable endpoint, method,
header and submitted query contracts can still run. Requirements whose values
could be in an unavailable form body, and body-dependent HTTP contracts, are
deferred. A capture with no errors can therefore still have incomplete coverage.
Provide decoded entity bytes and matching headers for those body checks.

Complete-capture findings carry fields and citations. Targets are omitted
because ranges in decoded JSON cannot be used as offsets into the capture's
escaped body string. Authentication header values and decoded Basic credentials
are redacted from findings. Captures themselves can contain credentials and
should remain private; this input feature does not add production collection.

Manifest `http` contracts reuse body contract fields, scopes and guards. The
normalized root exposes `url`, `host`, `path`, `method`, `content_type`,
`headers`, `query`, `body`, `body_encoding`, `authorization_scheme`, and, for
valid decoded Basic authentication, `basic_auth.username`,
`basic_auth.password` and `basic_auth.has_colon`. The last field records legacy
colonless keys without silently inventing a separator. Query and form values
are strings; repeated values are arrays. Header values preserve their case.

HTTP contracts run only on complete captures. Generated field codes use
`<pack-prefix>.http.<field>.<issue>`. A top-level HTTP spec's `condition` reads
its declared normalized fields. Raw body limits and encoded sources belong in
`body`, since the normalized wrapper has a different size and representation.
These local contracts cannot prove credential validity, account permissions,
historical duplicate events or remote acceptance.

`http_queries` declares bulk event strings, such as Matomo's `requests` array:

```json
"http_queries": [
  {
    "source_field": "body.requests[]",
    "encoding": "url_query",
    "inherited_params": { "token_auth": "body.token_auth" },
    "inherited_overrides": true
  }
]
```

By default each string is a query with optional leading `?`. `url_query`
extracts the query from a full or relative URL, stopping before its fragment;
strings without a nonempty query are skipped. Each item runs existing URL
contracts against the captured endpoint, preserving the query's original
encoding. Inherited values are fallbacks for missing keys and declared aliases;
`inherited_overrides` replaces per-item keys when the source defines envelope
precedence. One item cannot satisfy another item's requirements. Ordinary `http` contracts
validate the array and its native string items. Bulk field presence replaces
outer event-query checks, including empty or malformed arrays, whose shape is
then checked by those contracts. Findings identify `http.body.requests[index]`.
Core URL checks run for inner queries when core is selected.
Native parameter objects remain an explicit per-item information finding
because destination-specific native coercions are not modeled by URL contracts.

The bounded JSON reader accepts entity nesting through depth 64. Deeper
documents exceed a local parser limit, even when a vendor permits them. They
produce a core parse finding; method and header checks remain available. This
limit prevents exhaustive validation of vendor contracts above that depth.
