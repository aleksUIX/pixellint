# Standards

Every rule Pixellint ships, what it enforces, and where its authority comes
from. Nothing here is aspirational: if a rule is listed, it is implemented and
covered by a fixture.

## Evidence levels

| Level | Meaning |
| --- | --- |
| `normative` | A formal standard: WHATWG, W3C, IETF |
| `official_vendor` | A parameter contract the vendor publishes, cited by URL |
| `official_template` | Vendor-published templates or SDK behavior |
| `ecosystem_reference` | Consistent real-world behavior the vendor generates but does not document |
| `heuristic` | Pixellint's own judgment |

The manifest loader rejects any rule that claims `official_vendor` without a
documentation URL. Vendors change endpoints without notice; when a contract
cannot be verified in published documentation, the rule is demoted rather than
dressed up.

## `core`

Applies to every URL-like artifact: `url`, `vast`, `postback`, `request`, and
`unknown`.

| Standard | Enforced | Rule ids | Level |
| --- | --- | --- | --- |
| [WHATWG URL Standard](https://url.spec.whatwg.org/) | The artifact parses as an absolute URL and carries a host | `core.url.invalid`, `core.url.host_missing` | normative |
| [W3C Beacon](https://www.w3.org/TR/beacon/) | Network-delivered artifacts use `http` or `https` | `core.url.unsupported_scheme` | normative |
| [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986) | No embedded credentials; fragments never reach the server | `core.url.userinfo_deprecated`, `core.url.fragment_ignored` | normative |
| Secure transport baseline | Plain `http` endpoints are flagged for upgrade | `core.url.insecure_transport` | best practice |
| Input baseline | Empty artifacts are rejected before URL checks run | `core.input.empty` | heuristic |
| Macro handling | A fired URL carries no unresolved macros; macros never sit in scheme, authority, host, port, or userinfo; one artifact uses one macro syntax | `core.macro.unexpanded_in_fired_url`, `core.macro.unsafe_position`, `core.macro.mixed_syntax` | heuristic |

Macro rules recognize `[NAME]`, `${NAME}`, and `{{NAME}}`, the three syntaxes
in common ad-tech use. They are deliberately generic: per-vendor macro
vocabularies belong in vendor packs.

## `vendor/meta`

Meta Pixel browser requests to `facebook.com/tr`. Level: `official_vendor`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `id` | Required, numeric Pixel ID | `vendor.meta.param.id.missing`, `.empty`, `.invalid` |
| `ev` | Required. Unrecognized values warn, because custom events are legal | `vendor.meta.param.ev.missing`, `.empty`, `.invalid` |
| `noscript` | When present, `0` or `1` | `vendor.meta.param.noscript.invalid` |
| Unhashed PII | No parameter carries a raw email address | `vendor.meta.pii.unhashed_email` |

Sources: [pixel base code](https://developers.facebook.com/docs/meta-pixel/get-started),
[standard events](https://developers.facebook.com/docs/meta-pixel/reference),
[advanced matching](https://developers.facebook.com/docs/meta-pixel/advanced/advanced-matching).

## `vendor/google-analytics`

GA4 Measurement Protocol requests to `google-analytics.com`, including the
regional and debug endpoints. Level: `official_vendor`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `api_secret` | Required | `vendor.google-analytics.param.api_secret.missing`, `.empty` |
| `measurement_id` | When present, matches the documented `G-` form | `vendor.google-analytics.param.measurement_id.invalid` |
| Stream identity | Exactly one of `measurement_id` or `firebase_app_id` | `vendor.google-analytics.stream.identifier_missing`, `.identifier_ambiguous` |

Source: [sending events](https://developers.google.com/analytics/devguides/collection/protocol/ga4/sending-events).

Pixellint validates the request line. The Measurement Protocol payload travels
in the POST body, which Pixellint does not see.

## `vendor/floodlight`

Campaign Manager Floodlight activity tags on `doubleclick.net`. Parameters ride
on the path as semicolon-delimited pairs. Level: `official_vendor`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `src` | Required, numeric Floodlight configuration ID | `vendor.floodlight.param.src.missing`, `.empty`, `.invalid` |
| `type` | Required activity group tag | `vendor.floodlight.param.type.missing`, `.empty` |
| `cat` | Required activity tag | `vendor.floodlight.param.cat.missing`, `.empty` |
| `ord` | Required cache buster | `vendor.floodlight.param.ord.missing`, `.empty` |
| `num` | When present, not empty | `vendor.floodlight.param.num.empty` |
| Unique counting | `num` is only meaningful alongside `ord` | `vendor.floodlight.counting.unique_requires_ord` |

Source: [Floodlight tag structure](https://support.google.com/campaignmanager/answer/2823425).

## `vendor/meta-conversions-api`

Server-side events posted to the Graph API events edge. Level:
`official_vendor`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `access_token` | Required | `vendor.meta-conversions-api.param.access_token.missing`, `.empty` |
| `test_event_code` | Warns when present, since it diverts events to the test tool | `vendor.meta-conversions-api.testing.test_event_code_present` |
| Unhashed PII | No query parameter carries a raw email address | `vendor.meta-conversions-api.pii.unhashed_email` |

Source: [using the API](https://developers.facebook.com/docs/marketing-api/conversions-api/using-the-api),
[customer information parameters](https://developers.facebook.com/docs/marketing-api/conversions-api/parameters/customer-information-parameters).

## `vendor/google-tag-manager`

Container and tag loader requests on `googletagmanager.com`: `gtm.js`,
`gtag/js`, and the `ns.html` noscript iframe. Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `id` | Required. An unrecognized product prefix warns, because the prefix set is observed rather than published | `vendor.google-tag-manager.param.id.missing`, `.empty`, `.invalid` |
| `l` | When present, not empty | `vendor.google-tag-manager.param.l.empty` |

Sources: [install a web container](https://support.google.com/tagmanager/answer/14847097),
[install gtag.js](https://developers.google.com/tag-platform/gtagjs/install).

## `vendor/google-analytics-collect`

The `/g/collect` transport the Google tag uses in the browser. Level:
`ecosystem_reference`, because Google documents the tag and the Measurement
Protocol but not this request format.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `v` | Required. A value other than `2` warns, since `v=1` is Universal Analytics | `vendor.google-analytics-collect.param.v.missing`, `.empty`, `.invalid` |
| `tid` | Required. A non `G-` value warns, which is what a stale Universal Analytics property looks like | `vendor.google-analytics-collect.param.tid.missing`, `.empty`, `.invalid` |
| `cid` | Required client ID | `vendor.google-analytics-collect.param.cid.missing`, `.empty` |
| `en` | Expected event name | `vendor.google-analytics-collect.param.en.missing`, `.empty` |

Source: [GA4 collection](https://developers.google.com/analytics/devguides/collection/ga4).

## `vendor/pinterest`

Pinterest tag requests on `ct.pinterest.com`, including the noscript image.
Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `tid` | Required tag ID | `vendor.pinterest.param.tid.missing`, `.empty` |
| `event` | When present, one of the documented events. Custom names warn rather than error | `vendor.pinterest.param.event.invalid`, `.empty` |
| `noscript` | When present, `0` or `1` | `vendor.pinterest.param.noscript.invalid` |

Source: [Pinterest tag](https://developers.pinterest.com/docs/track-conversions/pinterest-tag/).

## `vendor/snapchat`

Snapchat Conversions API events requests on `tr.snapchat.com`. Level:
`official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `access_token` | Required | `vendor.snapchat.param.access_token.missing`, `.empty` |

Source: [using the API](https://developers.snap.com/marketing-api/Conversions-API/UsingTheAPI).

## `vendor/microsoft-uet`

Universal Event Tracking requests on `bat.bing.com` and `bat.bing.net`. Level:
`ecosystem_reference`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `ti` | Required numeric tag ID | `vendor.microsoft-uet.param.ti.missing`, `.empty`, `.invalid` |
| `Ver` | When present, numeric | `vendor.microsoft-uet.param.Ver.invalid`, `.empty` |
| `evt` | When present, not empty | `vendor.microsoft-uet.param.evt.empty` |

Source: [Universal Event Tracking](https://learn.microsoft.com/en-us/advertising/guides/universal-event-tracking).

Microsoft documents how to create and install a UET tag, not the request
format, which is why this pack is ecosystem evidence.

## `vendor/reddit`

Reddit Pixel requests on `alb.reddit.com`. Level: `ecosystem_reference`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `id` | Required advertiser ID | `vendor.reddit.param.id.missing`, `.empty` |
| `event` | Expected event name | `vendor.reddit.param.event.missing`, `.empty` |

Source: [verify the Reddit Pixel](https://business.reddithelp.com/en/categories/measurement/verify-reddit-pixel).

## `vendor/tiktok`

TikTok Pixel loader and collection requests on `analytics.tiktok.com`. Level:
`ecosystem_reference`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `sdkid` | Required Pixel ID | `vendor.tiktok.param.sdkid.missing`, `.empty` |
| `lib` | Expected `ttq` | `vendor.tiktok.param.lib.missing`, `.invalid` |

Source: [pixel setup](https://ads.tiktok.com/help/article/get-started-pixel),
[standard events](https://ads.tiktok.com/help/article/standard-events-parameters).

TikTok documents its events and parameters for the JavaScript and server APIs,
not the wire format of the loader URL. The pack is scoped to what its
Events Manager generates, and is labeled ecosystem evidence for that reason.

## `vendor/linkedin`

LinkedIn conversion image pixels on `px.ads.linkedin.com/collect`. Level:
`ecosystem_reference`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `pid` | Required numeric Partner ID | `vendor.linkedin.param.pid.missing`, `.empty`, `.invalid` |
| `conversionId` | Expected numeric conversion ID | `vendor.linkedin.param.conversionId.missing`, `.invalid` |
| `fmt` | Expected `gif`, `img`, or `js` | `vendor.linkedin.param.fmt.missing`, `.invalid` |

Source: [image pixel conversions](https://www.linkedin.com/help/lms/answer/a422796).

LinkedIn generates this pixel in Campaign Manager and documents the workflow
rather than the parameters, so the pack is labeled ecosystem evidence.

## Findings every manifest pack can produce

| Rule id | Severity | Meaning |
| --- | --- | --- |
| `<pack>.endpoint_mismatch` | info | The pack was selected explicitly but the artifact targets a different endpoint |
| `<pack>.claimed_vendor_mismatch` | info | The caller's `claimed_vendor` disagrees with the endpoint that matched |

## Not implemented yet

- Privacy and consent framework enforcement (GDPR signals, GPP, US Privacy)
- Macro vocabulary correctness per vendor, as opposed to generic macro handling
- Duplicate or conflicting artifacts across a document
- Document extraction: Pixellint validates artifacts a caller has already
  extracted
- VAST XML semantics beyond the tracking URLs a caller passes in

Those belong in future vendor packs, in the document-level model described in
[MULTI_ARTIFACT_SCHEMA.md](MULTI_ARTIFACT_SCHEMA.md), or in callers such as
Vastlint.

## Evidence

Rules are proven by two test layers:

- unit tests in `crates/pixellint-core/src/lib.rs` and
  `crates/pixellint-core/src/manifest.rs`
- a golden corpus in `fixtures/`, one directory per rulepack, enforced by
  `crates/pixellint-core/tests/rulepack_golden.rs`

A built-in pack without a fixture directory fails the test suite.
