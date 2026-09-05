# Changelog

All notable changes to Pixellint are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- `vendor/cm360-vast-event`, covering Campaign Manager VAST event pixels on
  `googlesyndication.com` `/ddm/activity`: required non-empty `dc_oe`. `eid1`
  is not contracted. Floodlight `src;type;cat;ord` and GAM `pagead` hops stay
  on their own packs or the directory.
- `vendor/amazon-vfw`, covering Amazon DSP Firefly DoubleVerify hops on
  `vfw.amazon-adsystem.com` `/dv/`: required `vstevt`. IAS `/ias/` hops on
  the same host stay directory-only.

### Changed

- Vendor directory splits Google and Amazon by product host group so
  `directory.no_rulepack_coverage` names the pack that actually covers sibling
  endpoints. GTM, Floodlight, Ads, Analytics, and Ad Manager are separate
  Google rows. Amazon Ad Tag, Firefly, and the remaining amazon-adsystem
  hosts are separate Amazon rows. GAM `pubads` / `googlesyndication` rows
  carry no pack pointer.
- Directory hosts for Adzerk, RTB SuperHub, Havas Edge, and Vault DCR.

## 0.17.1 - 2026-09-04

### Fixed

- Macro scanner no longer panics on non-ASCII bytes in a tracking URL
- Consent format rules skip the playground storage sentinel `REDACTED`, so a
  scrubbed `gdpr_consent` or `gpp` is not reported as a malformed TC or GPP
  string. `us_privacy=REDACTED` still warns that the deprecated param was
  present; it no longer claims the sentinel is a malformed USP string.

### Changed

- Vendor directory hosts from the 4 Sep 2026 D1 dump: Celtra, AdCanvas,
  Smartclip, Extreme Reach, XPLN, Connected Stories, IQM, Blis. Host extends
  for DoubleVerify `tpsc-video-as` / `tpsc-video-eu`, Flashtalking `d9` and
  `qa-xre.flashtalking.net`, TripleLift `s.update.3lift.com`, and LinkedIn
  RTB `/lax` hosts. Attribution only; no new rulepacks.

## 0.17.0 - 2026-09-03

### Added

- `vendor/cm360-tracking-ad`, covering Campaign Manager 360 `/ddm/trackimp`
  and `/ddm/trackclk` tags: required `dc_trk_aid` and `dc_trk_cid`
- `vendor/appsflyer-onelink-impression`, covering OneLink impression URLs on
  `impressions.onelink.me`: required template ID in the path and `pid`
- `vendor/ispot`, covering Unified Measurement impression GIFs on
  `pi.ispot.tv/v2`: required `TC-####-#` tracking code in the path

### Changed

- `vendor/adform` now matches video, impression, and click tags on every
  `*.adform.net` host that uses `/videoad`, `/C/`, or `/adfserve`, including
  the Americas domain `a2.adform.net`. Directory hosts add `a1`, `a2`, and
  `asia`.

## 0.16.0 - 2026-08-29

### Added

- `vendor/adform`, covering video, impression, and click tags on
  `track.adform.net`: required banner number `bn`
- `vendor/comscore`, covering Direct collect on `b.scorecardresearch.com/b`
  and `/p`: required `c1=2`, client ID `c2`, and page URL `c7`
- `vendor/quantcast`, covering Measure pixels on `pixel.quantserve.com/pixel`:
  required p-code `a`
- `vendor/plausible`, covering Events API JSON on `plausible.io/api/event`:
  required `name`, `url`, and `domain`
- `vendor/matomo`, covering `matomo.php` on Matomo Cloud: required `idsite`
  and `rec=1`
- `vendor/parsely`, covering the tracker loader on
  `cdn.parsely.com/keys/{site_id}/p.js`
- `vendor/crazyegg`, covering `script.crazyegg.com/pages/scripts/{account_id}/{script_id}.js`

Directory hosts from vastlint.org VAST samples: `ade.googlesyndication.com`,
`unified.adsafeprotected.com`, `vast.doubleverify.com`, plus Flashtalking,
Kantar, Epsilon, and iSpot. IAS skeleton, DV wrappers, and Floodlight `dc_oe`
fires stay attributed; those URLs have no published query contract.

