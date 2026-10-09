# Rulepack Manifest Schema

A rulepack is a JSON document. Pixellint compiles it into a validator that runs
beside `core` and any other pack. First-party vendor packs use this exact
format, so anything the shipped packs do, a custom pack can do.

Load one with the CLI:

```bash
pixellint validate url 'https://px.acme.example/collect?aid=1' --rulepack-file acme.json
```

or from Rust:

```rust
let mut engine = pixellint_core::Engine::default();
engine.register_manifest_path("acme.json")?;
```

## Top level

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Pack id such as `vendor/meta` or `custom/acme`. Lowercase letters, digits, `-`, and `/`. The code prefix is the id with `/` replaced by `.`. |
| `display_name` | yes | Human-readable name, used in finding text and listings. |
| `description` | yes | One line describing what the pack covers. |
| `version` | no | Defaults to the `pixellint-core` version. |
| `vendor` | no | Vendor slug reported as `detected_vendor` when the pack matches. |
| `gdpr_non_applicable_values` | no | Documented destination-specific false values, accepted by core only for a matching selected endpoint. Requires an exact `gdpr` enum containing each value and forbids `any_host`. |
| `gdpr_consent_aliases` | no | Documented alternative TC String query names, recognized by core only for a matching selected host-bound endpoint. Each must also be an alias of the declared `gdpr_consent` contract. Every literal carrier is validated, and ambiguous simultaneous carriers get a duplicate-signal warning. |
| `source_level` | no | Evidence level for every rule in the pack. Defaults to `official_vendor`. |
| `docs` | no | Pack-wide documentation URL. Rules inherit it when they omit their own. |
| `param_style` | no | `query` (default), `query_semicolon`, `matrix`, or `colon_path`. |
| `path_pattern` | no | Regular expression with named captures, run against the path. Each named group becomes a parameter. |
| `match` | yes | Which artifacts the pack claims. |
| `params` | no | Parameter contracts. |
| `rules` | no | Rules that span more than one parameter. |
| `body` | no | Contracts on the JSON request body. |
| `http` | no | Complete-request contracts over URL, method, headers, query and decoded body. See [HTTP_REQUEST_SCHEMA.md](HTTP_REQUEST_SCHEMA.md). |
| `http_url_presence_overrides` | no | Canonical required or recommended URL fields whose missing presence is checked by `http` alternatives on matching complete captures. Bare URL requirements and submitted value checks still run. |
| `http_queries` | no | Query strings in captured bulk bodies, each evaluated with this pack's URL contracts. |

`source_level` is one of `normative`, `official_vendor`, `official_template`,
`ecosystem_reference`, `heuristic`. A rule at `official_vendor` must resolve to
a documentation URL, from its own `doc` or the pack's `docs`, or the manifest
is rejected at load time.

### `param_style`

- `query` reads `?a=1&b=2`.
- `query_semicolon` reads the query string and splits on both `&` and `;`,
  the shape Adform uses: `?bn=123;C=1` next to `?bn=123&v=3`.
- `matrix` reads semicolon-delimited pairs carried on the path, the shape
  Floodlight uses: `/ddm/activity/src=123;type=abc;cat=xyz;ord=1?`.
- `colon_path` reads slash-delimited `key:value` path segments, the shape
  Partnerize uses: `/conversion/campaign:ID/clickref:ABC/[category:DVD/quantity:1]`.
  Leading `[` and trailing `]` on a segment are stripped so basket keys still
  match `category` and `quantity`. Repeated keys (two basket items) are each
  checked.

### `path_pattern`

Some endpoints carry their identifier as a path segment. A pattern with named
captures turns those segments into parameters you can contract like any other:

```json
"path_pattern": "/pagead/(?:viewthrough)?conversion/(?<conversion_id>[^/?]*)"
```

`conversion_id` then takes a `requirement` and a `format` in `params`, and its
findings point at the matched span of the path. A pattern with no named capture
group is rejected at load time. When the pattern does not match, the captured
parameters are simply absent, so a `required` contract reports `.missing`.

## `match`

