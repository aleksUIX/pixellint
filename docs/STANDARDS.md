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
`unknown`. A `json` artifact, or an `unknown` one that opens like a document,
gets the syntax check instead.

| Standard | Enforced | Rule ids | Level |
| --- | --- | --- | --- |
| [WHATWG URL Standard](https://url.spec.whatwg.org/) | The artifact parses as an absolute URL and carries a host | `core.url.invalid`, `core.url.host_missing` | normative |
| [W3C Beacon](https://www.w3.org/TR/beacon/) | Network-delivered artifacts use `http` or `https` | `core.url.unsupported_scheme` | normative |
| [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986) | No embedded credentials; fragments never reach the server | `core.url.userinfo_deprecated`, `core.url.fragment_ignored` | normative |
| Secure transport baseline | Plain `http` endpoints are flagged for upgrade | `core.url.insecure_transport` | best practice |
| Input baseline | Empty artifacts are rejected before URL checks run | `core.input.empty` | heuristic |
| [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259) | A JSON body parses, and the finding names the byte where it stops | `core.json.parse_error` | normative |
| Macro handling | A fired URL carries no unresolved macros; macros never sit in scheme, authority, host, port, or userinfo; one artifact uses one macro syntax | `core.macro.unexpanded_in_fired_url`, `core.macro.unsafe_position`, `core.macro.mixed_syntax` | heuristic |

Macro rules recognize `[NAME]`, `${NAME}`, and `{{NAME}}`, the three syntaxes
in common ad-tech use. They are deliberately generic: per-vendor macro
vocabularies belong in vendor packs.

### Consent and privacy signals

IAB Tech Lab specifies these parameters, every party in the chain is expected to
carry them, and the failure modes do not vary by vendor, so they are `core`
rules rather than vendor ones. They are read from the query string and from
Floodlight-style path parameters.

The strings are decoded, not just pattern-matched. Base64 is a permissive
alphabet: `gdpr_consent=1` and `gdpr_consent=true` are both well-formed base64
segments, and both pass any check that only looks at characters. What separates
a consent string from a string is the fields the specs fix.

| Field read | Spec says | Rule id |
| --- | --- | --- |
| TC String Version, first 6 bits | "the value is 2 for this format" | `core.privacy.tc_string_version` |
| TC String core segment length | The fields through PublisherCC need 213 bits, so 36 characters is the floor | `core.privacy.tc_string_truncated` |
| US Privacy version, first character | Version 1 is the only one published | `core.privacy.us_privacy_version` |
| GPP header type, first 6 bits | "Fixed to 3 as GPP Header field" | `core.privacy.gpp_header_type` |
| GPP header version, next 6 bits | Currently 1 | `core.privacy.gpp_header_version` |

A TCF v1 string decodes to version 1 and is reported as sunset rather than as
malformed, since it is a real consent string that no v2 vendor can read. A TC
String pasted into `gpp` decodes to header type 2 and is reported as such,
because putting the right string in the wrong parameter is the commonest way to
get this wrong.

Section IDs are not cross-checked against the GPP string's own section list.
`gpp_sid` carries "the section ID(s) in force for the current transaction",
which the spec does not require to match the sections the string contains.

| Standard | Enforced | Rule ids | Level |
| --- | --- | --- | --- |
| [TCF v2](https://github.com/InteractiveAdvertisingBureau/GDPR-Transparency-and-Consent-Framework/blob/master/TCFv2/IAB%20Tech%20Lab%20-%20Consent%20string%20and%20vendor%20list%20formats%20v2.md) | `gdpr` is `0` or `1` | `core.privacy.gdpr_invalid` | normative |
| TCF v2 | `gdpr=1` is accompanied by a TC String | `core.privacy.gdpr_consent_missing` | normative |
| TCF v2 | A TC String without a `gdpr` flag leaves the callee guessing | `core.privacy.gdpr_consent_without_flag` | normative |
| TCF v2 | A TC String is only meaningful when `gdpr=1` | `core.privacy.gdpr_consent_ignored` | normative |
| TCF v2 | The TC String is URL-safe base64 | `core.privacy.gdpr_consent_malformed` | normative |
| [US Privacy](https://github.com/InteractiveAdvertisingBureau/USPrivacy/blob/master/CCPA/US%20Privacy%20String.md) | The string is a version digit and three `Y`, `N`, or `-` characters | `core.privacy.us_privacy_malformed` | normative |
| US Privacy | The signal was deprecated on 31 January 2024 in favor of GPP | `core.privacy.us_privacy_deprecated` | normative |
| [GPP](https://github.com/InteractiveAdvertisingBureau/Global-Privacy-Platform/blob/main/Core/Consent%20String%20Specification.md) | The GPP string is URL-safe base64 with `~` between sections | `core.privacy.gpp_malformed` | normative |
| GPP | `gpp` is accompanied by the section IDs in force | `core.privacy.gpp_sid_missing` | normative |
| GPP | `gpp_sid` carries one section ID, at most two separated by a comma | `core.privacy.gpp_sid_malformed` | normative |
| TCF v2, GPP | Each signal appears only once in a URL | `core.privacy.duplicate_signal` | normative |

Two deliberate exemptions keep these rules quiet where trafficking is correct:
a value carrying an unexpanded macro is the macro rules' business, and an empty
value is an unfilled template slot, which is how Floodlight and VAST tags ship
before an ad server populates them.

## `vendor/meta`

Meta Pixel browser requests to `facebook.com/tr`. Level: `official_vendor`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `id` | Required, numeric Pixel ID | `vendor.meta.param.id.missing`, `.empty`, `.invalid` |
| `ev` | Required. Unrecognized values warn, because custom events are legal | `vendor.meta.param.ev.missing`, `.empty`, `.invalid` |
| `noscript` | When present, `0` or `1` | `vendor.meta.param.noscript.invalid` |
| `dpo` | Limited Data Use is enabled with `LDU` | `vendor.meta.param.dpo.invalid` |
| `dpoco` | `1` for the United States, `0` to let Meta geolocate | `vendor.meta.param.dpoco.invalid` |
| `dpost` | Numeric state code, or `0` to let Meta geolocate | `vendor.meta.param.dpost.invalid` |
| Limited Data Use | A country requires a state, otherwise Meta geolocates instead | `vendor.meta.ldu.country_without_state` |
| Unhashed PII | No parameter carries a raw email address | `vendor.meta.pii.unhashed_email` |

Sources: [pixel base code](https://developers.facebook.com/docs/meta-pixel/get-started),
[standard events](https://developers.facebook.com/docs/meta-pixel/reference),
[advanced matching](https://developers.facebook.com/docs/meta-pixel/advanced/advanced-matching),
[data processing options](https://developers.facebook.com/docs/marketing-apis/data-processing-options).

### Purchase value

Meta documents `currency` and `value` as required for `Purchase`. It does not
document how the browser pixel spells custom data on the wire, so the parameter
names are ecosystem evidence even though the requirement is Meta's own.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `cd[value]` | Required on `Purchase` | `vendor.meta.purchase_requires_value_and_currency` |
| `cd[currency]` | Required on `Purchase`, ISO 4217 three-letter code | `vendor.meta.purchase_requires_value_and_currency`, `vendor.meta.param.cd[currency].invalid` |

Source: [standard events reference](https://developers.facebook.com/docs/meta-pixel/reference).

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

The event payload is checked per event in `data`. Codes carry `.body.` to keep
them apart from the query parameters above, because the endpoint accepts some
fields in either place.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_name` | Required | `vendor.meta-conversions-api.body.event_name.missing`, `.empty` |
| `event_time` | Required, exactly 10 digits, since Meta documents seconds and a 13-digit value is milliseconds | `vendor.meta-conversions-api.body.event_time.missing`, `.invalid` |
| `action_source` | Required, one of the nine documented values | `vendor.meta-conversions-api.body.action_source.missing`, `.invalid` |
| `user_data` | Required | `vendor.meta-conversions-api.body.user_data.missing`, `.empty` |
| `event_id` | Expected, for deduplication against the browser pixel | `vendor.meta-conversions-api.body.event_id.missing` |
| Hashed identifiers | `em`, `ph`, `fn`, `ln`, `ge`, `db`, `ct`, `st`, `zp`, `country`, `external_id` must be SHA-256 hex digests, in either the scalar or the list form | `vendor.meta-conversions-api.body.user_data.<field>.invalid` |
| `fbc`, `fbp` | Must match the documented `fb.N.timestamp.value` shape | `vendor.meta-conversions-api.body.user_data.fbc.invalid`, `.fbp.invalid` |
| Purchase events | `custom_data.value` and `custom_data.currency` are required | `vendor.meta-conversions-api.body.purchase_requires_value_and_currency` |
| Website events | `event_source_url` is required when `action_source` is `website` | `vendor.meta-conversions-api.body.website_requires_source_url` |
| Limited Data Use | `data_processing_options_country` is required when `LDU` is sent | `vendor.meta-conversions-api.body.ldu_requires_country` |
| Unhashed PII | No field carries a raw email address | `vendor.meta-conversions-api.body.unhashed_email` |
| Over-hashing | `client_ip_address` and `client_user_agent` must not be digests | `vendor.meta-conversions-api.body.hashed_plaintext_field` |

Source: [using the API](https://developers.facebook.com/docs/marketing-api/conversions-api/using-the-api),
[server event parameters](https://developers.facebook.com/docs/marketing-api/conversions-api/parameters/server-event),
[customer information parameters](https://developers.facebook.com/docs/marketing-api/conversions-api/parameters/customer-information-parameters),
[custom data](https://developers.facebook.com/docs/marketing-api/conversions-api/parameters/custom-data).

The hashed identifier contracts accept upper-case hex as well as lower-case.
Meta documents lower-casing the input before hashing, not the digest, so
rejecting an upper-case digest would be inventing a requirement.

### Measurement Protocol payload

The request body is contracted at two levels: the envelope once, and each event
in `events` on its own.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `client_id` | Expected, since events without it are not joined to a user | `vendor.google-analytics.body.client_id.missing` |
| `events` | Flagged when present and empty, which sends nothing | `vendor.google-analytics.body.events.empty` |
| `timestamp_micros` | Exactly 16 digits, since Google documents microseconds and a 13-digit value is milliseconds | `vendor.google-analytics.body.timestamp_micros.invalid` |
| `non_personalized_ads` | Deprecated in favor of the `consent` object | `vendor.google-analytics.body.non_personalized_ads.deprecated` |
| `events[].name` | Required; 40 characters or fewer warns when longer | `vendor.google-analytics.body.name.missing`, `.invalid` |
| Value without currency | `currency` is required whenever `value` is set | `vendor.google-analytics.body.value_requires_currency` |
| `purchase` | Needs `currency`, `value`, `transaction_id`, and `items` | `vendor.google-analytics.body.purchase_requires_ecommerce_fields` |
| `refund` | Needs `currency`, `value`, and `transaction_id` | `vendor.google-analytics.body.refund_requires_ecommerce_fields` |
| `add_to_cart`, `begin_checkout` | Need `currency`, `value`, and `items` | `vendor.google-analytics.body.cart_requires_ecommerce_fields` |

Source: [Measurement Protocol reference](https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference),
[recommended events](https://developers.google.com/analytics/devguides/collection/ga4/reference/events).

A payload with no `events` at all carries nothing that identifies it as GA4, so
it is not claimed and not reported on. An `events` that is present and empty is
identifiable, and is flagged.

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

## `vendor/google-ads-conversion`

Google Ads conversion and view-through conversion image pixels. The conversion
ID travels in the path. Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `conversion_id` | Required numeric ID, read from the path | `vendor.google-ads-conversion.param.conversion_id.missing`, `.empty`, `.invalid` |
| `label` | Expected. Without it the hit lands on the account rather than a conversion action | `vendor.google-ads-conversion.param.label.missing`, `.empty` |
| `guid` | The generated tag sends `ON` | `vendor.google-ads-conversion.param.guid.invalid` |
| `script` | The image fallback sends `0` | `vendor.google-ads-conversion.param.script.invalid` |

Source: [Google Ads conversion tracking errors](https://support.google.com/tagassistant/answer/2947038).

## `vendor/adobe-analytics`

Adobe Analytics data collection beacons on `omtrdc.net` and `2o7.net`. The
report suite rides on the path after `/b/ss/`. Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `report_suite` | Required, read from the path | `vendor.adobe-analytics.param.report_suite.missing`, `.empty` |
| `mid` | When present, not empty | `vendor.adobe-analytics.param.mid.empty` |

Sources: [identify your tracking server and report suites](https://experienceleague.adobe.com/en/docs/analytics-learn/tutorials/implementation/implementation-basics/how-to-identify-your-analytics-tracking-server-and-report-suites),
[A4T reporting](https://experienceleague.adobe.com/en/docs/target-dev/developer/server-side/integration/a4t-reporting).

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

The event payload is checked per event in `data`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_name` | Required, one of the documented upper-case event types | `vendor.snapchat.body.event_name.missing`, `.invalid` |
| `event_time` | Required epoch timestamp | `vendor.snapchat.body.event_time.missing`, `.invalid` |
| `action_source` | Required, one of `WEB`, `OFFLINE`, `MOBILE_APP` | `vendor.snapchat.body.action_source.missing`, `.invalid` |
| `user_data` | Required | `vendor.snapchat.body.user_data.missing`, `.empty` |
| Hashed identifiers | `em`, `ph`, `fn`, `ln`, `ge`, `ct`, `st`, `zp`, `country` must be SHA-256 hex digests | `vendor.snapchat.body.user_data.<field>.invalid` |
| Web events | `event_source_url` is required when `action_source` is `WEB` | `vendor.snapchat.body.web_requires_source_url` |
| Unhashed PII | No field carries a raw email address | `vendor.snapchat.body.unhashed_email` |
| Over-hashing | `client_ip_address` and `client_user_agent` must not be digests | `vendor.snapchat.body.hashed_plaintext_field` |

Source: [using the API](https://developers.snap.com/api/marketing-api/Conversions-API/UsingTheAPI),
[parameters](https://developers.snap.com/api/marketing-api/Conversions-API/Parameters).

Snap and Meta post the same envelope, down to the field names. A bare payload
carries no host, so the two packs tell each other apart by `action_source`:
Snap writes `WEB` where Meta writes `website`. A payload whose `action_source`
is missing or misspelled matches both, and both report it.

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

## `vendor/linkedin-conversions-api`

Conversion events streamed to `api.linkedin.com/rest/conversionEvents`, either
as a single event or as a batch under `elements`. Level: `official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `conversion` | Required, in the form `urn:lla:llaPartnerConversion:ID` | `vendor.linkedin-conversions-api.body.conversion.missing`, `.invalid` |
| `conversionHappenedAt` | Required, exactly 13 digits, since LinkedIn documents milliseconds and a 10-digit value is seconds | `vendor.linkedin-conversions-api.body.conversionHappenedAt.missing`, `.invalid` |
| `user.userIds` | Required, even when matching on `lead`, `externalIds`, or `userInfo`, where it is sent as an empty list | `vendor.linkedin-conversions-api.body.user.userIds.missing` |
| `user.userIds[].idType` | Required; unrecognized types warn rather than error | `vendor.linkedin-conversions-api.body.user.userIds[].idType.missing`, `.invalid` |
| `user.userIds[].idValue` | Required and non-empty | `vendor.linkedin-conversions-api.body.user.userIds[].idValue.missing`, `.empty` |
| `conversionValue` | An amount needs a `currencyCode` | `vendor.linkedin-conversions-api.body.value_needs_both_fields` |
| Unhashed PII | No identifier carries a raw email address | `vendor.linkedin-conversions-api.body.unhashed_email` |

Source: [Conversions API](https://learn.microsoft.com/en-us/linkedin/marketing/integrations/ads-reporting/conversions-api).

The `idType` list is taken from LinkedIn's own validation error, which may not
be exhaustive, so an unfamiliar value is a warning rather than an error.

## `vendor/amplitude`

Event uploads to the Amplitude HTTP V2 API on `amplitude.com`. Level:
`official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `api_key` | Required | `vendor.amplitude.body.api_key.missing`, `.empty` |
| `events` | Required | `vendor.amplitude.body.events.missing`, `.empty` |
| `events[].event_type` | Required | `vendor.amplitude.body.event_type.missing` |
| `user_id`, `device_id` | 5 characters or more, which Amplitude documents as the minimum it accepts | `vendor.amplitude.body.user_id.invalid`, `.device_id.invalid` |
| `time` | 13 digits, since Amplitude documents milliseconds | `vendor.amplitude.body.time.invalid` |
| Identity | One of `user_id` or `device_id` is required | `vendor.amplitude.body.event_needs_an_identifier` |

Source: [HTTP V2 API](https://amplitude.com/docs/apis/analytics/http-v2).

## `vendor/posthog`

Capture requests to PostHog, single or batched under `batch`. Level:
`official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `api_key` | Required | `vendor.posthog.body.api_key.missing`, `.empty` |
| `event` | Required, per event | `vendor.posthog.body.event.missing`, `.empty` |
| `distinct_id` | Required, per event | `vendor.posthog.body.distinct_id.missing`, `.empty` |
| `timestamp` | ISO 8601, since an epoch number is read as the ingestion time instead | `vendor.posthog.body.timestamp.invalid` |

Source: [capture API](https://posthog.com/docs/api/capture).

## `vendor/mixpanel`

Ingestion requests to the Mixpanel track endpoint, which posts a bare array of
events rather than an envelope. Level: `official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event` | Required, per event | `vendor.mixpanel.body.event.missing`, `.empty` |
| `properties` | Required, since the project token rides inside it | `vendor.mixpanel.body.properties.missing`, `.empty` |
| `properties.token` | Required | `vendor.mixpanel.body.properties.token.missing` |
| `properties.distinct_id` | Flagged when present and empty | `vendor.mixpanel.body.properties.distinct_id.empty` |
| `properties.$insert_id` | Flagged when present and empty | `vendor.mixpanel.body.properties.$insert_id.empty` |

Source: [track event](https://docs.mixpanel.com/reference/track-event).

## `vendor/klaviyo`

Event creation on the Klaviyo events API, in JSON:API shape. Level:
`official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `data.type` | Required, and must be `event` | `vendor.klaviyo.body.data.type.missing`, `.invalid` |
| Metric name | Required at `data.attributes.metric.data.attributes.name` | `vendor.klaviyo.body.data.attributes.metric.data.attributes.name.missing`, `.empty` |
| Profile identity | One of id, email, phone number, or external id is required | `vendor.klaviyo.body.profile_needs_an_identifier` |

Source: [create event](https://developers.klaviyo.com/en/reference/create_event).

## `vendor/braze`

Attribute, event, and purchase uploads to the Braze `/users/track` endpoint.
Level: `official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `events[].name` | Required | `vendor.braze.body.name.missing`, `.empty` |
| `events[].time`, `purchases[].time` | Required, ISO 8601 datetime | `vendor.braze.body.time.missing`, `.invalid` |
| `purchases[].product_id` | Required | `vendor.braze.body.product_id.missing` |
| `purchases[].currency` | Required, ISO 4217 three-letter code | `vendor.braze.body.currency.missing`, `.invalid` |
| `purchases[].price` | Required | `vendor.braze.body.price.missing` |
| Identity | Every event and purchase needs one of `external_id`, `user_alias`, `braze_id`, `email`, or `phone` | `vendor.braze.body.event_needs_an_identifier`, `.purchase_needs_an_identifier` |

Source: [POST /users/track](https://www.braze.com/docs/api/endpoints/user_data/post_user_track/).

An object carrying no identifier at all is only reported when something else in
the payload identifies it as Braze's, since the identifier fields are part of
what tells this endpoint's payload from another's.

## `vendor/segment`

Calls to the Segment HTTP Tracking API on `api.segment.io` and the regional
`segmentapis.com` hosts, single or batched under `batch`. Level:
`official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `writeKey` | Flagged when present and empty. Segment also accepts it as basic auth, so it is not required in the body | `vendor.segment.body.writeKey.empty` |
| `batch[].type` | Required, and one of identify, track, page, screen, group, alias | `vendor.segment.body.type.missing`, `.invalid` |
| Track calls | A `track` in a batch needs an `event` | `vendor.segment.body.track_requires_an_event_name` |
| Identity | Every call needs `userId` or `anonymousId` | `vendor.segment.body.call_needs_an_identifier` |
| `timestamp` | ISO 8601 date string | `vendor.segment.body.timestamp.invalid` |

Source: [HTTP API source](https://segment.com/docs/connections/sources/catalog/libraries/server/http-api/),
[track spec](https://segment.com/docs/connections/spec/track/).

The rendered documentation returns 403 to automated fetches, so the contract was
read from the source the site is built from, Segment's own published docs
repository. The citation points at the live page.

A single call carries its type in the URL path rather than the body, so the
`type` contract is scoped to the batch and stays quiet on a single call.

Segment and PostHog both post a root `event`, and a bare body has no URL to tell
them apart. Each rules the other out by the keys only it uses: `writeKey`,
`userId`, and `anonymousId` for Segment, `api_key` and `distinct_id` for
PostHog.

## Vendor directory

The directory attributes endpoints no rulepack claims. It asserts only that a
host belongs to a vendor, so it produces one finding and never raises severity
above `info`.

| Rule id | Severity | Meaning |
| --- | --- | --- |
| `directory.no_rulepack_coverage` | info | The endpoint belongs to a known vendor that no rulepack covers |

Full behavior: [VENDOR_DIRECTORY.md](VENDOR_DIRECTORY.md).

## Findings every manifest pack can produce

| Rule id | Severity | Meaning |
| --- | --- | --- |
| `<pack>.endpoint_mismatch` | info | The pack was selected explicitly but the artifact targets a different endpoint |
| `<pack>.payload_mismatch` | info | The pack was selected explicitly but the JSON body does not have the shape its endpoint accepts |
| `<pack>.claimed_vendor_mismatch` | info | The caller's `claimed_vendor` disagrees with the endpoint that matched |

## Not implemented yet

- HubSpot. The endpoint in the directory is the `__ptq.gif` tracking pixel, and
  HubSpot documents the `_hsq` client-side calls rather than the request that
  pixel makes, so its parameters have no citable contract
- TikTok Events API and Pinterest Conversions API payloads. Both endpoints are
  in the directory and both vendors have browser packs, but their request
  schemas are published behind rendered documentation portals that could not be
  read at the time of writing. Writing the contracts from memory would mean
  citing documentation nobody checked, so the packs stay URL-only until the
  schemas can be read
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
