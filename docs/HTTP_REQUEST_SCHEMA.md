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
present it must be a raw string, including original whitespace. Alternatively,
`body_base64` holds the original binary wire bytes in canonical, padded standard
Base64. The two body fields are mutually exclusive, including a raw `body: null`
alongside `body_base64`. A null or non-string `body_base64` is an envelope error.
A parsed JSON
object loses the wire representation needed for byte limits and is rejected.
An optional `capture` availability declaration is documented in
[HAR request replay](HAR_REQUEST_SCHEMA.md). Other unknown envelope fields and duplicate object keys are rejected.

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
use `ArtifactKind::NetworkRequest`. The exported `BinaryHttpRequest` serializes
the binary envelope and its optional capture context. Existing `HttpRequest`
struct fields and complete-capture semantics are unchanged. `validate_at` shares one explicit clock
between URL, transport and payload checks, including on WASM.

Matching uses the URL endpoint rather than a standalone body shape. A payload
that resembles another vendor's schema does not select that vendor. Explicitly
forcing a different vendor produces its endpoint mismatch finding. JSON syntax
remains an input check when only vendor packs are selected.

JSON MIME types, including `+json`, decode a native body. JSON-looking raw
bodies with absent or text MIME can still receive body checks, while transport
rules separately decide whether that MIME is allowed. `application/x-ndjson`
decodes nonempty JSON lines into an array. Body byte limits use the original
entity text, including NDJSON whitespace and newlines. For raw `body`, this is
the submitted wire string. For gzip it is the complete uncompressed entity,
before NDJSON normalization, rather than compressed bytes or Base64 characters.

## Explicit binary captures preserve their original bytes.

```json
{
  "url": "https://api.mixpanel.com/import?strict=1",
  "method": "POST",
  "headers": {
    "Content-Type": "application/json",
    "Content-Encoding": "gzip",
    "Authorization": "Basic VEVTVF9PTkxZX1BST0pFQ1RfVE9LRU46"
  },
  "body_base64": "<canonical Base64 of the original gzip wire bytes>"
}
```

The example shows the envelope shape; replace the explanatory body string
with captured bytes. `body_base64` accepts the standard alphabet, required
padding and canonical trailing bits, with no whitespace. An empty string is
a present empty body. Explicit `capture.body: available` requires either body
representation; `absent` requires neither. Declared unavailable or redacted
bodies retain provenance but are never decoded.

A single observed `Content-Encoding: gzip`, ignoring case and surrounding
header whitespace, enables local gzip decoding. Observed absent encoding and
`identity` mean uncompressed bytes. The engine does not sniff gzip magic.
Repeated encoding headers, coding lists, `x-gzip` and other codecs remain
explicitly unsupported. Unknown or redacted MIME and compression headers
defer payload inspection. Globally incomplete headers still leave supplied,
unredacted decoding headers observable.

Gzip decoding verifies every member's framing, checksum and size trailer.
Concatenated members contribute to one entity, including boundaries inside a
Unicode scalar or JSON token. The complete entity is checked before payload
rules run. Truncation, a corrupt later member and trailing junk cannot expose
partially valid events. A successful body then uses the existing JSON, NDJSON,
form or multipart decoder, while the original headers remain available to
vendor transport rules. Binary decoding does not certify that a destination
accepts gzip.

`core.request.invalid_base64_body` and `core.request.invalid_gzip_body` are
input errors. Invalid UTF-8 in known JSON or NDJSON produces
`core.request.invalid_body_utf8` with the JSON specification citation. Opaque
non-UTF-8 binary content remains informationally unvalidated. Diagnostics never
include captured bytes, gzip filenames, comments or decompressed excerpts.

Local inspection is bounded to 16 MiB of captured bytes, 16 MiB of decoded
entity bytes, 1024 gzip members and 64 KiB of aggregate header bytes per member.
An encoded-length preflight runs before allocating the decoded byte buffer.
Exceeding a bound produces `core.request.body_decode_limit` at informational
severity and suppresses dependent payload assertions. These are Pixellint
resource limits, not vendor quotas. There is no compression-ratio limit.