At least one of `hosts` or `host_suffixes` is required unless `any_host: true`
declares a self-hosted endpoint. That opt-in requires a specific non-root
`paths` or `path_prefixes` selector.

| Field | Meaning |
| --- | --- |
| `hosts` | Exact host matches, compared case-insensitively. |
| `any_host` | Accept configurable hosts only with a specific non-root path or prefix and no `path_contains` selector. Defaults to false. |
| `host_suffixes` | Domain suffix matches. `example.com` matches `example.com` and `a.example.com`, never `notexample.com`. |
| `paths` | Exact path matches. |
| `path_prefixes` | Path prefix matches. |
| `path_contains` | Path substring matches. |
| `query_params_any` | At least one exact decoded query key must be submitted, including an empty value. |
| `query_params_none` | None of the exact decoded query keys may be submitted. |
| `json_paths` | Shapes that identify a JSON body as this pack's. Required when the pack declares a `body`, and rejected without one. |
| `artifact_kinds` | Artifact kinds the pack applies to. Defaults to the URL-like kinds: `url`, `vast`, `postback`, `request`, `unknown`. |

Host and path are evaluated after macros are neutralized, so a templated URL
still resolves to its endpoint. If any path field is present, at least one path
condition must hit. Query key selectors then apply independently. They do not
read fragments or query values. Contradictory or empty query key declarations
are rejected. Explicit pack selection still validates an unrecognized branch.

## `params`

```json
{
  "name": "id",
  "aliases": ["pixel_id"],
  "requirement": "required",
  "format": { "kind": "integer", "min_digits": 15, "max_digits": 16 },
  "severity": "error",
  "format_severity": "warning",
  "description": "It is the numeric Pixel ID from Events Manager.",
  "fix_hint": "Set `id` to the numeric Pixel ID.",
  "doc": "https://example.com/docs/pixel-parameters",
  "source_level": "official_vendor"
}
```

`requirement` drives which findings a parameter can produce:

| Requirement | Absent | Present |
| --- | --- | --- |
| `required` | `.missing` at error | value checks run |
| `recommended` | `.missing` at warning | value checks run |
| `optional` (default) | nothing | value checks run |
| `forbidden` | nothing | `.forbidden` at error |
| `deprecated` | nothing | `.deprecated` at warning, then value checks |

Value checks are `.empty` when the value is blank and `.invalid` when it fails
`format`. Generated codes are `<prefix>.param.<name>.<issue>`, for example
`vendor.meta.param.id.missing`.

`severity` overrides the severity of every finding for that parameter.
`format_severity` overrides only `.invalid`, which is how a parameter can be
mandatory while unrecognized values stay a warning.
`condition` uses the same scope-local predicates as cross-field rules. It guards
all presence and value checks, including dynamic member families. Every referenced
field must be declared in the same URL or body spec. Unknown macro values do not
activate guarded checks.
`name_pattern` applies the same value checks to every present parameter whose
name matches the regex. Findings use the matched name, so `u21=` reports
`param.u21.empty` rather than a family label. It cannot be `required` or
`recommended`, and cannot declare aliases. Body families also require
`name_pattern_parent`, described below.
`allow_empty` skips the `.empty` finding, so a blank value is an unfilled
template slot. Format checks still run when the value is populated.