## 0.15.0 - 2026-08-22

### Added

- `vendor/the-trade-desk`, covering universal pixel iframe fires on
  `insight.adsrvr.org/track/up`: required `adv`, `upid`, `ref`, and `upv`
- `vendor/criteo`, covering the OneTag loader on `dynamic.criteo.com` and
  `static.criteo.net` `/js/ld/ld.js`: required partner ID `a`
- `vendor/taboola`, covering the base pixel loader on
  `cdn.taboola.com/libtrc/unip/{account_id}/tfa.js`
- `vendor/hotjar`, covering `static.hotjar.com/c/hotjar-{hjid}.js`: required
  site ID in the path, recommended `sv`
- `vendor/hubspot`, covering `js.hs-scripts.com/{hubId}.js` and
  `js.hs-analytics.net/{hubId}.js`
- `vendor/awin`, covering `/sread.img` and `/sread.php`: required `merchant`,
  `tt`, `tv`, `amount`, `ch`, `parts`, and `ref`
- `vendor/x`, covering website tag image pixels on
  `analytics.twitter.com/i/adsct`: required `txn_id`
- `vendor/amazon-ads`, covering the Ad Tag conversion loader on
  `s.amazon-adsystem.com/iu3/conversion/{id}.js`
- `vendor/outbrain`, covering `tr.outbrain.com/pixel`: `ob_adv_id` or
  `ob_click_id`
- `vendor/baidu`, covering Tongji collect on `hm.baidu.com/hm.gif`: required
  `si`
- `vendor/kwai`, covering Pixel loaders on `s1.kwai.net`: required `sdkid`
- `vendor/hubspot-pixel`, covering `track.hubspot.com/__ptq.gif`: required `a`
- `vendor/cj`, covering `www.emjcd.com/u`: required `CID`
- `vendor/impact`, covering `utt.impactcdn.com/{UUID}.js`
- `vendor/rakuten`, covering `track.linksynergy.com/ep`: required `mid` and
  `ord`
- `vendor/brevo-js`, covering `sibautomation.com/sa.js`: required `key`

Snap Pixel stays directory-only. `scevent.min.js` has no pixel ID on the URL,
and `tr.snapchat.com/p` is a POST without a published query. Snap CAPI remains
`vendor/snapchat`. `trc.taboola.com` stays attributed.

## 0.14.0 - 2026-08-21

### Added

- `vendor/brevo`, covering Marketing Automation `trackEvent` JSON on
  `in-automate.brevo.com`: required `email` and `event`
- `vendor/rudderstack`, covering the Pixel API on hosted data planes
  (`writeKey`, `event`, and `userId` or `anonymousId`) and the HTTP Tracking
  API JSON (Segment-compatible, claimed by `context.library.name`)
- Directory entries for Brevo and RudderStack. Matomo Cloud now matches
  `*.matomo.cloud`, not only `cdn.matomo.cloud`

The Brevo JS tracker, HubSpot `__ptq.gif`, Criteo, Taboola, Outbrain, and The
Trade Desk stay directory-only.

## 0.13.0 - 2026-08-21

### Added

- `vendor/yandex-metrica`, covering Measurement Protocol requests to
  `mc.yandex.ru/collect`: required `tid`, `cid`, and `t`, page fields on
  `pageview`, and transaction fields on `pa=purchase`
- `vendor/yahoo-dot`, covering Yahoo DSP Dot image pixels on
  `sp.analytics.yahoo.com/spp.pl`: required `a` and `.yp`, hashed `he`
- `vendor/openai`, covering the Ads image tag on `bzr.openai.com/v1/sdk/events`:
  required `pid`, `event`, and `data[type]`
- `vendor/openai-conversions-api`, covering `/v1/events`: required event `id`,
  `type`, and `timestamp_ms` in milliseconds, plus `source_url` on web events
- `vendor/kochava`, covering S2S JSON on `control.kochava.com/track/json`:
  required `kochava_app_id`, `action`, `data`, and `data.event_name`
- `vendor/singular`, covering S2S EVENT on `s2s.singular.net`: required SDK key
  `a`, platform `p`, app id `i`, event name `n`, and `ip` or `use_ip`