[Mixpanel Import](https://docs.mixpanel.com/reference/import-events) documents
gzip and at most 2000 events. The pack checks that event count for decoded
JSON and NDJSON. Its current page gives inconsistent total-byte examples and
does not establish an exact per-event MB unit, so those byte quotas remain
unvalidated. Service-account ownership, token acceptance, import history and
receiver acceptance also require external evidence.

HAR request `encoding` and `_encoding` extensions retain their existing
unavailable-body behavior. This explicit binary envelope does not reinterpret
them as standard HAR request data.

`application/x-www-form-urlencoded` decodes fields with repeated values
preserved. Query values, including declared aliases, take precedence over form
fallbacks. Path captures remain authoritative and cannot be supplied by query
or form fields. Encoded `body.source_param` contracts can validate JSON/Base64
inside form fields. Ordinary native JSON body scopes do not run on the enclosing
form object.

`multipart/form-data` decodes UTF-8 text fields using its declared boundary.
Names, repeated values, Unicode, literal plus signs and percent characters are
preserved. JSON inside a field uses that destination's existing encoded body
contracts. The local decoder is bounded to 4 MiB, 1024 parts and 16 KiB of
headers per part. File parts, alternate charsets, transfer encodings, folded or
ambiguous headers and captures above these limits remain unavailable.
Malformed required framing or disposition produces `core.request.invalid_multipart`
with the multipart RFC citation. Valid but unsupported representations and
raw text bodies claiming compression produce `core.request.unsupported_body_encoding` at
informational severity. Pixellint does not claim to have inspected those
payloads. Observable endpoint, method,
header and submitted query contracts can still run. Requirements whose values
could be in an unavailable form body, and body-dependent HTTP contracts, are
deferred. A capture with no errors can therefore still have incomplete coverage.
Provide decoded entity bytes and matching headers for those body checks.

Explicitly unavailable MIME or compression headers defer entity decoding.
For a destination with bulk query bodies, an unavailable entity can contain
events that are absent from the URL. Those missing URL fields remain unknown;
supplied URL values and observable transport violations still receive checks.

Complete-capture findings carry fields and citations. Targets are omitted
because ranges in decoded JSON cannot be used as offsets into the capture's
escaped body string. Authentication header values, decoded Basic credentials and credential-like
query or decoded form values are redacted from findings. Captures themselves can contain credentials and
should remain private; this input feature does not add production collection.

Manifest `http` contracts reuse body contract fields, scopes and guards. The
normalized root exposes `url`, `host`, `path`, `method`, `content_type`,
`headers`, `query`, `body`, `body_encoding`, `authorization_scheme`, and, for
valid decoded Basic authentication, `basic_auth.username`,
`basic_auth.password` and `basic_auth.has_colon`. The last field records legacy
colonless keys without silently inventing a separator. Query and form values
are strings; repeated values are arrays. Header values preserve their case.
Successful explicit binary captures also expose numeric `wire_body_bytes` and
`entity_body_bytes`. The former counts captured compressed or identity bytes;
the latter counts the expanded entity. Original `Content-Length` is unchanged.
Failed, unavailable or limited binary entities do not expose these decoded
metadata fields. Raw captures retain their existing normalized shape.

HTTP contracts run on request captures. Explicit availability metadata defers
only checks that depend on unavailable fields, while observed checks stay active. Generated field codes use
`<pack-prefix>.http.<field>.<issue>`. A top-level HTTP spec's `condition` reads
its declared normalized fields. Entity body limits and encoded sources belong in
`body`, since the normalized wrapper has a different size and representation.
These local contracts cannot prove credential validity, account permissions,
historical duplicate events or remote acceptance.

`http_url_presence_overrides` transfers missing URL presence checks to explicit
HTTP alternatives when the complete capture matches the pack's host, path and
query selectors. Use it for a documented credential that can be carried in the
query, form body or header:

```json
"http_url_presence_overrides": ["access_token"],
"params": [
  {"name": "access_token", "requirement": "required"}
],
"http": {
  "params": [
    {"name": "query.access_token", "json_type": "string", "allow_empty": true},
    {"name": "body.access_token", "json_type": "string"},
    {"name": "headers.authorization", "json_type": "string",
     "format": {"kind": "regex", "pattern": "(?i)^Bearer[ \\t]+[^ \\t]+$"}}
  ],
  "rules": [
    {"code": "vendor.example.http.auth", "kind": "require_any_of",
     "groups": [["query.access_token"], ["body.access_token"], ["headers.authorization"]],
     "severity": "error", "message": "Provide a populated authentication carrier."}
  ]
}
```

Names must be unique canonical names of exact required or recommended URL
contracts. Aliases, unknown names, optional, forbidden and deprecated contracts
are rejected. At least one HTTP contract scope and a host or host suffix matcher
are required. A self-hosted `any_host` matcher cannot enable this deferral.

The list alone does not validate alternatives. Declare their formats and presence
rules in `http`, using the destination's documented carriers. A populated URL or
form value retains its original format and blank checks, even when a valid header
is also supplied. Forbidden and deprecated fields keep their checks. Bare URLs,
legacy URL strings with kind `request`, and unmatched capture branches retain
ordinary URL behavior. Explicit selection of an unrelated endpoint still reports
its endpoint mismatch and does not validate that endpoint's body.

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
Native maps are opt-in with `native_map: "matomo_php8"`. The pinned Matomo
PHP 8 reader profile applies documented per-field integer, float, string and
embedded JSON readers, including its empty-value credential inheritance.
Ignored scalar or empty bulk entries follow the source behavior. Unsupported
plugin fields and platform-sensitive numeric conversions produce per-item
information findings instead of invented missing values. Other packs retain
an explicit native-map information finding.

String bulk entries in that profile use a bounded PHP query model: one URL
decode, PHP root-name normalization, bracket trees, ordered append indexes,
and scalar/array shadowing. Supported readers apply the pinned Matomo HTML4
sanitizer before their field conversion. Entity decoding runs once, literal
null bytes are removed, and quotes and HTML delimiters are escaped. Supported
JSON reader values are sanitized recursively. Template macros retain their
unresolved expansion state.

The profile models the default `&` separator, at most 1,000 nonempty input
variables and 64 bracket levels. Alternate separator candidates, malformed
brackets, raw control characters, invalid UTF-8, nonportable indexes, negative-only append behavior,
sanitized JSON-key collisions and unmapped plugin readers remain observable
coverage gaps. These bounds do not certify arbitrary PHP configurations or
historical Matomo deployments. The independent oracle records PHP 8.3.35 and
the exact upstream source checksum beside its parsing and reader outputs.

`check_chronological_order: true` requires that Matomo profile. It compares
only explicit parseable `cdt` timestamps across supported bulk entries and
warns on a backward step under the documented oldest-first recommendation.
Missing or unresolved timestamps never acquire an inferred clock.

The bounded JSON reader accepts entity nesting through depth 64. Deeper
documents exceed a local parser limit, even when a vendor permits them. They
produce a core parse finding; method and header checks remain available. This
limit prevents exhaustive validation of vendor contracts above that depth.