`min_length` and `max_length` limit decoded strings in Unicode characters.
`min_byte_length` and `max_byte_length` instead count decoded UTF-8 bytes.
`max_utf16_length` counts decoded UTF-16 code units, matching JavaScript
`String.length`; supplementary Unicode characters count as two units.
`normalization` can be `trim` or `trim_lowercase` when the vendor documents
normalization before validation. It applies to string emptiness, length and
scalar formats, while native JSON type checks and evidence retain the original
field. Guards and cross-field rules read the original scope values.
`minimum` and `maximum` are inclusive numeric bounds, written as JSON numbers.
`numeric_strings: true` also applies them to strings in a numeric/string JSON
union. Without that option, a mixed union's numeric bounds apply to numbers.
`max_joined_length` and `join_separator` limit a decoded string array after
joining its elements. They must be declared together. Counts use Unicode scalar
values and include separators. Nonstring arrays and unknown macro values are
not coerced into strings.
`max_compact_bytes` measures compact UTF-8 JSON serialization of the selected
value. Whitespace and original escape spelling do not contribute to this metric.
It differs from the request's raw bytes and from a stored object's final size.
`max_key_length` and `key_pattern` constrain the decoded object member name at
the selected field location. A body root contract can use them within a member
scope. Array indices and the document root have no object member name.
URL values with numeric bounds must parse as JSON numbers. Comparisons preserve
the decimal value, including integers above JavaScript's exact integer range.
`multiple_of` requires an exact multiple of a positive JSON number. Moduli
support significands through `u128::MAX`; larger significands are rejected
at load rather than producing incorrect findings. Numeric bounds retain arbitrary
decimal precision, with checked signed 64-bit exponents.
Body numeric limits also work on numeric strings when the declared type is string. In a
union containing number or integer, numeric limits apply to numeric values.

Body contracts can also declare `json_type`, either a single type or a list:
`string`, `number`, `integer`, `boolean`, `object`, `array`, or `null`.
This checks the actual JSON value before its textual format. A string `"123"`
does not satisfy `json_type: "integer"`. Integral numeric forms such as `1.0`
and `1e3` do satisfy it. Arrays and objects accept empty values unless a minimum
size or `non_empty` format is declared. Explicitly permitted `null` skips scalar
formats.

`min_items` and `max_items` limit arrays. `min_properties` and `max_properties`
limit objects. Bounds apply only to the corresponding type in a union. An
array bound cannot be paired with a type union that excludes arrays. Empty
type unions and reversed bounds are rejected when loading the manifest.
`property_exclusions` lists reserved keys to omit from object property counts.
`max_depth` limits nested objects and arrays, counting the selected container
as level one. `max_member_values` limits an object's total immediate values:
an array contributes its element count and any other value contributes one.

A body `name_pattern` requires `name_pattern_parent`, an object path relative
to its scope. An empty parent means the current scope. The pattern checks
immediate member names; the value contract checks each matching member.
Exact contracts take precedence over families, as they do on URLs:

```json
{
  "name": "property_values",
  "name_pattern_parent": "properties",
  "name_pattern": ".*",
  "json_type": ["string", "number"],
  "max_length": 1023
}
```

Values carrying an unexpanded macro skip format checks: unresolved macros are
the `core` pack's finding to report, and reporting both would double-count one
defect.

### Formats

| `kind` | Fields | Passes when |
| --- | --- | --- |
| `non_empty` | | the value is not empty |
| `integer` | `min_digits`, `max_digits` | the value is all ASCII digits within the bounds |
| `enum` | `values`, `case_insensitive` | the value is in `values` |
| `regex` | `pattern` | the pattern matches; compiled at load time |
| `url` | `require_https` | the percent-decoded value parses as an absolute URL |
| `hex` | `length` | the value is exactly `length` lowercase hex characters, for hashed identifiers |
| `ip` | optional `version`: `v4` or `v6`, `exclude_ranges` | the value parses as an IP address literal of the specified family outside the declared CIDR ranges |
| `datetime` | `require_timezone`, `allow_date_only`, `allow_basic`, `allow_space_separator`, `allow_javascript_date`, `allow_unpadded_date` | calendar-valid date and time; alternate forms and ECMA-262 Date string output are opt-in |
| `date` | none | a calendar-valid `YYYY-MM-DD` date |
| `json` | none | valid JSON syntax in a string, accepting any JSON value |
| `braze_time` | none | calendar and year checks for documented explicit `$time` forms; opaque timezone behavior remains advisory |
| `adobe_products` | none | Adobe product rows, required names, UTF-8 byte limits, numeric cells, and merchandising event/eVar grammar |
| `adobe_events` | none | Adobe built-in and numbered event names, numeric assignments, and nonempty serialization suffixes |
| `currency` | `allow_historical`, `case_insensitive` | membership in the dated SIX ISO 4217 registry; current uppercase codes by default |
| `tcf` | none | TCF v2 fixed fields, vendor vectors/ranges, publisher restrictions and optional segments; policy conflicts produce warnings |
| `additional_consent` | none | Google Additional Consent v1/v2 provider-list grammar; overlap and dated ATP registry conflicts produce warnings |
| `gpp` | none | GPP header IDs/counts, TCF, Canada, US Privacy and all 21 published US layouts; unknown layouts and source conflicts produce warnings |
| `us_privacy` | none | version 1 and three published Y, N, or - indicators |

