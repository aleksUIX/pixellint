# Vendor Directory

The directory answers a question rulepacks cannot: whose pixel is this?

It maps hosts to vendors. That is the entire claim. Directory entries carry no
parameter contracts, no required fields, and no rule text, so a directory hit
never says an artifact is right or wrong. It says which vendor owns the
endpoint and whether Pixellint has a rulepack for it.

```bash
$ pixellint validate url 'https://trc.taboola.com/actions?a=1'
rulepack: core
  ok
rulepack: directory (vendor: taboola)
  info  directory.no_rulepack_coverage  This endpoint belongs to Taboola (native). No Pixellint rulepack covers it, so only the core checks ran.
    fix: Write a custom rulepack for this endpoint, or ask for first-party coverage.
```

## Why it is separate from rulepacks

A rule needs a contract someone published, otherwise Pixellint would be
asserting requirements it invented. Most vendors never publish one. Attribution
is a weaker claim that can be made honestly across the long tail, so it lives
in its own layer with its own evidence level.

The practical result: deep validation where a vendor documents its parameters,
identification everywhere else, and no pretending the second is the first.

## Behavior

- The directory runs after rulepacks and only when **no rulepack detected a
  vendor**. A matching vendor pack is strictly better information.
- A vendor with a rulepack still gets attributed on endpoints that pack does not
  cover. The finding names the pack so you know coverage exists elsewhere.
- Findings are always `info` severity. Attribution never changes an exit code.
- Toggle it like a rulepack: `--rulepack directory` runs only attribution,
  `--except directory` turns it off.

## Inspecting it

```bash
pixellint list-vendors                 # vendor, name, category, rulepack, hosts
pixellint list-vendors --json
```

Over MCP, the `list_vendors` tool takes an optional `category` filter or a
`host` to attribute directly.

## Entry shape

```json
{
  "vendor": "taboola",
  "display_name": "Taboola",
  "category": "native",
  "hosts": ["cdn.taboola.com", "trc.taboola.com"]
}
```

`vendor` is the slug reported as `detected_vendor`. `hosts` match exactly and
also cover subdomains, so `sc-static.net` covers `cdn.sc-static.net`.
`rulepack` is present only when a first-party pack covers some of that vendor's
endpoints.

Categories in use: `social`, `search`, `programmatic`, `native`, `identity`,
`verification`, `measurement`, `video`, `analytics`, `martech`, `commerce`,
`affiliate`, `mobile`, `consent`.

## Loading your own

```rust
let mut engine = pixellint_core::Engine::default();
engine.set_directory(pixellint_core::VendorDirectory::from_path("vendors.json")?);
```

Passing `VendorDirectory::default()` disables attribution entirely.

The loader rejects duplicate hosts across entries, empty fields, empty host
lists, and unknown fields, so a directory that would silently misattribute
traffic fails to load instead.

## Accuracy

Entries are attributions, not contracts. They were compiled from vendor
documentation, tag installation guides, and first-hand tag inventories, then
spot-checked against public tracker research. Ownership changes: companies
acquire each other, endpoints move, and CDNs get shared. If an entry is wrong,
that is a bug worth reporting; it is also why attribution never carries a
severity above `info`.

Some entries name a parent company rather than the product brand when the
domain is shared across a portfolio.
