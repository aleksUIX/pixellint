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
| `source_level` | no | Evidence level for every rule in the pack. Defaults to `official_vendor`. |
| `docs` | no | Pack-wide documentation URL. Rules inherit it when they omit their own. |
| `param_style` | no | `query` (default) or `matrix`. |
| `path_pattern` | no | Regular expression with named captures, run against the path. Each named group becomes a parameter. |
| `match` | yes | Which artifacts the pack claims. |
| `params` | no | Parameter contracts. |
| `rules` | no | Rules that span more than one parameter. |
| `body` | no | Contracts on the JSON request body. |

`source_level` is one of `normative`, `official_vendor`, `official_template`,
`ecosystem_reference`, `heuristic`. A rule at `official_vendor` must resolve to
a documentation URL, from its own `doc` or the pack's `docs`, or the manifest
is rejected at load time.

### `param_style`

- `query` reads `?a=1&b=2`.
- `matrix` reads semicolon-delimited pairs carried on the path, the shape
  Floodlight uses: `/ddm/activity/src=123;type=abc;cat=xyz;ord=1?`.

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

At least one of `hosts` or `host_suffixes` is required, so a pack cannot
silently claim every artifact.

| Field | Meaning |
| --- | --- |
| `hosts` | Exact host matches, compared case-insensitively. |
| `host_suffixes` | Domain suffix matches. `example.com` matches `example.com` and `a.example.com`, never `notexample.com`. |
| `paths` | Exact path matches. |
| `path_prefixes` | Path prefix matches. |
| `path_contains` | Path substring matches. |
| `json_paths` | Shapes that identify a JSON body as this pack's. Required when the pack declares a `body`, and rejected without one. |
| `artifact_kinds` | Artifact kinds the pack applies to. Defaults to the URL-like kinds: `url`, `vast`, `postback`, `request`, `unknown`. |

Host and path are evaluated after macros are neutralized, so a templated URL
still resolves to its endpoint. If any path field is present, at least one path
condition must hit.

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
| `forbid_value_pattern` | `pattern`, `params` | a value matches `pattern`. Empty `params` checks every parameter, macro values excluded |

## `body`

Endpoints that carry their events in a JSON request body contract them here.
Conversion APIs batch events into an array, and every element has to satisfy the
same contract, so `scope` names that array and the contracts underneath are
written relative to one element. Each element is then evaluated on its own:
three events that all omit `event_name` produce three findings, each pointing at
its own bytes.

| Field | Required | Meaning |
| --- | --- | --- |
| `scope` | no | The batch array, such as `data[]`. A list means alternative envelopes, tried in order, first that resolves wins; `""` means the document itself. Omitting it evaluates the document once. |
| `params` | no | Parameter contracts, named by path relative to the scope. |
| `rules` | no | Cross-field rules, using the same names. |

Body contracts get their own name space and their own code segment, `body`
rather than `param`, because an endpoint may accept the same field in the query
string and in the payload under different rules.

### Paths

A path is dotted keys with `[]` for "every element": `user_data.em[]` under a
scope of `data[]` reads `data[0].user_data.em[0]` and up. Keys containing `.` or
`[` cannot be addressed.

Two absences are treated differently. A field missing from an element that
exists is reported once per element. A field under a container that is itself
missing is not reported at all, since the container has already been reported
and nothing underneath it was ever addressable.

A value format describes a scalar, so it is skipped when the value is an object
or an array. Vendors accept several identifier fields as either one value or a
list of them, so contract both forms: `user_data.em` and `user_data.em[]`.

### `match.json_paths`

A bare body carries no host, so the shape of the payload is what identifies it.
Every entry has to hold. An entry is one of:

| Form | Holds when |
| --- | --- |
| `"data[].event_name"` | the path resolves to a field that is present |
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
- A `match` with no `hosts` and no `host_suffixes`
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