Currency assignment uses the SIX current list dated 2026-09-17 and historical
list dated 2026-01-01. Vendor account eligibility is a separate requirement.
Case folding and historical-code acceptance require explicit format options.
IP exclusion lists are vendor declarations. The engine does not apply a shared
public-address policy to every destination.
Consent formats reuse core's structural checks for documented body fields or
URL aliases. Structural decoding does not establish processing permission or
jurisdiction. `tcf_consent` adds a destination's explicitly documented positive
vendor/purpose requirements. TCF policy and GPP field dependencies produce
advisories separately from encoding errors. Standard URL consent names are
already checked by core. GPP headers may encode more than two sections; the CMP
API's applicable-section list has its own two-section cap. Applicable IDs must
be present in the encoded header, while `-1` alone means no applicable section.

`client_fragment_params` lists browser configuration keys read from a loader
fragment. It requires `path_pattern`. Core suppresses the server-fragment warning
only when a selected pack matches the host and path and every fragment key is
declared. Unknown keys and unrelated endpoints keep the warning. This declaration
does not validate fragment values or establish measurement protocol coverage.

## `rules`

Each rule needs a `code` starting with the pack prefix, a `severity`, and a
`message`. `fix_hint`, `doc`, and `source_level` are optional. Every parameter a
rule names must also appear in `params`.

```json
{
  "code": "vendor.google-analytics.stream.identifier_missing",
  "kind": "require_one_of",
  "params": ["measurement_id", "firebase_app_id"],
  "severity": "error",
  "message": "The request identifies no GA4 stream.",
  "doc": "https://example.com/docs"
}
```

