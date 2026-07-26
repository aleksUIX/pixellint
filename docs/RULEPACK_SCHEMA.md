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
| `match` | yes | Which artifacts the pack claims. |
| `params` | no | Parameter contracts. |
| `rules` | no | Rules that span more than one parameter. |

`source_level` is one of `normative`, `official_vendor`, `official_template`,
`ecosystem_reference`, `heuristic`. A rule at `official_vendor` must resolve to
a documentation URL, from its own `doc` or the pack's `docs`, or the manifest
is rejected at load time.

### `param_style`

- `query` reads `?a=1&b=2`.
- `matrix` reads semicolon-delimited pairs carried on the path, the shape
  Floodlight uses: `/ddm/activity/src=123;type=abc;cat=xyz;ord=1?`.

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

## Findings a pack produces on its own

- `<prefix>.endpoint_mismatch`, info: the pack was selected explicitly but the
  artifact does not target its endpoints
- `<prefix>.claimed_vendor_mismatch`, info: the caller passed a `claimed_vendor`
  that is not this pack's vendor, and the endpoint matched anyway