- `vendor/google-ads-click-conversions`, covering UploadClickConversions JSON:
  required `conversionAction` and `conversionDateTime`, plus a click ID or
  hashed `userIdentifiers`

Snap Pixel, the X website tag, Amazon Ad Tag, Baidu Tongji, and Kwai stay
directory-only. Those vendors document a JS API, not an HTTP parameter contract.

## 0.12.0 - 2026-08-21

### Added

- GitHub Action (`uses: aleksUIX/pixellint@<tag>`) and musl / macOS CLI tarballs
  on GitHub Releases, the same install path RTBlint uses in CI
- `vendor/tiktok-events-api`, covering the Events API track and batch calls on
  `business-api.tiktok.com`: required `pixel_code` and `event`, ISO 8601
  timestamps, SHA-256 customer identifiers, and unhashed IP / user agent
- `vendor/adjust`, covering S2S events on `s2s.adjust.com`: required `app_token`,
  `event_token`, `s2s=1`, and a documented device ID
- `vendor/appsflyer`, covering S2S in-app events on `api3.appsflyer.com`:
  required `appsflyer_id` and `eventName`, UTC `eventTime`, hashed PII, and the
  iOS `id` path prefix
- `vendor/branch`, covering the Events API standard and custom calls: required
  `branch_key`, `name`, `user_data`, and at least one documented identifier
- `vendor/pinterest-conversions-api`, covering `api.pinterest.com` events:
  required `event_name`, `action_source`, `event_id`, `event_time` in seconds,
  and hashed `user_data`
- `vendor/reddit-conversions-api`, covering CAPI v3 on `ads-api.reddit.com`:
  required `event_at` in milliseconds, `action_source`, and `type.tracking_type`
- `vendor/x-conversions-api`, covering measurement conversions on
  `ads-api.x.com` and `ads-api.twitter.com`: required `conversion_time`,
  `event_id`, and at least one of `twclid`, `hashed_email`, or
  `hashed_phone_number`

### Fixed

- The vendor directory now points at the analytics and martech packs that
  already shipped: Amplitude, Mixpanel, PostHog, Segment, Klaviyo, Braze, and
  Adobe Analytics. `list-vendors` and `directory.no_rulepack_coverage` were
  still describing those hosts as uncovered
- ROADMAP, ARCHITECTURE, and STANDARDS caught up with what 0.6.0 through 0.11.0
  actually shipped, including GA4 Measurement Protocol body contracts
- `vendor/posthog` no longer claims TikTok Events API or Amplitude payloads.
  Those share an `event` or `api_key` field; PostHog now rules them out by the
  keys only they use
- Meta and Snap `action_source` matchers now rule out Pinterest's `web`,
  `app_android`, `app_ios`, and `offline`, so a Pinterest CAPI payload is not
  claimed as theirs

## 0.11.0 - 2026-07-26

### Added

- `vendor/segment`, covering the HTTP Tracking API single and batched: the call
  type enum, a `track` with no event name, a call with neither `userId` nor
  `anonymousId`, and a timestamp that is not ISO 8601

### Changed

- `vendor/posthog` no longer claims Segment payloads. Both post a root `event`,
  so each now rules the other out by the keys only it uses

## 0.10.0 - 2026-07-26

### Added

- Consent strings are decoded rather than pattern-matched. Base64 is permissive
  enough that `gdpr_consent=1` and `gdpr_consent=true` both passed an alphabet
  check, so the fields the specs fix are now read: the TC String version, its
  core segment length, the US Privacy version digit, and the GPP header type and
  version
- `core.privacy.tc_string_version`, which separates a TC String from a
  placeholder and reports a TCF v1 string as sunset rather than malformed
- `core.privacy.tc_string_truncated`, for a string too short to hold the fields
  the spec makes mandatory
- `core.privacy.us_privacy_version`, `core.privacy.gpp_header_type`, and
  `core.privacy.gpp_header_version`. The header type catches a TC String pasted
  into `gpp`, which is the commonest way to get this wrong

### Fixed