| `kind` | Fields | Fires when |
| --- | --- | --- |
| `require_one_of` | `params` | none of the named parameters is present |
| `mutually_exclusive` | `params` | more than one is present |
| `required_with` | `when`, `requires` | `when` is present and something in `requires` is not |
| `required_when_value` | `when`, `equals`, `requires` | `when` carries one of `equals` and something in `requires` is not present. Macro values never trigger it |
| `value_when` | `when`, `equals`, `param`, `value` | `when` carries one of `equals` and `param` is present but is not `value`. Missing `param` is left to a presence contract. Macro values never trigger it |
| `forbidden_when_value` | `when`, `equals`, `params` | `when` carries one of `equals` and something in `params` is present. Macro values never trigger it |
| `forbid_value_pattern` | `pattern`, `params` | a value matches `pattern`. Empty `params` checks every parameter, macro values excluded |
| `require_any_of` | `groups`, optional `allow_empty` | none of the alternative groups has all its fields populated; `allow_empty: true` requires keys alone |
| `format_when` | `when`, `equals`, `param`, `format`, optional `pair_occurrences` | the discriminator matches and a present scalar fails its conditional format |
| `format` | `param`, `format` | a present scalar fails its format independently of a discriminator |
| `value_with` | `when`, `param`, `value` | `when` is present and the other present field differs from `value` |
| `min_length_from` | `params`, `length_param`, `default_length` | a populated string is shorter than the request's length option, or the default when the option is absent |
| `last_param` | `param` | the named URL query parameter is followed by another parameter |
| `require_https` | none | the literal URL scheme is not HTTPS; macro schemes remain unknown |
| `max_url_length` | `max_length` | the complete trimmed URL exceeds the Unicode character limit, including path, query and fragment; unresolved macros remain unknown |
| `max_query_length` | `max_length` | the raw query text after `?` exceeds the limit, excluding any fragment |
| `max_body_bytes` | `max_bytes` | the complete JSON request exceeds the UTF-8 byte limit, including whitespace |
| `less_equal` | `left`, `right`, optional `strict`, `unit` | the left exceeds the right; `strict` also rejects equality; `unit: datetime` compares normalized calendar timestamps and their full fractions |
| `equal_values` | `left`, `right` | populated literal scalar values differ; missing, container and macro values are left to their field contracts |
| `url_path_length` | `param`, `max_length` | submitted URL path exceeds the Unicode character bound, excluding origin, query and fragment; relative and custom scheme URLs are supported |
| `delimited_sum` | `param`, `total`, `separator`, `value_separator` | a delimited amount list does not sum exactly to its total; decimal arithmetic has no binary-float tolerance |
| `awin_basket` | `parts`, `check` | `index_sequence` checks sequential `bd[n]` entries from zero; `group_membership` checks each product's commission group against `parts` |
| `gpp_sections` | `param`, `sections` | a supplied applicable ID is absent from the encoded GPP header; native integer arrays and comma-separated strings are supported |
| `tcf_consent` | `param`, `vendor_id`, `purpose_ids` | a structurally valid TCF string lacks the declared destination vendor or purpose consent |
| `gpp_field_values` | `param`, `section_param`, `field`, `values` | an applicable decoded US state field contains a choice outside the destination's permitted values; absent state fields and opaque layouts are skipped |
| `equal_array_lengths` | `params` | present native JSON arrays contain different numbers of records; missing fields and wrong native types are left to their field contracts |
| `unique_array_by` | `param`, `field` | objects in one native JSON array repeat a literal scalar member value; `field` is an exact member name, including punctuation; missing members, containers and macros are skipped; strings, numbers and Booleans remain distinct; numeric spellings normalize exactly within signed 64-bit exponent arithmetic, and identical numeric spellings outside that range still count as duplicates |
| `equal_split_lengths` | `params`, `separator` | populated delimited lists contain different numbers of records; missing optional lists and macros are skipped |
| `equal_occurrences` | `params` | repeated field families have different occurrence counts, including empty and macro positions |
| `time_window` | `param`, `unit`, optional `fallback_param`, `max_age_seconds`, `max_future_seconds` | the effective timestamp falls outside inclusive age or future bounds relative to the shared validation clock |

Timestamp units are `seconds`, `milliseconds`, `microseconds`,
`seconds_or_milliseconds`, `datetime`, and `datetime_utc`. The last form treats
an omitted timestamp zone as UTC only when the vendor explicitly documents that
interpretation. Microsecond windows compare exact
values without rounding to milliseconds. The mixed Unix form treats magnitudes
at least `10^12` as milliseconds. `fallback_param` is used only when `param` is
absent. An explicit macro retains unknown status and does not use the fallback.
At least one window bound is required. Normal validation captures the current
clock once. `Engine::validate_at`, `Engine::validate_many_at`, and CLI
`--at <unix-seconds>` allow deterministic replay. Historical golden cases declare
`reference_time` in Unix seconds. The JavaScript wrapper supplies `Date.now()`;
Rust callers on a WebAssembly target without a clock must use the explicit API.

A rule can declare `condition` to restrict when its assertion runs. Supported
predicates are `exists` or `present` with `param`, `value_in` with `param` and `values`,
`value_pattern` with `param` and `pattern`, `json_type` with `param` and a type or
type list, `all` or `any` with nested `conditions`, and `not` with one nested
`condition`. Presence includes empty containers. Scalar empty strings do not
satisfy a `present` guard. A native type guard reads the original JSON type.
`exists` checks submitted key presence, including null, empty strings, and empty
containers. It retains key presence when explicit `null_as_missing` treats the
value as unset for other rules.
Every referenced field must be contracted in the same scope. Unknown macro
values never satisfy a value predicate, including under negation.
`value_in` compares native JSON numbers by exact numeric value when their
decimal exponents can be normalized. Thus `-1`, `-1.0`, and `-1e0` select the
same numeric condition. Strings and URL values keep exact textual matching.
For exponents outside normalization range, identical numeric spellings match.

