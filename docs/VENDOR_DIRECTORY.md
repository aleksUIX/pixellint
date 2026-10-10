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
  info  directory.no_rulepack_coverage  This endpoint belongs to Taboola (native). No Pixellint rulepack covers it, so only the core checks ran. The `vendor/taboola` rulepack covers other Taboola endpoints, not this one.
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
  "hosts": ["cdn.taboola.com", "trc.taboola.com"],
  "rulepack": "vendor/taboola"
}
```

`vendor` is the slug reported as `detected_vendor`. `hosts` match exactly and
also cover subdomains, so `sc-static.net` covers `cdn.sc-static.net`.
`rulepack` is present only when a first-party pack covers some of that vendor's
endpoints. One vendor slug can have several rows (Google Tag Manager vs Google
Ad Manager) so the pointer names the pack that actually covers sibling hosts.

Categories in use: `social`, `search`, `programmatic`, `native`, `identity`,
`verification`, `measurement`, `video`, `analytics`, `martech`, `commerce`,
`affiliate`, `mobile`, `consent`.

## Loading your own

```rust
let mut engine = pixellint_core::Engine::default();
engine.set_directory(pixellint_core::VendorDirectory::from_path("vendors.json")?);
```

Passing `VendorDirectory::default()` disables attribution entirely.

## Overlay files

`--directory-file` and `Engine::merge_directory` add entries to the built-in
directory. The file uses the same `{ "entries": [...] }` shape. A host that
the built-in directory already claims is rejected, so an overlay cannot steal
attribution. New vendors and extra hosts on a new vendor slug are the point.

```bash
pixellint validate url 'https://px.acme.example/collect' --directory-file extra.json
pixellint list-vendors --directory-file extra.json
```

Contribute first-party hosts through a PR to `tools/build-directory.py` and
`directory.json`. Runtime overlays are for private pixels and local
corrections. How to contribute: [CONTRIBUTING.md](../CONTRIBUTING.md).

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

## Live VAST hosts reviewed on October 9, 2026

The tester findings supplied for this review attribute these exact endpoints:

- Adxspace: `00px.net`. The reported tag identifies SPACE ADSERVER and an
  `adxspace` ad ID.
- Stredeo: `t.stredeo.com`. The reported tag identifies Stredeo.
- Beeswax: `us-east-1.event.prod.bidr.io`.
- Aarki: `rm.aarki.net`.
- AdWrap: `track.adwrap.io`.
- Krush Media: `ads106.krushmedia.com` and `ads133.krushmedia.com`.

These observed video tracking routes remain outside vendor rulepacks. Their ownership evidence
comes from the supplied live-tag inventory, rather than a published parameter
contract. Generic URL, macro and privacy checks still run. No vendor parameters
are inferred from frequency, paths or VAST event names. FreeWheel's
[Beeswax cookie-sync documentation](https://api-docs.freewheel.tv/advertiser/docs/cookie-syncing)
independently identifies Beeswax's `prod.bidr.io` infrastructure, but does not
define contracts for the reported `/log/act/svr` and `/log/imp/svr` endpoints.

### Additional hosts confirmed in the Pixellint corpus

A subsequent private corpus review supplied complete hostnames for three more
vendors and additional endpoints for two existing entries:

- Adtelligent: `ads55.adtelligent.com`, `ads228.adtelligent.com` and
  `ads283.adtelligent.com`.
- Smadex: `br-trk.smadex.com`, `cr-err.smadex.com`, `ec-ed.smadex.com`,
  `geo-tracker.smadex.com`, `pixel-ed.smadex.com` and `va-trk.smadex.com`.
- Sovrn: `n.adv.lijit.com`.
- Krush Media: `ads161.krushmedia.com`.
- Beeswax: `segment.prod.bidr.io`.

The corpus establishes which exact hosts occurred. Primary vendor sources
corroborate attribution: Adtelligent's
[official AdPush examples](https://support.adtelligent.com/838838-AdPush-requestresponse-format-and-examples)
use its `adtelligent.com` infrastructure; Smadex's
[official website](https://smadex.com/)
identifies its advertising platform on `smadex.com`;
Sovrn's
[Lijit documentation](https://knowledge.sovrn.com/kb/can-i-remove-the-lijit-com-lines-from-my-ads-txt-file)
explicitly identifies `lijit.com` as Sovrn's operational ad-serving domain;
Krush Media's [privacy policy](https://krushmedia.com/privacy-policy/)
identifies its advertising services on `krushmedia.com`; and the Beeswax
cookie-sync documentation linked above identifies its `prod.bidr.io`
infrastructure. These sources support host attribution, without documenting
the contracts of these observed tracking endpoints.

All additions remain directory-only. Adtelligent's
[required-parameter guide](https://support.adtelligent.com/347898-Required-Parameters-for-different-traffic-types)
governs incoming ad requests, so its required dimensions, user agent and IP
fields are not imposed on `/t/e/` or `/tre/imp/` tracking pixels. Smadex's
[public documentation index](https://docs.smadex.com/llms.txt)
covers MMP integrations rather than the observed video tracking routes.
No complete event enumeration, required identifier or privacy requirement is
inferred from a sampled path or a corporate privacy policy.

The directory includes only these observed hosts, not entire corporate domains
or guessed regional siblings. Core URL, macro and privacy checks continue to
run, including rejection of populated `gdpr=NaN` values. At version 0.37.0, RZR Global,
rtactivate and ActiveMetering remained deferred because independent attribution
for their exact tracking hosts was not established. Talpa's private mock host
does not establish a public vendor contract.

## The October 10 review adds bounded tracker coverage.

The `00px.net` directory entry now points to `vendor/adxspace-pixel` for the
published `/pixel/{token}/e.gif` counting-pixel template. Its observed
`/tracking/` and `/vast/pixel/` video routes remain outside that pack. The
[00px installation guide](https://wiki.00px.com.br/en/technologies/pixel)
independently corroborates this host and the narrower counting-pixel route.

`track.adctv.com` is attributed to ADCTV and points to `vendor/adctv-tracker`.
The [official website](https://www.adctv.com/) links its studio, whose generated
embed code loads the [public tag producer](https://ads.adctv.com/scripts/src/tag.ins/tag.ins.js).
That source names the exact tracker host and appends an event query field to
the root URL in both sendBeacon and POST fetch branches. The pack provides
template completeness advisories; it does not define a closed event list or
certify the server accepts a request.

`track.activemetering.com` is attributed to DISQO, without a vendor rulepack.
Both [Roku's approved-partner list](https://help.ads.roku.com/en/articles/10386309-measuring-campaigns-with-third-party-solutions)
and [Edmunds' accepted-vendor list](https://www.edmunds.com/media-kit/ad-specs/third-party.html)
explicitly map this exact host to DISQO. Neither source documents the observed
`/pixel/v1/all/pixel.gif` parameter contract.

The RZR Global and rtactivate endpoints remain unattributed. The supplied
Index attribution for `idxgm.rtactivate.com` is unconfirmed. The reviewed
sources and remaining endpoint schemas are recorded in
[the tracker review](reviews/vast-tracker-depth/programmatic.json).