- Test and fixture consent strings were shortened stand-ins that the new
  truncation rule correctly rejects. They are full-length strings now

## 0.9.0 - 2026-07-26

### Added

- Five packs for the analytics and marketing tier, each contracting the JSON
  body its endpoint actually takes: `vendor/amplitude`, `vendor/posthog`,
  `vendor/mixpanel`, `vendor/klaviyo`, and `vendor/braze`. They were directory
  entries with nothing checking them
- Path patterns can address a bare root array with a leading `[]`, which is the
  shape Mixpanel posts
- The Braze REST endpoints are in the vendor directory, so a URL hitting one is
  attributed even where no pack claims it

### Changed

- Shape matching is stricter where the tier overlaps. Amplitude, Braze, and GA4
  all post an `events` array, and Braze and GA4 both use `events[].name`, so
  each pack now keys on paths that are actually its own: GA4 pairs the envelope
  with a Measurement Protocol field, and Braze keys on the identifier every
  object it accepts has to carry

Every unit here differs from its neighbours: Amplitude wants milliseconds,
PostHog and Braze want ISO 8601, Mixpanel takes either, and the three conversion
APIs from 0.7.0 want seconds, milliseconds, and microseconds respectively.

## 0.8.0 - 2026-07-26

### Added

- `vendor/meta` requires `value` and `currency` on `Purchase`. Meta documents
  both as required, and a Purchase without them reports no revenue. The
  requirement is the vendor's; the wire spelling of custom data is not
  documented, so the rule carries ecosystem evidence
- `vendor/google-analytics` contracts the Measurement Protocol request body at
  two levels: the envelope once, and each event in `events` on its own. It
  catches `timestamp_micros` in milliseconds rather than microseconds, a `value`
  with no `currency`, and the ecommerce fields Google documents as required for
  `purchase`, `refund`, `add_to_cart`, and `begin_checkout`
- A manifest's `body` may be a list of specs, so one pack can contract both the
  envelope and the elements inside it

### Fixed

- The `clean-purchase` fixture was not clean: it fired a Meta `Purchase` with no
  value or currency, which the new rule reports

## 0.7.0 - 2026-07-26

### Added

- JSON request bodies are a first-class artifact. A new `json` artifact kind,
  and an `unknown` artifact that opens like a document is read as one
- Rulepack manifests can contract a JSON body with `body`, addressing fields by
  path with `[]` for "every element". Contracts are written against one element
  of a batch and evaluated per element, so three broken events report three
  findings, each pointing at its own bytes
- `match.json_paths` claims a payload by its shape, since a bare body carries no
  host. Entries can require a path, exclude values that belong to another
  vendor, or accept any of several alternatives
- `vendor/meta-conversions-api` contracts the documented server event:
  `event_name`, `event_time` in seconds rather than milliseconds, the
  `action_source` enum, the hashed customer information fields, and the rules
  that Purchase needs value and currency, that website events need a source URL,
  and that `client_ip_address` must not arrive hashed
- `vendor/snapchat` contracts the Snap Conversions API v3 payload
- `vendor/linkedin-conversions-api`, covering conversion events sent singly or
  batched under `elements`, including the millisecond timestamp LinkedIn
  requires where Meta requires seconds
- `core.json.parse_error`, which reports a body that does not parse and names
  the byte where it stops

### Changed

- Findings about body fields carry `.body.` in their code rather than `.param.`,
  because an endpoint may accept the same field in the query string and in the
  payload under different rules
- A finding about a missing field points at the container it belongs in and
  names the exact path

## 0.6.0 - 2026-07-26

### Added

- `pixellint-wasm`: wasm-bindgen bindings over the same engine, exposing
  validation, the rulepack list, and the vendor directory
- `pixellint` npm package, WASM-backed, with TypeScript types and a
  dependency-free smoke test suite