`format_when` with `pair_occurrences: true` pairs the nth discriminator with the
nth value. Empty and macro positions retain their indices. It does not require
adjacent query keys. Combine it with `equal_occurrences` when every discriminator
must have one value.

```json
{
  "code": "custom/shop.purchase_currency",
  "kind": "required_with",
  "when": "value",
  "requires": ["currency"],
  "condition": {"kind": "value_in", "param": "event", "values": ["purchase"]},
  "severity": "error",
  "message": "A purchase value requires its currency."
}
```

## `query_scopes`

Semicolon-separated query transports can declare an array of independent
namespaces. Each entry contains `params`, `rules`, optional `condition`,
`first_index` (default 0), and optional inclusive `last_index`. Indices count raw
semicolon groups, including empty groups. Percent-encoded semicolons are data.
This feature requires `param_style: "query_semicolon"`.

`source: "path_items"` instead evaluates balanced bracketed baskets in
`param_style: "colon_path"` URLs. Item parameters are excluded from the global
URL namespace. Each basket must satisfy its own requirements; an empty basket
is checked, and a whole basket macro has unknown structure. Nested field macros
and percent-encoded brackets preserve item boundaries. Unclosed or misplaced
closing brackets produce a structural finding. `global_params` can name exact
declared parameters read from outside all baskets, for relationships such as
each monetary item requiring a global currency. Basket values cannot override
those global parameters. The default `source` is `query_semicolon`.

A guard reads values in that group alone. Global or targeting fields cannot
satisfy required fields in later groups. `unique_by` lists exact contracted
fields whose literal values must differ across selected groups. Uniqueness uses
the field contract's citation and severity. Empty values and macros are skipped.
Findings retain original byte spans and use fields such as `query[2].slau`.
Missing fields point at their own group.

## `body`

Endpoints that carry their events in a JSON request body contract them here.
An exact body parameter can declare `root_path` to read a document-root field
into its scope-local `name`. This supports request defaults and policies in each
batch item while retaining the original field's byte target. `root_path` cannot
be combined with aliases or dynamic name families.
`ancestor_path: {"levels": 2, "path": "type"}` reads relative to an ancestor
of the current scope. Each object member or array index counts as one level.
This preserves a frame's own parent stacktrace type in mixed batches. It cannot
be combined with `root_path`, aliases, or dynamic name families.
Aliases satisfy the same logical field for conditions and cross-field rules.
Alternate field names within wildcard paths satisfy only the corresponding
logical array slot. Missing alternate spellings report once per absent slot.
Targets retain the spelling and location submitted in the artifact.
`null_as_missing: true` implements a destination's explicit null-as-unset field
semantics. It is body-only and must be enabled per field. Array element type
contracts still reject unsupported null members.
`scope_exclusions` lists absolute JSON path patterns. Selected nodes and their
descendants are excluded by their parsed subtree spans. This allows custom
attribute checks to leave reserved profile object subtrees out of that scope.
Conversion APIs batch events into an array, and every element has to satisfy the
same contract, so `scope` names that array and the contracts underneath are
written relative to one element. Each element is then evaluated on its own:
three events that all omit `event_name` produce three findings, each pointing at
its own bytes.

| Field | Required | Meaning |
| --- | --- | --- |
| `scope` | no | The batch array, such as `data[]`. A list means alternative envelopes, tried in order, first that is present wins; `""` means the document itself. Omitting it evaluates the document once. |
| `source_param` | no | A declared URL parameter whose decoded value contains JSON. |
| `source_field` | no | A JSON string field containing another JSON value, with paths such as `events[].value`. |
| `decoded_source_field` | no | A second JSON string field inside the decoded `source_field` document, decoded as JSON. |
| `decoded_source_condition` | no | A JSON shape selecting that second representation in the first decoded document. Requires `decoded_source_field`. |
| `source_max_length` | no | Unicode character limit on the original `source_field` string, including its embedded JSON whitespace. |
| `source_max_length_when` | no | A first-decoded-document JSON shape controlling `source_max_length`. Requires the limit. |
| `field_encoding` | no | Inner field encoding when both sources are declared. Defaults to `json`; `base64_json`, `base64_latin1_json` `pipe_delimited_json`, `percent_encoded_json` and `query_params_json` are opt-in. |
| `encoding` | no | `json` by default; `base64_json` decodes UTF-8 JSON and `base64_latin1_json` decodes browser `btoa` JSON. Both accept standard or URL-safe base64 and optional padding. `pipe_delimited_json` represents a pipe-separated tuple as a JSON array of strings. `percent_encoded_json` decodes one additional URI component layer. `query_params_json` reads a nested form-style query as a JSON object of strings or repeated arrays. |
| `encoding_severity` | no | `error` by default, or `warning` or `info`, for encoded-source decoding and JSON syntax failures. Requires `source_param` or `source_field`. Applies to outer, inner and `decoded_source_field` decoding; field contracts and source-length limits retain their own severities. |
| `condition` | no | An outer URL parameter condition for a `source_param` spec. |
| `params` | no | Parameter contracts, named by path relative to the scope. |
| `rules` | no | Cross-field rules, using the same names. |

`body` may also be a list of specs. That is how a pack contracts more than one
level of the same payload: one spec with no scope for the envelope, another
scoped to the batch array for the events inside it.

`base64_latin1_json` maps each decoded byte to the Unicode code point with the
same numeric value, matching browser `btoa(JSON.stringify(...))` producers.
The JSON parser then handles ordinary JSON escapes. Decoded body byte limits
count the original Latin-1 bytes. Existing `base64_json` keeps strict UTF-8
decoding, and source targets still point to the original encoded parameter or
string field.

`pipe_delimited_json` preserves component order, empty components and trailing
empty components without numeric coercion. Tuple positions use paths such as
`[0]` and `[1]`. Repeated source parameters are evaluated independently. Body
byte limits count the decoded original tuple's UTF-8 bytes, before its synthetic
JSON representation adds quoting and escapes. Macro components remain unknown,
and findings point to the original parameter or string field.

`percent_encoded_json` decodes one strict percent-encoded UTF-8 layer after
the enclosing URL or form transport has already decoded its layer. Literal
plus signs remain plus signs, matching JavaScript `encodeURIComponent`.
Malformed escapes and invalid UTF-8 produce the declared encoding severity.
Successfully decoded whole-source macros remain unknown; they do not become
fabricated JSON syntax failures.

`query_params_json` reads an inner form-style query. Plus signs become spaces,
percent escapes must decode to UTF-8, and duplicate literal names preserve
ordered arrays. Bracketed names remain literal names. This codec does not invent
PHP object semantics. Body byte limits count the original inner wire string,
before the synthetic JSON representation adds quoting or escapes. Its field
contracts decide the source-specific severity.

Declaring both `source_param` and `source_field` decodes the URL parameter first,
then reads the inner string field from that document. `encoding` applies to the
outer parameter and `field_encoding` to the inner string. URL guards apply
before either decode. Nested findings retain the original URL parameter target.
`decoded_source_field` supports JSON encoded inside an already decoded JSON
string. Its optional shape guard leaves documented scalar alternatives intact.
Every nested finding retains the original outer source span. A source length
guard reads the first decoded document and measures the original string, not
the compact serialization of its JSON value.
Encoded-only URL packs
omit `match.json_paths`; body string decoding still needs the normal body
matcher. Declare the outer field's native string type and the decoded root's
type explicitly. Invalid encoding gets a finding unless it contains a template
macro. Findings from decoded fields point to the original URL parameter or JSON
string span, since decoded offsets do not identify the original bytes.

Specs sharing a source and encoding produce one syntax finding at the strongest
applicable severity: `error`, then `warning`, then `info`. URL conditions select
active specs before decoding. A `decoded_source_condition` selects nested
decoding after its containing JSON parses, so inactive nested specs cannot raise
that stage's severity. A malformed containing source cannot evaluate that
nested guard and uses all specs whose outer conditions apply. Different
encodings remain independent representations. Existing manifests keep error
severity unless they explicitly opt into advisory syntax checks.