- Playground at [pixellint.org](https://pixellint.org), served from `site/` in
  this repository. It runs entirely in the browser and sends nothing anywhere
- CI builds the WASM crate and runs the npm package tests against a fresh build,
  so the committed artifacts stay honest

## 0.5.0 - 2026-07-26

### Added

- `path_pattern` in rulepack manifests: a regular expression with named captures
  run against the path, turning path segments into contractable parameters
- `vendor/google-ads-conversion`: Google Ads conversion and view-through
  conversion image pixels, whose conversion ID rides on the path
- `vendor/adobe-analytics`: Adobe Analytics beacons, whose report suite rides on
  the path after `/b/ss/`

## 0.4.0 - 2026-07-26

Consent and privacy signals.

### Added

- Eleven `core` rules for the IAB consent signals, checked against the specs
  that define them: TCF v2 `gdpr` and `gdpr_consent` coherence and format, US
  Privacy string format plus its January 2024 deprecation, GPP string and
  section id format, and the specs' single-occurrence requirement
- Signals are read from Floodlight-style path parameters as well as the query
  string
- Meta Limited Data Use parameters `dpo`, `dpoco`, and `dpost`, with a rule for
  Meta's requirement that a country is sent with a state
- `required_when_value` rule kind for manifests: a requirement that applies only
  when another parameter carries a given value

### Notes

- Values carrying an unexpanded macro and empty values never trigger a privacy
  finding. Both are normal in templates that an ad server fills at serve time

## 0.3.0 - 2026-07-26

### Added

- Vendor endpoint directory: 89 vendors across 217 hosts, covering social,
  search, programmatic, identity, verification, measurement, analytics,
  martech, affiliate, mobile attribution, and consent platforms. An endpoint no
  rulepack covers now reports `directory.no_rulepack_coverage` at info severity
  with the vendor that owns it
- `pixellint list-vendors`, with `--json`
- `list_vendors` MCP tool, filterable by category or resolving a single host
- `Engine::set_directory` and `VendorDirectory::from_path`, so callers can
  supply or disable attribution
- `directory` is togglable like a rulepack through `--rulepack` and `--except`

### Notes

- Directory entries make one claim, that a host belongs to a vendor. They carry
  no parameter contracts, and attribution never changes an exit code

## 0.2.0 - 2026-07-26

Coverage release. Twelve rulepacks, up from six.

### Added

- `vendor/google-tag-manager`: container and tag loader requests, covering
  `gtm.js`, `gtag/js`, and the `ns.html` noscript iframe
- `vendor/google-analytics-collect`: the browser `/g/collect` transport the
  Google tag actually uses, including a check that catches Universal Analytics
  hits still pointed at a dead property
- `vendor/pinterest`: Pinterest tag requests with the documented event set
- `vendor/microsoft-uet`: Microsoft Advertising Universal Event Tracking
- `vendor/reddit`: Reddit Pixel conversion requests
- `vendor/meta-conversions-api`: the Graph API events edge, including a warning
  when `test_event_code` reaches live traffic and a raw-email guard
- `vendor/snapchat`: Snapchat Conversions API events endpoint

### Changed

- The CLI rulepack listing test now derives its expectations from the built-in
  pack list, so adding a pack cannot leave a stale assertion behind

## 0.1.0 - 2026-07-25

First release.

### Added

- `pixellint-core`: validation engine with rulepack plugins, stable rule ids,
  typed severities, evidence levels, documentation citations, and byte-offset
  targets on findings
- `core` rulepack with ten spec-backed and baseline rules covering URL
  validity, transport, credentials, fragments, empty input, and ad-tech macro
  handling
- Declarative rulepack manifests, with load-time validation of matchers,
  parameter contracts, regular expressions, rule codes, cross-references, and
  documentation citations
- First-party vendor packs: `vendor/meta`, `vendor/google-analytics`,
  `vendor/floodlight`, `vendor/tiktok`, `vendor/linkedin`
- Custom rulepack loading from disk via `--rulepack-file` and
  `Engine::register_manifest_path`
- `pixellint` CLI: `validate` and `list-rulepacks`, JSON output, inline, file,
  and stdin input, rulepack selection, vendor hints, and documented exit codes
- `pixellint-mcp`: MCP server over stdio exposing `list_rulepacks` and
  `validate_artifact`, with live rulepack ids in the tool schema and detected
  vendors in every response
- Golden fixture corpus with one directory per rulepack, plus integration tests
  that drive the real CLI binary and the real MCP transport