Body contracts get their own name space and their own code segment, `body`
rather than `param`, because an endpoint may accept the same field in the query
string and in the payload under different rules.

### Paths

A path is dotted keys with `[]` for "every element": `user_data.em[]` under a
scope of `data[]` reads `data[0].user_data.em[0]` and up. Fixed `[N]` selectors
address tuple positions. Quoted selectors address literal punctuation or empty
keys, such as `properties["a.b"]` and `properties[""]`.

`*` expands all immediate members of an object. A scope such as
`user_properties.*` checks every dynamic property object independently.
`**` expands every descendant value, including array elements and nested object
members. Use an explicit type union to apply string limits recursively without
rejecting legitimate containers and other scalar types.
An empty contract name addresses the current scope itself, including the
document root. Its generated finding name is `root`. Use root contracts to
check a bare array's size or each batch element's JSON type.

Two absences are treated differently. A field missing from an element that
exists is reported once per element. A field under a container that is itself
missing is not reported at all, since the container has already been reported
and nothing underneath it was ever addressable.

A value format describes a scalar, so it is skipped when the value is an object
or an array. Vendors accept several identifier fields as either one value or a
list of them, so contract both forms: `user_data.em` and `user_data.em[]`.
`json_type` and size constraints still check containers before this scalar
format step. Fields beneath a malformed container are skipped; contract the
container itself to report that defect.

Automatic selection needs recognizable vendor fields. An empty or malformed
batch may lack those fields. Explicit `--rulepack` selection still checks the
body contracts and reports `payload_mismatch` as context. A mismatch does not
count as a detected vendor.
JSON syntax errors are reported by core even when explicit selection excludes
core, since vendor contracts cannot inspect an unparsed document.

### `match.json_paths`

A bare body carries no host, so the shape of the payload is what identifies it.
Every entry has to hold. An entry is one of:

| Form | Holds when |
| --- | --- |
| `"data[].event_name"` | the path resolves to a field that is present |
| `{ "path": "event", "json_type": "string" }` | a present field has the specified native JSON type or type union |
| `{ "path": "content", "pattern": "^\\s*\\[" }` | at least one present value matches the regular expression |
| `{"path": "...", "excludes": "regex"}` | no value at the path matches the pattern |
| `{"any_of": [...]}` | at least one nested entry holds |

The exclusion form exists because conversion APIs have converged on the same
envelope. Meta and Snap both post `{"data": [...]}` with the same field names,
and only the values differ: `action_source` is `website` for Meta and `WEB` for
Snap. It is written as an exclusion rather than as "my values match" on purpose.
The discriminating field is usually one the pack also contracts, and a payload
with a typo in it is the one that most needs validating, so an unfamiliar value
must leave the payload claimable. When nothing discriminates, both packs claim
the payload and both report.

## What the loader rejects

- Unknown fields anywhere in the manifest
- A pack id that is not lowercase segments
- Empty `display_name` or `description`
- A `match` with no `hosts` or `host_suffixes`, unless explicitly constrained by `any_host`
- The same parameter name or alias contracted twice
- An invalid regular expression
- An `official_vendor` rule with no resolvable documentation URL
- A rule code that does not start with the pack prefix
- A rule referencing a parameter that is not contracted
- A `body` with no `match.json_paths`, which would claim every payload it is shown
- `match.json_paths` with no `body`, which would match a shape and check nothing
- A malformed JSON path

## Findings a pack produces on its own

- `<prefix>.endpoint_mismatch`, info: the pack was selected explicitly but the
  artifact does not target its endpoints
- `<prefix>.payload_mismatch`, info: the pack was selected explicitly but the
  JSON body does not have the shape its endpoint accepts
- `<prefix>.claimed_vendor_mismatch`, info: the caller passed a `claimed_vendor`
  that is not this pack's vendor, and the endpoint matched anyway
