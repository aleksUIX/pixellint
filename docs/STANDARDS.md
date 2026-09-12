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

### Advanced Matching

Meta requires customer information to be normalized and SHA-256 hashed. It
documents the hashing, not the browser query names. `em`, `ph`, `fn`, `ln`,
`external_id`, `fbc`, and `fbp` are ecosystem evidence for that reason.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `em`, `ph`, `fn`, `ln`, `external_id` | When present, SHA-256 hex | `vendor.meta.param.em.invalid`, `.ph.invalid`, `.fn.invalid`, `.ln.invalid`, `.external_id.invalid` |
| `fbc`, `fbp` | When present, `fb.N.timestamp.value` | `vendor.meta.param.fbc.invalid`, `.fbp.invalid` |

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
| `qty`, `cost` | When present, not empty | `vendor.floodlight.param.qty.empty`, `.cost.empty` |
| `dc_lat` | When present, `0` or `1` | `vendor.floodlight.param.dc_lat.invalid` |
| `u1`–`u20` | When present, not empty | `vendor.floodlight.param.u1.empty` through `.u20.empty` |
| Unique counting | `num` is only meaningful alongside `ord` | `vendor.floodlight.counting.unique_requires_ord` |

Campaign Manager documents custom variables through `u100`. This pack
contracts `u1`–`u20`. `u21`–`u100` have the same shape and are not enumerated.

Source: [Floodlight tag structure](https://support.google.com/campaignmanager/answer/2823425).

## `vendor/cm360-tracking-ad`

Campaign Manager 360 impression and click tracking tags on
`doubleclick.net` `/ddm/trackimp` and `/ddm/trackclk`. Parameters ride on the
path as semicolon-delimited pairs. Level: `official_vendor`. Floodlight
activity tags (`src;type;cat;ord`) stay `vendor/floodlight`. GAM/CM360 VAST
`dc_oe` event pixels are `vendor/cm360-vast-event`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `dc_trk_aid` | Required numeric tracking ad ID | `vendor.cm360-tracking-ad.param.dc_trk_aid.missing`, `.empty`, `.invalid` |
| `dc_trk_cid` | Required numeric tracking creative ID | `vendor.cm360-tracking-ad.param.dc_trk_cid.missing`, `.empty`, `.invalid` |

Source: [Tagging issues in Campaign Manager 360](https://support.google.com/campaignmanager/answer/2829774). Placement-tag examples with `/trackimp` and `/trackclk` are in [placement tags](https://support.google.com/campaignmanager/answer/2826636).

## `vendor/cm360-vast-event`

Campaign Manager 360 VAST event pixels on `googlesyndication.com`
`/ddm/activity`, whose event payload rides as `dc_oe`. Level:
`ecosystem_reference`. Google generates these in exported VAST and does not
publish a parameter table for `dc_oe` or `eid1`. The pack requires `dc_oe`
non-empty so a truncated export is visible. `eid1` is not contracted.
Floodlight `src;type;cat;ord` tags stay `vendor/floodlight`. GAM
`pagead/interaction` and `pcs/view` stay directory-only.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `dc_oe` | Required, non-empty event payload | `vendor.cm360-vast-event.param.dc_oe.missing`, `.empty` |

Source: [placement tags](https://support.google.com/campaignmanager/answer/2826636), observed CM360-exported VAST.

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

## `vendor/google-ads-click-conversions`

Click conversions posted to `googleads.googleapis.com` `UploadClickConversions`.
Level: `official_vendor`. The image pixel is a different pack.

The payload is checked per conversion in `conversions`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `conversions` | Required | `vendor.google-ads-click-conversions.body.conversions.missing` |
| `partialFailure` | Required `true` | `vendor.google-ads-click-conversions.body.partialFailure.missing`, `.invalid` |
| `conversionAction` | Required resource name | `vendor.google-ads-click-conversions.body.conversionAction.missing`, `.empty` |
| `conversionDateTime` | Required, `yyyy-mm-dd hh:mm:ss+|-hh:mm` | `vendor.google-ads-click-conversions.body.conversionDateTime.missing`, `.invalid` |
| Click ID or user | One of `gclid`, `gbraid`, `wbraid`, or `userIdentifiers` | `vendor.google-ads-click-conversions.body.click_id_or_user_required` |
| Hashed PII | `hashedEmail` and `hashedPhoneNumber` must be SHA-256 | `vendor.google-ads-click-conversions.body.userIdentifiers[].<field>.invalid` |
| Unhashed PII | No field carries a raw email address | `vendor.google-ads-click-conversions.body.unhashed_email` |

Sources: [upload offline conversions](https://developers.google.com/google-ads/api/docs/conversions/upload-offline),
[ClickConversion](https://developers.google.com/google-ads/api/reference/rpc/v24/ClickConversion).

Call conversions on `UploadCallConversions` are `vendor/google-ads-call-conversions`.
Conversion adjustments on `UploadConversionAdjustments` are
`vendor/google-ads-conversion-adjustments`.

## `vendor/google-ads-call-conversions`

Call conversions posted to `googleads.googleapis.com` `UploadCallConversions`.
Level: `official_vendor`. Click uploads stay `vendor/google-ads-click-conversions`.
The image pixel is a different pack.

The payload is checked per conversion in `conversions`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `conversions` | Required | `vendor.google-ads-call-conversions.body.conversions.missing` |
| `partialFailure` | Required `true` | `vendor.google-ads-call-conversions.body.partialFailure.missing`, `.invalid` |
| `conversionAction` | Required resource name | `vendor.google-ads-call-conversions.body.conversionAction.missing`, `.empty` |
| `callerId` | Required E.164 with a leading `+` | `vendor.google-ads-call-conversions.body.callerId.missing`, `.invalid` |
| `callStartDateTime` | Required, `yyyy-mm-dd hh:mm:ss+|-hh:mm` | `vendor.google-ads-call-conversions.body.callStartDateTime.missing`, `.invalid` |
| `conversionDateTime` | Required, same timestamp shape | `vendor.google-ads-call-conversions.body.conversionDateTime.missing`, `.invalid` |
| `currencyCode` | ISO 4217 three-letter code when present | `vendor.google-ads-call-conversions.body.currencyCode.invalid` |

Source: [upload call conversions](https://developers.google.com/google-ads/api/docs/conversions/upload-calls).

## `vendor/google-ads-conversion-adjustments`

Conversion adjustments posted to `googleads.googleapis.com`
`UploadConversionAdjustments`. Level: `official_vendor`. Click uploads stay
`vendor/google-ads-click-conversions`. Call conversions stay
`vendor/google-ads-call-conversions`.

The payload is checked per adjustment in `conversionAdjustments`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `conversionAdjustments` | Required | `vendor.google-ads-conversion-adjustments.body.conversionAdjustments.missing` |
| `partialFailure` | Required `true` | `vendor.google-ads-conversion-adjustments.body.partialFailure.missing`, `.invalid` |
| `conversionAction` | Required resource name | `vendor.google-ads-conversion-adjustments.body.conversionAction.missing`, `.empty` |
| `adjustmentType` | Required `RETRACTION`, `RESTATEMENT`, or `ENHANCEMENT` | `vendor.google-ads-conversion-adjustments.body.adjustmentType.missing`, `.invalid` |
| `adjustmentDateTime` | Required, `yyyy-mm-dd hh:mm:ss+|-hh:mm` | `vendor.google-ads-conversion-adjustments.body.adjustmentDateTime.missing`, `.invalid` |
| Identity | One of `orderId` or `gclidDateTimePair.gclid` | `vendor.google-ads-conversion-adjustments.body.order_or_gclid_required` |
| Restatement value | `RESTATEMENT` needs `restatementValue.adjustedValue` | `vendor.google-ads-conversion-adjustments.body.restatement_requires_value` |

Source: [import conversion adjustments](https://developers.google.com/google-ads/api/docs/conversions/upload-adjustments).

## `vendor/adobe-analytics`

Adobe Analytics data collection beacons on `omtrdc.net` and `2o7.net`. The
report suite rides on the path after `/b/ss/`. Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `report_suite` | Required, read from the path | `vendor.adobe-analytics.param.report_suite.missing`, `.empty` |
| `mid` | When present, not empty | `vendor.adobe-analytics.param.mid.empty` |
| `g` | When present, an absolute URL | `vendor.adobe-analytics.param.g.invalid` |
| `pageName` / `gn` | When present, not empty | `vendor.adobe-analytics.param.pageName.empty` |
| `events` / `ev` | When present, not empty | `vendor.adobe-analytics.param.events.empty` |
| `products` / `pl` | When present, not empty | `vendor.adobe-analytics.param.products.empty` |
| `pe` | When present, `lnk_o`, `lnk_d`, `lnk_e`, or `tnt` | `vendor.adobe-analytics.param.pe.invalid` |
| Truncated request | `AQB` requires `AQE` | `vendor.adobe-analytics.truncated_request` |

Sources: [query parameters](https://experienceleague.adobe.com/en/docs/analytics/implementation/validate/query-parameters),
[identify your tracking server and report suites](https://experienceleague.adobe.com/en/docs/analytics-learn/tutorials/implementation/implementation-basics/how-to-identify-your-analytics-tracking-server-and-report-suites),
[A4T reporting](https://experienceleague.adobe.com/en/docs/target-dev/developer/server-side/integration/a4t-reporting).

Edge Network interact and collect are `vendor/adobe-web-sdk`. Direct Visitor
ID Service calls on `dpm.demdex.net/id` are `vendor/adobe-ecid`.

## `vendor/adobe-ecid`

Adobe Experience Cloud ID Service direct integration on `dpm.demdex.net/id`.
Level: `official_vendor`. Audience Manager `/event` and cookie sync on
`cm.everesttech.net` are not contracted.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `d_ver` | Required `2` | `vendor.adobe-ecid.param.d_ver.missing`, `.invalid` |
| Identifier | One of `d_orgid` or `d_mid` | `vendor.adobe-ecid.identifier_required` |

Sources: [direct integration](https://experienceleague.adobe.com/en/docs/id-service/using/implementation/direct-integration),
[direct integration use cases](https://experienceleague.adobe.com/en/docs/id-service/using/implementation/direct-integration-examples).

## `vendor/adobe-web-sdk`

Adobe Experience Platform Edge Network `interact` and `collect` on
`edge.adobedc.net` and `server.adobedc.net`. Level: `official_vendor`.
AppMeasurement `/b/ss/` stays `vendor/adobe-analytics`.

`interact` posts a single `event`. `collect` posts `events[]`. Both carry the
same XDM contract.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `datastreamId` | Required. v1 alias `configId` | `vendor.adobe-web-sdk.param.datastreamId.missing`, `.empty` |
| `xdm.timestamp` | Required ISO 8601 date-time | `vendor.adobe-web-sdk.body.xdm.timestamp.missing`, `.invalid` |
| `xdm.eventType` | Recommended | `vendor.adobe-web-sdk.body.xdm.eventType.missing`, `.empty` |

Sources: [interact](https://developer.adobe.com/data-collection-apis/docs/endpoints/interact/),
[collect](https://developer.adobe.com/data-collection-apis/docs/endpoints/collect/).

## `vendor/pinterest`

Pinterest tag requests on `ct.pinterest.com`, including the noscript image.
Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `tid` | Required tag ID | `vendor.pinterest.param.tid.missing`, `.empty` |
| `event` | When present, one of the documented events. Custom names warn rather than error | `vendor.pinterest.param.event.invalid`, `.empty` |
| `noscript` | When present, `0` or `1` | `vendor.pinterest.param.noscript.invalid` |

Source: [Pinterest tag](https://developers.pinterest.com/docs/track-conversions/pinterest-tag/).

## `vendor/pinterest-conversions-api`

Server-side events posted to `api.pinterest.com/v5/ad_accounts/{ad_account_id}/events`.
Level: `official_vendor`.

The ad account ID rides on the path. The event payload is checked per event in
`data`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_name` | Required | `vendor.pinterest-conversions-api.body.event_name.missing`, `.empty` |
| `action_source` | Required, one of `web`, `app_android`, `app_ios`, `offline` | `vendor.pinterest-conversions-api.body.action_source.missing`, `.invalid` |
| `event_id` | Required, for deduplication against the tag | `vendor.pinterest-conversions-api.body.event_id.missing`, `.empty` |
| `event_time` | Required Unix timestamp in seconds. A 13-digit value is milliseconds | `vendor.pinterest-conversions-api.body.event_time.missing`, `.invalid` |
| `user_data` | Required, with at least `em`, `hashed_maids`, or `client_ip_address` | `vendor.pinterest-conversions-api.body.user_data.missing`, `.user_needs_an_identifier` |
| Hashed identifiers | `em`, `ph`, `external_id`, `hashed_maids` must be SHA-256 hex digests | `vendor.pinterest-conversions-api.body.user_data.<field>.invalid` |
| Unhashed PII | No field carries a raw email address | `vendor.pinterest-conversions-api.body.unhashed_email` |
| Over-hashing | `client_ip_address` and `client_user_agent` must not be digests | `vendor.pinterest-conversions-api.body.hashed_plaintext_field` |

Source: [track conversion events in the API](https://developers.pinterest.com/docs/track-conversions/track-conversions-in-the-api/).

Custom event names are allowed, so `event_name` is not enum-checked. Pinterest
spells `web` in lower case; that is how this pack is told apart from Meta
(`website`) and Snap (`WEB`).

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

Snap, Meta, and Pinterest post the same `data[]` envelope. A bare payload
carries no host, so the packs tell each other apart by `action_source`:
Snap writes `WEB`, Meta writes `website`, Pinterest writes `web`. A payload
whose `action_source` is missing or misspelled matches more than one, and
each reports it.

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

## `vendor/microsoft-conversions-api`

Server-side UET events posted to
`capi.uet.microsoft.com/v1/{tagId}/events`. Level: `official_vendor`. Browser
UET is `vendor/microsoft-uet`. Clarity is `vendor/microsoft-clarity`.

The tag ID rides on the path. The event payload is checked per event in
`data`. Microsoft documents `eventTime` as Unix seconds, so a 13-digit value
is milliseconds.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `tag_id` | Required numeric UET tag ID in the path | `vendor.microsoft-conversions-api.param.tag_id.missing`, `.empty`, `.invalid` |
| `eventType` | Required `pageLoad` or `custom` | `vendor.microsoft-conversions-api.body.eventType.missing`, `.invalid` |
| `eventTime` | Required Unix seconds, 10 digits | `vendor.microsoft-conversions-api.body.eventTime.missing`, `.invalid` |
| `eventId` | Recommended for UET and CAPI deduplication | `vendor.microsoft-conversions-api.body.eventId.missing` |
| `userData` | Required | `vendor.microsoft-conversions-api.body.userData.missing` |
| User identifiers | At least one of `anonymousId`, `externalId`, `em`, `ph`, `msclkid`, `idfa`, `gaid` | `vendor.microsoft-conversions-api.body.user_needs_an_identifier` |
| `eventSourceUrl` | Required on `pageLoad` | `vendor.microsoft-conversions-api.body.page_load_requires_url` |
| `userData.em` | SHA-256 hex when present; raw email is an error | `vendor.microsoft-conversions-api.body.userData.em.invalid`, `.unhashed_email` |

Source: [Conversions API](https://learn.microsoft.com/en-us/advertising/guides/uet-conversion-api-integration).

## `vendor/microsoft-clarity`

Microsoft Clarity tag loader on `www.clarity.ms/tag/{projectId}`. Level:
`official_template`. Microsoft documents copying the generated tracking code
and comparing `src`. Collect POSTs on `/collect` are not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `project_id` | Required in the path | `vendor.microsoft-clarity.param.project_id.missing`, `.empty` |

Source: [setup](https://learn.microsoft.com/en-us/clarity/setup-and-installation/clarity-setup),
[troubleshooting](https://learn.microsoft.com/en-us/clarity/setup-and-installation/troubleshooting-installation).

## `vendor/reddit`

Reddit Pixel requests on `alb.reddit.com`. Level: `ecosystem_reference`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `id` | Required advertiser ID | `vendor.reddit.param.id.missing`, `.empty` |
| `event` | Expected event name | `vendor.reddit.param.event.missing`, `.empty` |

Source: [verify the Reddit Pixel](https://business.reddithelp.com/en/categories/measurement/verify-reddit-pixel).

## `vendor/reddit-conversions-api`

CAPI v3 events posted to `ads-api.reddit.com/api/v3/pixels/{pixel_id}/conversion_events`.
Level: `official_vendor`.

The Pixel ID rides on the path. The event payload is checked per event in
`data.events`. The v2 envelope (`events` at the root, ISO 8601 `event_at`) is
a different shape and is not contracted here.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_at` | Required, exactly 13 digits, since v3 documents milliseconds | `vendor.reddit-conversions-api.body.event_at.missing`, `.invalid` |
| `action_source` | Required, one of `WEBSITE`, `APP`, `PHYSICAL_STORE`, `OTHER` | `vendor.reddit-conversions-api.body.action_source.missing`, `.invalid` |
| `type.tracking_type` | Required; unrecognized types warn rather than error | `vendor.reddit-conversions-api.body.type.tracking_type.missing`, `.invalid` |
| Match key | At least one of `click_id`, `user.email`, `phone_number`, `uuid`, `external_id`, `ip_address`, `idfa`, `aaid` | `vendor.reddit-conversions-api.body.match_key_required` |
| `event_source_url` | Recommended on `WEBSITE` | `vendor.reddit-conversions-api.body.website_requires_source_url` |
| `type.custom_event_name` | Required when `tracking_type` is `CUSTOM` | `vendor.reddit-conversions-api.body.custom_requires_name` |
| `metadata.value` | Recommended on `PURCHASE` | `vendor.reddit-conversions-api.body.purchase_requires_value` |
| Over-hashing | `user.ip_address` and `user.user_agent` must not be digests | `vendor.reddit-conversions-api.body.hashed_plaintext_field` |

Sources: [direct integration](https://ads-api-reddit.netlify.app/docs/v3/guides/programs/capi/direct-integration),
[v2 to v3 migration](https://ads-api-reddit.netlify.app/docs/v3/guides/programs/capi/migration),
[About the Conversions API](https://business.reddithelp.com/s/article/Conversions-API).

Reddit accepts email and phone unhashed or SHA-256 hashed, so this pack does
not require a digest on those fields.

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

## `vendor/tiktok-events-api`

Server-to-server track and batch requests to `business-api.tiktok.com`. Level:
`official_vendor`.

The track call posts one event at the root. The batch call posts the same
event object under `batch`, with `pixel_code` on the envelope.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `pixel_code` | Required Pixel ID | `vendor.tiktok-events-api.body.pixel_code.missing`, `.empty` |
| `event` | Required conversion event name, per event | `vendor.tiktok-events-api.body.event.missing`, `.empty` |
| `timestamp` | ISO 8601, since an epoch number is stamped as the arrival time instead | `vendor.tiktok-events-api.body.timestamp.invalid` |
| `context.user.email`, `phone_number`, `external_id` | SHA-256 hex digests | `vendor.tiktok-events-api.body.context.user.<field>.invalid` |
| `context.ip`, `context.user_agent` | Sent unhashed | `vendor.tiktok-events-api.body.hashed_plaintext_field` |
| Unhashed PII | No field carries a raw email address | `vendor.tiktok-events-api.body.unhashed_email` |
| `properties.currency` | ISO 4217 three-letter code when present | `vendor.tiktok-events-api.body.properties.currency.invalid` |
| Identifier | Hashed email, phone, `external_id`, or `context.ip` | `vendor.tiktok-events-api.body.user_needs_an_identifier` |
| `CompletePayment`, `PlaceAnOrder` | Need `properties.value` and `properties.currency` | `vendor.tiktok-events-api.body.complete_payment_requires_value_and_currency` |

Sources: [where to find pixel_code](https://ads.tiktok.com/marketing_api/docs?id=1739584855420929),
[event deduplication](https://ads.tiktok.com/marketing_api/docs?id=1739584864945154),
[official Events API SDK models](https://github.com/tiktok/tiktok-business-api-sdk/blob/main/js_sdk/docs/PixelTrackBody.md).

The `/open_api/v1.3/event/track/` Events 2.0 envelope is `vendor/tiktok-events-2`.

## `vendor/tiktok-events-2`

Events API 2.0 track requests to `business-api.tiktok.com`
`/open_api/v1.3/event/track/`. Level: `official_vendor`. The Events 1.0
`/pixel/track` envelope stays `vendor/tiktok-events-api`.

The envelope is checked once. Each element of `data` is checked on its own.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_source` | Required `web`, `app`, `offline`, or `crm` | `vendor.tiktok-events-2.body.event_source.missing`, `.invalid` |
| `event_source_id` | Required Pixel Code or event-set ID | `vendor.tiktok-events-2.body.event_source_id.missing`, `.empty` |
| `data` | Required event array | `vendor.tiktok-events-2.body.data.missing` |
| `event` | Required conversion name, per event | `vendor.tiktok-events-2.body.event.missing`, `.empty` |
| `event_time` | Unix seconds, 10 digits | `vendor.tiktok-events-2.body.event_time.missing`, `.invalid` |
| `event_id` | Recommended when the Pixel also fires | `vendor.tiktok-events-2.body.event_id.missing` |
| `page.url` | Recommended absolute URL on web events | `vendor.tiktok-events-2.body.page.url.missing`, `.invalid` |
| `user.email`, `phone`, `external_id` | SHA-256 hex digests | `vendor.tiktok-events-2.body.user.<field>.invalid` |
| `user.ip`, `user.user_agent` | Sent unhashed | `vendor.tiktok-events-2.body.hashed_plaintext_field` |
| Unhashed PII | No field carries a raw email address | `vendor.tiktok-events-2.body.unhashed_email` |
| Identifier | Hashed email, phone, `external_id`, `ttclid`, or IP | `vendor.tiktok-events-2.body.user_needs_an_identifier` |
| `Purchase`, `CompletePayment`, `PlaceAnOrder` | Need `properties.value` and `properties.currency` | `vendor.tiktok-events-2.body.purchase_requires_value_and_currency` |

Source: [report app, web, offline, or CRM events](https://business-api.tiktok.com/portal/docs/report-app-web-offline-or-crm-events/v1.3).

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

Identify is `vendor/amplitude-identify`. Group identify is
`vendor/amplitude-group-identify`.

## `vendor/amplitude-identify`

Amplitude Identify API on `/identify`. Level: `official_vendor`. HTTP V2 stays
`vendor/amplitude`. GET puts the fields on the query. POST uses the same names
as form fields.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `api_key` | Required | `vendor.amplitude-identify.param.api_key.missing`, `.empty` |
| `identification` | Required JSON object or array | `vendor.amplitude-identify.param.identification.missing`, `.empty` |

Source: [Identify API](https://amplitude.com/docs/apis/analytics/identify).

## `vendor/amplitude-group-identify`

Amplitude Group Identify API on `/groupidentify`. Level: `official_vendor`.
User identify stays `vendor/amplitude-identify`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `api_key` | Required | `vendor.amplitude-group-identify.param.api_key.missing`, `.empty` |
| `identification` | Required JSON object or array | `vendor.amplitude-group-identify.param.identification.missing`, `.empty` |

Source: [Group Identify API](https://amplitude.com/docs/apis/analytics/group-identify).

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

Adobe Edge interact payloads carry `event.xdm`. This pack excludes that
shape so they stay `vendor/adobe-web-sdk`.

## `vendor/mixpanel`

Ingestion requests to the Mixpanel track endpoint, which posts a bare array of
events rather than an envelope. Level: `official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event` | Required, per event | `vendor.mixpanel.body.event.missing`, `.empty` |
| `properties` | Required, since the project token rides inside it | `vendor.mixpanel.body.properties.missing`, `.empty` |
| `properties.token` | Required | `vendor.mixpanel.body.properties.token.missing`, `.empty` |
| `properties.distinct_id` | Recommended | `vendor.mixpanel.body.properties.distinct_id.missing`, `.empty` |
| `properties.$insert_id` | Recommended | `vendor.mixpanel.body.properties.$insert_id.missing`, `.empty` |
| `properties.time` | When present, an integer Unix timestamp | `vendor.mixpanel.body.properties.time.invalid` |

`/import` is `vendor/mixpanel-import`. `/engage` is `vendor/mixpanel-engage`.
This pack matches `/track` and a JSON array whose events carry `properties.token`.

Source: [track event](https://docs.mixpanel.com/reference/track-event).

## `vendor/mixpanel-import`

Batch event imports posted to Mixpanel `/import`. Level: `official_vendor`.
`/track` stays `vendor/mixpanel`. `/engage` stays `vendor/mixpanel-engage`.
`/groups` is `vendor/mixpanel-groups`. Auth is a header or basic auth, not a
JSON field.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `strict` | Recommended `0` or `1` | `vendor.mixpanel-import.param.strict.missing`, `.invalid` |
| `event` | Required, per event | `vendor.mixpanel-import.body.event.missing`, `.empty` |
| `properties` | Required | `vendor.mixpanel-import.body.properties.missing` |
| `properties.time` | Required integer Unix timestamp | `vendor.mixpanel-import.body.properties.time.missing`, `.invalid` |
| `properties.distinct_id` | Required. Empty string is allowed | `vendor.mixpanel-import.body.properties.distinct_id.missing` |
| `properties.$insert_id` | Required | `vendor.mixpanel-import.body.properties.$insert_id.missing`, `.empty` |

Source: [import events](https://docs.mixpanel.com/reference/import-events).

## `vendor/mixpanel-engage`

Mixpanel user profile updates posted to `/engage`. Level: `official_vendor`.
`/track` stays `vendor/mixpanel`. `/import` stays `vendor/mixpanel-import`.
`/groups` is `vendor/mixpanel-groups`.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `verbose` | Recommended `0` or `1` | `vendor.mixpanel-engage.param.verbose.missing`, `.invalid` |
| `$token` | Required project token | `vendor.mixpanel-engage.body.$token.missing`, `.empty` |
| `$distinct_id` | Required | `vendor.mixpanel-engage.body.$distinct_id.missing`, `.empty` |
| Operation | One of `$set`, `$set_once`, `$add`, `$union`, `$append`, `$remove`, `$unset`, `$delete` | `vendor.mixpanel-engage.body.operation_required` |

Source: [profile set](https://docs.mixpanel.com/reference/profile-set).

## `vendor/mixpanel-groups`

Mixpanel group profile updates posted to `/groups`. Level: `official_vendor`.
`/track` stays `vendor/mixpanel`. `/engage` stays `vendor/mixpanel-engage`.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `verbose` | Recommended `0` or `1` | `vendor.mixpanel-groups.param.verbose.missing`, `.invalid` |
| `$token` | Required project token | `vendor.mixpanel-groups.body.$token.missing`, `.empty` |
| `$group_key` | Required group type | `vendor.mixpanel-groups.body.$group_key.missing`, `.empty` |
| `$group_id` | Required group instance | `vendor.mixpanel-groups.body.$group_id.missing`, `.empty` |
| Operation | One of `$set`, `$set_once`, `$add`, `$union`, `$remove`, `$unset`, `$delete` | `vendor.mixpanel-groups.body.operation_required` |

Source: [group set](https://docs.mixpanel.com/reference/group-set-property).

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

## `vendor/adjust`

Server-to-server events on `s2s.adjust.com/event`, as query or form parameters.
Level: `official_vendor`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `app_token` | Required | `vendor.adjust.param.app_token.missing`, `.empty` |
| `event_token` | Required | `vendor.adjust.param.event_token.missing`, `.empty` |
| `s2s` | Required, and must be `1` | `vendor.adjust.param.s2s.missing`, `.invalid` |
| Device ID | One of `idfa`, `gps_adid`, or the other documented device IDs | `vendor.adjust.device_id_required` |
| `created_at` | ISO 8601 when present | `vendor.adjust.param.created_at.invalid` |
| Over-hashing | `ip_address` must not be a digest | `vendor.adjust.hashed_plaintext_field` |

Source: [S2S events](https://dev.adjust.com/en/api/s2s-api/events/).

## `vendor/appsflyer`

In-app events posted to `api3.appsflyer.com/inappevent/{app_id}`. Level:
`official_vendor`.

The app ID rides on the path. iOS IDs must be prefixed with `id`; without it
the call still returns 200 and the event is not recorded.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `app_id` | Required, from the path. Digits-only IDs warn that the iOS prefix is missing | `vendor.appsflyer.param.app_id.missing`, `.empty`, `.ios_app_id_unprefixed` |
| `appsflyer_id` | Required | `vendor.appsflyer.body.appsflyer_id.missing`, `.empty` |
| `eventName` | Required | `vendor.appsflyer.body.eventName.missing`, `.empty` |
| `eventTime` | UTC as `yyyy-mm-dd hh:mm:ss.sss` when present | `vendor.appsflyer.body.eventTime.invalid` |
| Hashed PII | `email_hashed`, `phone_number_hashed`, and name fields must be SHA-256 | `vendor.appsflyer.body.<field>.invalid` |
| Unhashed PII | No field carries a raw email address | `vendor.appsflyer.body.unhashed_email` |
| Over-hashing | `ip` must not be a digest | `vendor.appsflyer.body.hashed_plaintext_field` |

Source: [S2S events API 3](https://dev.appsflyer.com/hc/reference/s2s-events-api3-overview).

`eventValue` is documented as required, including as an empty string when there
is no value. Empty is a legal payload, so the pack does not contract it.

## `vendor/appsflyer-onelink-impression`

OneLink view-through impression URLs on `impressions.onelink.me/{template_id}`.
Level: `official_vendor`. The S2S in-app event API stays `vendor/appsflyer`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `template_id` | Required in the path. Letters, digits, and underscores | `vendor.appsflyer-onelink-impression.param.template_id.missing`, `.empty`, `.invalid` |
| `pid` | Required media source | `vendor.appsflyer-onelink-impression.param.pid.missing`, `.empty` |

Source: [Create deep linking and redirection links with OneLink](https://support.appsflyer.com/hc/en-us/articles/208874366).

## `vendor/branch`

Standard and custom events posted to `api2.branch.io/v2/event/standard` and
`/v2/event/custom`. Level: `official_vendor`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `branch_key` | Required | `vendor.branch.body.branch_key.missing`, `.empty` |
| `name` | Required | `vendor.branch.body.name.missing`, `.empty` |
| `user_data` | Required | `vendor.branch.body.user_data.missing` |
| Identity | At least one of `developer_identity`, `browser_fingerprint_id`, `idfa`, `idfv`, `android_id`, or `aaid` | `vendor.branch.body.user_needs_an_identifier` |
| Over-hashing | `user_data.ip` must not be a digest | `vendor.branch.body.hashed_plaintext_field` |

Source: [Events API](https://help.branch.io/developers-hub/reference/events-api).

## `vendor/x-conversions-api`

Website conversions posted to `ads-api.x.com` and `ads-api.twitter.com`
`/{version}/measurement/conversions/{pixel_id}`. Level: `official_vendor`.

The Pixel ID rides on the path. The event payload is checked per event in
`conversions`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `conversion_time` | Required ISO 8601 timestamp | `vendor.x-conversions-api.body.conversion_time.missing`, `.invalid` |
| `event_id` | Required conversion event UUID from Ads Manager | `vendor.x-conversions-api.body.event_id.missing`, `.empty` |
| Identifiers | At least one of `twclid`, `hashed_email`, or `hashed_phone_number`. IP and user agent are not enough on their own | `vendor.x-conversions-api.body.identifier_required` |
| Hashed PII | `hashed_email` and `hashed_phone_number` must be SHA-256 hex digests | `vendor.x-conversions-api.body.identifiers[].<field>.invalid` |
| Unhashed PII | No field carries a raw email address | `vendor.x-conversions-api.body.unhashed_email` |
| Over-hashing | `ip_address` and `user_agent` must not be digests | `vendor.x-conversions-api.body.hashed_plaintext_field` |

Source: [conversion API](https://developer.twitter.com/en/docs/twitter-ads-api/measurement/api-reference/conversions).

## `vendor/yahoo-dot`

Yahoo DSP Dot image pixels on `sp.analytics.yahoo.com/spp.pl`. Level:
`official_vendor`. The contract is the instrumentation code Yahoo returns from
the pixels API.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `a` | Required project ID | `vendor.yahoo-dot.param.a.missing`, `.empty` |
| `.yp` | Required numeric pixel ID | `vendor.yahoo-dot.param..yp.missing`, `.empty`, `.invalid` |
| `he` | SHA-256 hex digest when present | `vendor.yahoo-dot.param.he.invalid` |
| Unhashed PII | No parameter carries a raw email address | `vendor.yahoo-dot.unhashed_email` |

Sources: [pixels](https://help.yahooinc.com/dsp-api/docs/pixels),
[enhanced matching](https://help.yahooinc.com/identity/docs/enhanced-matching).

## `vendor/yandex-metrica`

Measurement Protocol requests to `mc.yandex.ru/collect`. Level:
`official_vendor`. The browser tag on `/watch` is `vendor/yandex-watch`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `tid` | Required numeric tag ID | `vendor.yandex-metrica.param.tid.missing`, `.empty`, `.invalid` |
| `cid` | Required ClientID | `vendor.yandex-metrica.param.cid.missing`, `.empty` |
| `t` | Required `pageview` or `event` | `vendor.yandex-metrica.param.t.missing`, `.invalid` |
| Pageview fields | `dl`, `dr`, and `dt` when `t=pageview` | `vendor.yandex-metrica.pageview_requires_page_fields` |
| Purchase fields | `ti` and `tr` when `pa=purchase` | `vendor.yandex-metrica.purchase_requires_transaction` |
| `ms` | Expected Measurement Protocol token | `vendor.yandex-metrica.param.ms.missing`, `.empty` |

Source: [uploading data](https://yandex.com/dev/metrika/en/data-import/measurement-upload).

The parameter table marks `ea` and `pa` as required on `event`, but the official
goal examples omit `pa` and the ecommerce examples omit `ea`. Those fields are
checked when present, not required on every event.

## `vendor/yandex-watch`

Yandex Metrica browser counter hits to `mc.yandex.ru/watch/{counter_id}`.
Level: `official_vendor`. Measurement Protocol `/collect` stays
`vendor/yandex-metrica`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `counter_id` | Required numeric tag ID in the path | `vendor.yandex-watch.param.counter_id.missing`, `.empty`, `.invalid` |

Source: [installing a tag on a site with CSP](https://yandex.com/support/metrica/en/code/install-counter-csp).

## `vendor/openai`

OpenAI Ads image tag requests to `bzr.openai.com/v1/sdk/events`. Level:
`official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `pid` | Required Pixel ID | `vendor.openai.param.pid.missing`, `.empty` |
| `event` | Required documented event name | `vendor.openai.param.event.missing`, `.invalid` |
| `data[type]` | Required data shape | `vendor.openai.param.data[type].missing`, `.empty` |
| Custom name | `custom_event_name` when `event=custom` | `vendor.openai.custom_requires_name` |

Sources: [image tag](https://developers.openai.com/ads/image-tag),
[supported events](https://developers.openai.com/ads/supported-events).

## `vendor/openai-conversions-api`

Server-side events posted to `bzr.openai.com/v1/events`. Level:
`official_vendor`. The payload is checked per event in `events`.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `pid` | Required on the URL | `vendor.openai-conversions-api.param.pid.missing`, `.empty` |
| `id` | Required event id | `vendor.openai-conversions-api.body.id.missing`, `.empty` |
| `type` | Required documented event type | `vendor.openai-conversions-api.body.type.missing`, `.invalid` |
| `timestamp_ms` | Required, exactly 13 digits | `vendor.openai-conversions-api.body.timestamp_ms.missing`, `.invalid` |
| Web events | `source_url` when `action_source` is `web` | `vendor.openai-conversions-api.body.web_requires_source_url` |

Source: [Conversions API](https://developers.openai.com/ads/conversions-api).

## `vendor/kochava`

Post-install events posted as JSON to `control.kochava.com/track/json`. Level:
`official_vendor`.

| Body field | Enforced | Rule ids |
| --- | --- | --- |
| `kochava_app_id` | Required | `vendor.kochava.body.kochava_app_id.missing`, `.empty` |
| `action` | Required | `vendor.kochava.body.action.missing`, `.empty` |
| `data` | Required | `vendor.kochava.body.data.missing` |
| `data.event_name` | Required | `vendor.kochava.body.data.event_name.missing`, `.empty` |

Source: [post-install event setup](https://support.kochava.com/articles/server-to-server-integration/185-post-install-event-setup/).

The article's field table and JSON sample disagree on whether `device_ids` and
`origination_ip` sit at the root or inside `data`. Those fields are not
required here.

## `vendor/singular`

EVENT requests to `s2s.singular.net/api/v1/evt` and `/api/v2/evt`. Level:
`official_vendor`. Parameters are query or form fields, not JSON.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `a` | Required SDK Key | `vendor.singular.param.a.missing`, `.empty` |
| `p` | Required documented platform spelling | `vendor.singular.param.p.missing`, `.invalid` |
| `i` | Required app identifier | `vendor.singular.param.i.missing`, `.empty` |
| `n` | Required, 1 to 32 ASCII characters | `vendor.singular.param.n.missing`, `.invalid` |
| IP | Exactly one of `ip` or `use_ip` | `vendor.singular.ip_required`, `.ip_ambiguous` |
| Over-hashing | `ip` must not be a digest | `vendor.singular.hashed_plaintext_field` |

Source: [S2S EVENT endpoint](https://support.singular.net/hc/en-us/articles/31496864868635-Server-to-Server-EVENT-Endpoint-API-Reference).

## `vendor/brevo`

Marketing Automation `trackEvent` posts to `in-automate.brevo.com` and
`in-automate.sendinblue.com`. Level: `official_vendor`. The JavaScript tracker
on `sibautomation.com` is `vendor/brevo-js`.

| Body field | Enforced | Rule ids |
| --- | --- | --- |
| `email` | Required | `vendor.brevo.body.email.missing`, `.empty` |
| `event` | Required | `vendor.brevo.body.event.missing`, `.empty` |

Source: [track custom events (REST)](https://developers.brevo.com/docs/track-custom-events-rest).

`properties` and `eventdata` are optional. The identify endpoint is a different
call and is not contracted here.

## `vendor/rudderstack`

HTTP Tracking API JSON, plus Pixel API GET `/pixel/v1/track` on hosted
data planes. Level: `official_vendor`. Self-hosted data planes on other hosts
stay directory-only.

The HTTP API is Segment-compatible. Bare payloads are told apart by
`context.library.name` as in RudderStack's HTTP samples (`http`), and by
leaving `writeKey` out of the JSON body (Pixel and HTTP POST put it in the
query or in basic auth).

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `writeKey` | Required on the pixel URL | `vendor.rudderstack.param.writeKey.missing`, `.empty` |
| Pixel `event` | Required on `/pixel/v1/track` | `vendor.rudderstack.param.event.missing`, `.empty` |
| Pixel identity | `userId` or `anonymousId` | `vendor.rudderstack.identifier_required` |
| Batch `type` | Required documented method | `vendor.rudderstack.body.type.missing`, `.invalid` |
| Track `event` | Required when `type` is `track` | `vendor.rudderstack.body.track_requires_an_event_name` |
| Call identity | `userId` or `anonymousId` | `vendor.rudderstack.body.call_needs_an_identifier` |
| `timestamp` | ISO 8601 when present | `vendor.rudderstack.body.timestamp.invalid` |

Sources: [HTTP API](https://www.rudderstack.com/docs/api/http-api/),
[Pixel API](https://www.rudderstack.com/docs/api/pixel-api/).

## `vendor/the-trade-desk`

Universal pixel iframe fires to `insight.adsrvr.org/track/up`. Level:
`official_vendor`. The JS loader on `js.adsrvr.org` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `adv` | Required advertiser ID | `vendor.the-trade-desk.param.adv.missing`, `.empty` |
| `upid` | Required universal pixel ID | `vendor.the-trade-desk.param.upid.missing`, `.empty` |
| `ref` | Required absolute page URL | `vendor.the-trade-desk.param.ref.missing`, `.empty`, `.invalid` |
| `upv` | Required pixel version | `vendor.the-trade-desk.param.upv.missing`, `.empty` |

Source: [universal pixel](https://open.thetradedesk.com/provider/docsApp/GuidesProvider/data/doc/TrackingTagsUniversalPixel).

## `vendor/criteo`

Criteo OneTag loader on `dynamic.criteo.com` and `static.criteo.net`
`/js/ld/ld.js`. Level: `official_vendor`. Cookie sync on `gum.criteo.com` is
not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `a` | Required numeric Partner ID | `vendor.criteo.param.a.missing`, `.empty`, `.invalid` |

Source: [OneTag](https://developers.criteo.com/retailer-integration/docs/onetag).

## `vendor/taboola`

Taboola base pixel loader on `cdn.taboola.com/libtrc/unip/{account_id}/tfa.js`.
Level: `official_vendor`. Browser events are `vendor/taboola-unip`. S2S
postbacks are `vendor/taboola-s2s`. Bulk submit is `vendor/taboola-s2s-bulk`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `account_id` | Required numeric Account ID in the path | `vendor.taboola.param.account_id.missing` |

Source: [add the base pixel manually](https://developers.taboola.com/pixel/docs/add-the-base-pixel-manually).

## `vendor/taboola-s2s`

Taboola S2S conversion postbacks on
`trc.taboola.com/actions-handler/log/3/s2s-action`. Level: `official_vendor`. The
base pixel loader stays `vendor/taboola`. Bulk submit is `vendor/taboola-s2s-bulk`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `click-id` | Required Click ID | `vendor.taboola-s2s.param.click-id.missing`, `.empty` |
| `name` | Required Realize Event Name | `vendor.taboola-s2s.param.name.missing`, `.empty` |
| `revenue` | Integer or decimal when present | `vendor.taboola-s2s.param.revenue.invalid` |
| `currency` | Documented three-letter code when present | `vendor.taboola-s2s.param.currency.invalid` |
| `quantity` | Integer when present | `vendor.taboola-s2s.param.quantity.invalid` |
| `orderid` | Non-empty when present | `vendor.taboola-s2s.param.orderid.empty` |

Source: [the S2S postback URL](https://developers.taboola.com/pixel/docs/the-postback-url).

## `vendor/taboola-s2s-bulk`

Taboola bulk S2S conversions posted to
`trc.taboola.com/{account-id}/log/3/bulk-s2s-action`. Level: `official_vendor`.
Single postbacks stay `vendor/taboola-s2s`.

| Parameter or body field | Enforced | Rule ids |
| --- | --- | --- |
| `account_id` | Required numeric Account ID in the path | `vendor.taboola-s2s-bulk.param.account_id.missing`, `.invalid` |
| `actions` | Required array | `vendor.taboola-s2s-bulk.body.actions.missing` |
| `click-id` | Required on every record | `vendor.taboola-s2s-bulk.body.click-id.missing`, `.empty` |
| `timestamp` | Required milliseconds since epoch | `vendor.taboola-s2s-bulk.body.timestamp.missing`, `.invalid` |
| `name` | Required Realize Event Name | `vendor.taboola-s2s-bulk.body.name.missing`, `.empty` |
| `revenue` | Number when present | `vendor.taboola-s2s-bulk.body.revenue.invalid` |
| `currency` | Documented three-letter code when present | `vendor.taboola-s2s-bulk.body.currency.invalid` |
| `quantity` | Integer when present | `vendor.taboola-s2s-bulk.body.quantity.invalid` |

Source: [bulk submit S2S conversions](https://developers.taboola.com/pixel/docs/bulk-submit-s2s-conversions).

## `vendor/taboola-unip`

Taboola browser event pixels on `/log/3/unip`. Level: `official_vendor`. The
`tfa.js` loader stays `vendor/taboola`. S2S postbacks stay `vendor/taboola-s2s`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `en` | Required event name | `vendor.taboola-unip.param.en.missing`, `.empty` |

Source: [Chrome DevTools verification](https://developers.taboola.com/pixel/docs/verification-network-traffic).

## `vendor/hotjar`

Hotjar site loader on `static.hotjar.com/c/hotjar-{hjid}.js`. Level:
`official_template`. Session traffic on `in.hotjar.com` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `hjid` | Required numeric site ID in the path | `vendor.hotjar.param.hjid.missing` |
| `sv` | Recommended snippet version | `vendor.hotjar.param.sv.missing`, `.invalid` |

Source: [What is the Hotjar Tracking Code](https://help.hotjar.com/hc/en-us/articles/115011639927-What-is-the-Hotjar-Tracking-Code).

## `vendor/hubspot`

HubSpot embed loader on `js.hs-scripts.com/{hubId}.js` and
`js.hs-analytics.net/{hubId}.js`. Level: `official_template`. Collect fires
on `track.hubspot.com/__ptq.gif` are `vendor/hubspot-pixel`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `hub_id` | Required numeric Hub ID in the path | `vendor.hubspot.param.hub_id.missing` |

Source: [tracking code API](https://developers.hubspot.com/docs/api-reference/latest/account/settings/tracking-code/overview).

## `vendor/awin`

Awin fall-back conversion image on `www.awin1.com/sread.img` and S2S on
`/sread.php`. Level: `official_vendor`. The MasterTag on `www.dwin1.com` is
`vendor/awin-mastertag`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `merchant` | Required numeric advertiser ID | `vendor.awin.param.merchant.missing`, `.empty`, `.invalid` |
| `tt` | Required `ns` or `ss` | `vendor.awin.param.tt.missing`, `.invalid` |
| `tv` | Required `2` | `vendor.awin.param.tv.missing`, `.invalid` |
| `amount` | Required sale subtotal | `vendor.awin.param.amount.missing`, `.empty` |
| `ch` | Required last-click channel | `vendor.awin.param.ch.missing`, `.empty` |
| `parts` | Required commission group plus amount | `vendor.awin.param.parts.missing`, `.empty` |
| `ref` | Required unique order reference | `vendor.awin.param.ref.missing`, `.empty` |
| `cr` | Recommended ISO 4217 currency | `vendor.awin.param.cr.missing`, `.invalid` |

Source: [fall-back conversion pixel](https://help.awin.com/developers/docs/fall-back-conversion-pixel).

## `vendor/awin-mastertag`

Awin Advertiser MasterTag on `www.dwin1.com/{advertiserId}.js`. Level:
`official_vendor`. Conversion pixels stay `vendor/awin`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `advertiser_id` | Required numeric advertiser ID in the path | `vendor.awin-mastertag.param.advertiser_id.missing`, `.empty`, `.invalid` |

Source: [advertiser MasterTag](https://help.awin.com/developers/docs/advertiser-mastertag).

## `vendor/partnerize`

Partnerize S2S and clickref conversion URLs on `prf.hn/conversion`. Level:
`official_vendor`. Parameters ride as colon-delimited path segments. The
loader on `cdn.performancehorizon.com` is not contracted. Basket item
containers (`[category:.../sku:...]`) are not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `campaign` | Required campaign id in the path | `vendor.partnerize.param.campaign.missing`, `.empty` |
| `clickref` | Required click reference | `vendor.partnerize.param.clickref.missing`, `.empty` |
| `currency` | Required ISO 4217 three-letter code | `vendor.partnerize.param.currency.missing`, `.invalid` |

Source: [S2S integration](https://help.phgsupport.com/hc/en-us/articles/360020395238-Tracking-Partnerize-Server-to-Server-S2S-Integration). Clickref pixel: [clickref pixel](https://help.phgsupport.com/hc/en-us/articles/4834811308957-Tracking-Partnerize-Clickref-Pixel-Integration).

## `vendor/x`

X website tag image pixels on `analytics.twitter.com/i/adsct`. Level:
`ecosystem_reference`. X documents `twq` and `uwt.js`, not this query. The
conversion API is `vendor/x-conversions-api`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `txn_id` | Required conversion ID | `vendor.x.param.txn_id.missing`, `.empty` |
| `p_id` | Recommended; generated pixels send `Twitter` | `vendor.x.param.p_id.missing` |

Source: [conversion tracking for websites](https://business.twitter.com/en/help/campaign-measurement-and-analytics/conversion-tracking-for-websites.html).

## `vendor/amazon-ads`

Amazon Ad Tag conversion loader on
`s.amazon-adsystem.com/iu3/conversion/{id}.js`. Level: `ecosystem_reference`.
Amazon documents Tag IDs in the Advertising Tag GTM template. Generated
loaders put that ID in the path. APS `apstag.js` is not contracted. Firefly
`/dv/` hops are `vendor/amazon-vfw`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `advertiser_id` | Required Tag ID in the path | `vendor.amazon-ads.param.advertiser_id.missing` |

Source: [Amazon Advertising Tag GTM template](https://github.com/amzn/ads-pao-amznjs-gtm-template).

## `vendor/amazon-vfw`

Amazon DSP Firefly viewability hops on `vfw.amazon-adsystem.com` `/dv/`, which
wrap DoubleVerify measurement. Level: `ecosystem_reference`. Amazon documents
DSP third-party verification with DoubleVerify and IAS, not this query.
Generated `/dv/event.png` and `/dv/proxy` tags always send `vstevt`. IAS
`/ias/` hops on the same host stay directory-only. The Ad Tag loader is
`vendor/amazon-ads`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `vstevt` | Required, non-empty Firefly event code | `vendor.amazon-vfw.param.vstevt.missing`, `.empty` |

Source: [approved third-party providers](https://advertising.amazon.com/resources/ad-policy/approved-3p-ad-servers), observed Firefly VAST.

## `vendor/outbrain`

Outbrain conversion pixels on `tr.outbrain.com/pixel`. Level:
`ecosystem_reference`. Outbrain documents the Marketer ID in GTM, not this
query. The JS loader on `amplify.outbrain.com` is not contracted.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| Identifier | `ob_adv_id` or `ob_click_id` | `vendor.outbrain.identifier_required` |

Source: [install Outbrain pixel on GTM](https://www.outbrain.com/help/advertisers/outbrain-pixel-gtm/).

## `vendor/baidu`

Baidu Tongji collect hits on `hm.baidu.com/hm.gif`. Level:
`ecosystem_reference`. Baidu documents the site ID on `hm.js?{siteId}` as a
nameless query string. Collect hits send it as `si`. The loader is not
contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `si` | Required 32-hex site ID | `vendor.baidu.param.si.missing`, `.empty`, `.invalid` |

Source: [Baidu Tongji code introduction](https://tongji.baidu.com/web/help/article?id=174).

## `vendor/kwai`

Kwai Pixel loader on `s1.kwai.net` paths that contain `/pixel/`. Level:
`ecosystem_reference`. Kwai documents `kwaiq.load(pixelId)`, not this query.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `sdkid` | Required Pixel ID | `vendor.kwai.param.sdkid.missing`, `.empty` |
| `lib` | Recommended; generated loaders send `kwaiq` | `vendor.kwai.param.lib.missing`, `.invalid` |

Source: [install developer-mode Pixel](https://docs.qingque.cn/d/home/eZQCNZ1wBFnEpQEAMmOhfoVwI?identityId=1pTerwwOjbg).

## `vendor/hubspot-pixel`

HubSpot collect pixel on `track.hubspot.com/__ptq.gif`. Level:
`ecosystem_reference`. HubSpot tells you to look for `__ptq.gif` and documents
`_hsq`, not this query. The embed loader is `vendor/hubspot`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `a` | Required numeric Hub ID | `vendor.hubspot-pixel.param.a.missing`, `.empty`, `.invalid` |

Source: [troubleshoot the HubSpot tracking code](https://knowledge.hubspot.com/reports/how-do-i-know-if-my-hubspot-tracking-code-is-working).

## `vendor/cj`

CJ Affiliate conversion image and S2S on `www.emjcd.com/u`. Level:
`official_vendor`. Click-redirect hosts are not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `CID` | Required numeric Enterprise ID | `vendor.cj.param.CID.missing`, `.empty`, `.invalid` |
| `TYPE` | Recommended Action ID | `vendor.cj.param.TYPE.missing`, `.invalid` |
| `OID` | Recommended order reference | `vendor.cj.param.OID.missing` |

Source: [MMP Adjust plugin](https://developers.cj.com/docs/plugins/mmp---adjust).

## `vendor/impact`

impact.com Universal Tracking Tag on `utt.impactcdn.com/{UUID}.js`. Level:
`official_template`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `account_id` | Required account UUID in the path | `vendor.impact.param.account_id.missing` |

Source: [UTT installation](https://integrations.impact.com/integration-guides/for-brands/tracking-integrations/javascript-tag-utt/installation).

Conversion POSTs to `api.impact.com` are `vendor/impact-conversions`.

## `vendor/impact-conversions`

Server-to-server conversion submissions to
`api.impact.com/Advertisers/{AccountSID}/Conversions`. Level: `official_vendor`.
The UTT loader stays `vendor/impact`.

The official call is form-urlencoded POST. The pack contracts the path and the
query names impact.com documents.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `account_sid` | Required Account SID in the path | `vendor.impact-conversions.param.account_sid.missing`, `.empty` |
| `CampaignId` | Required numeric program id | `vendor.impact-conversions.param.CampaignId.missing`, `.invalid` |
| Event type | One of `ActionTrackerId`, `EventTypeId`, or `EventTypeCode` | `vendor.impact-conversions.event_type_required` |
| `EventDate` | Recommended ISO 8601 | `vendor.impact-conversions.param.EventDate.missing`, `.invalid` |
| `OrderId` | Recommended order id | `vendor.impact-conversions.param.OrderId.missing` |
| `ClickId` | Recommended `im_ref` click id | `vendor.impact-conversions.param.ClickId.missing` |
| `CurrencyCode` | ISO 4217 three-letter code when present | `vendor.impact-conversions.param.CurrencyCode.invalid` |

Sources: [API online sale](https://integrations.impact.com/integration-guides/for-brands/tracking-integrations/api-online-sale/implementation),
[conversion submission fields](https://integrations.impact.com/integration-guides/for-brands/action-and-conversion-field-references/conversion-submission-field-references).

## `vendor/rakuten`

Rakuten Advertising conversion image on `track.linksynergy.com/ep`. Level:
`ecosystem_reference`. Rakuten documents tracking methods rather than this
query. Click redirects on `click.linksynergy.com` are not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `mid` | Required numeric merchant ID | `vendor.rakuten.param.mid.missing`, `.empty`, `.invalid` |
| `ord` | Required order ID | `vendor.rakuten.param.ord.missing`, `.empty` |
| `skulist` | Recommended item SKUs | `vendor.rakuten.param.skulist.missing` |

Source: [tracking methods](https://pubhelp.rakutenadvertising.com/hc/en-us/articles/4403182382861-Tracking-Methods-and-Transaction-Reports).

## `vendor/brevo-js`

Legacy Brevo JavaScript tracker on `sibautomation.com/sa.js`. Level:
`official_template`. The V2 loader on `cdn.brevo.com/js/sdk-loader.js` has no
key on the URL and is not contracted. REST events are `vendor/brevo`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `key` | Required client key | `vendor.brevo-js.param.key.missing`, `.empty` |

Source: [JS implementation](https://tracker-doc.brevo.com/docs/installation).

## `vendor/adform`

Adform video, impression, and click tags on `*.adform.net` paths `/videoad`,
`/C/`, and `/adfserve`. Level: `official_vendor`. That includes the regional
tracking domains (`a2` Americas, `track` EMEA, `a1` global, `asia` APAC).
Verification scripts on `s2.adform.net` stay directory-only; they do not use
those paths.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `bn` | Required banner ID, leading digits | `vendor.adform.param.bn.missing`, `.empty`, `.invalid` |

Source: [serve third-party banners](https://www.adformhelp.com/hc/en-us/articles/9738565242385-Serve-Third-Party-Banners-with-Adform-Ad-Server). Regional hosts: [site-tracking privacy information](https://www.adformhelp.com/hc/en-us/articles/9740578281873-Learn-About-Site-Tracking-Privacy-Information).

## `vendor/comscore`

Comscore Direct collect beacons on `b.scorecardresearch.com/b` and `/p`. Level:
`official_vendor`. The `beacon.js` loader is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `c1` | Required `2` | `vendor.comscore.param.c1.missing`, `.empty`, `.invalid` |
| `c2` | Required client ID, at least seven digits | `vendor.comscore.param.c2.missing`, `.empty`, `.invalid` |
| `c7` | Required absolute page URL | `vendor.comscore.param.c7.missing`, `.empty`, `.invalid` |

Source: [validate the tag](https://direct-support.comscore.com/hc/en-us/articles/360002578333-How-can-I-validate-my-tag-is-working-as-intended).

## `vendor/quantcast`

Quantcast Measure pixels on `pixel.quantserve.com/pixel`. Parameters ride on
the path as semicolon-delimited pairs. Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `a` | Required p-code starting with `p-` | `vendor.quantcast.param.a.missing`, `.empty`, `.invalid` |

Source: [inspect your tag](https://help.quantcast.com/docs/inspect-your-tag).

## `vendor/plausible`

Plausible Events API JSON posted to `plausible.io/api/event`. Level:
`official_vendor`. Self-hosted endpoints stay directory-only. Short keys `n`,
`u`, and `d` satisfy the same contracts as `name`, `url`, and `domain`.

| Body field | Enforced | Rule ids |
| --- | --- | --- |
| `name` | Required event name (`n` accepted) | `vendor.plausible.event_name_required`, `.body.name.empty` |
| `url` | Required absolute page URL (`u` accepted) | `vendor.plausible.page_url_required`, `.body.url.empty`, `.invalid` |
| `domain` | Required site domain (`d` accepted) | `vendor.plausible.domain_required`, `.body.domain.empty` |

Source: [Events API](https://plausible.io/docs/events-api).

## `vendor/cloudflare`

Cloudflare Web Analytics beacon on `static.cloudflareinsights.com/beacon.min.js`.
Level: `official_vendor`. POST `/cdn-cgi/rum` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `token` | Recommended site token on the query | `vendor.cloudflare.param.token.missing`, `.empty` |
| `spa` | `true` or `false` when present | `vendor.cloudflare.param.spa.invalid` |

Cloudflare documents the GTM install as `beacon.min.js?token=`. Automatic
injection puts the token in `data-cf-beacon`, so a bare script URL is a
warning, not an error.

Source: [Web Analytics FAQ](https://developers.cloudflare.com/web-analytics/faq/).

## `vendor/matomo`

Matomo Tracking API hits to `matomo.php` on Matomo Cloud. Level:
`official_vendor`. Self-hosted `matomo.php` on other hosts stays directory-only.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `idsite` | Required numeric site ID | `vendor.matomo.param.idsite.missing`, `.empty`, `.invalid` |
| `rec` | Required `1` | `vendor.matomo.param.rec.missing`, `.empty`, `.invalid` |
| `action_name` | Recommended | `vendor.matomo.param.action_name.missing` |
| `url` | Recommended, absolute URL | `vendor.matomo.param.url.missing`, `.invalid` |
| `_id` | Recommended, 16 hex characters | `vendor.matomo.param._id.missing`, `.invalid` |
| Event | `e_c` requires `e_a` | `vendor.matomo.event_requires_action` |
| Order | `ec_id` requires `revenue` | `vendor.matomo.order_requires_revenue` |

Source: [Tracking HTTP API](https://developer.matomo.org/api-reference/tracking-api).

## `vendor/parsely`

Parse.ly tracker loader on `cdn.parsely.com/keys/{site_id}/p.js`. Level:
`official_vendor`. Collect on `p1.parsely.com` is `vendor/parsely-collect`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `site_id` | Required Site ID in the path | `vendor.parsely.param.site_id.missing` |

Source: [tracking code setup](https://docs.parse.ly/installation-resources/parsely-integration/tracking-code-setup/).

## `vendor/parsely-collect`

Parse.ly collect beacons on `p1.parsely.com` and `p1-irl.parsely.com`. Level:
`official_vendor`. The tracker loader stays `vendor/parsely`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `idsite` | Required Site ID | `vendor.parsely-collect.param.idsite.missing`, `.empty` |
| `url` | Recommended absolute URL | `vendor.parsely-collect.param.url.missing`, `.invalid` |
| `action` | Recommended | `vendor.parsely-collect.param.action.missing`, `.empty` |

Sources: [tracker details](https://docs.parse.ly/tracker-details/),
[test integration](https://docs.parse.ly/installation-resources/parsely-integration/test-integration/),
[content security policy](https://docs.parse.ly/privacy/content-security-policy/).

## `vendor/crazyegg`

Crazy Egg account script on
`script.crazyegg.com/pages/scripts/{account_id}/{script_id}.js`. Level:
`official_template`. Session traffic on `tracking.crazyegg.com` is not
contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `account_id` | Required numeric account folder | `vendor.crazyegg.param.account_id.missing` |
| `script_id` | Required numeric script file | `vendor.crazyegg.param.script_id.missing` |

Source: [check that Crazy Egg is installed](https://support.crazyegg.com/knowledge-base/how-to-check-that-crazy-egg-is-installed/).

## `vendor/ispot`

iSpot Unified Measurement impression GIFs on `pi.ispot.tv/v2/{tracking_code}.gif`.
Level: `official_vendor`. TV conversion pixels on `pt.ispot.tv` are
`vendor/ispot-conversion`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `tracking_code` | Required in the path as `TC-{digits}-{digits}` | `vendor.ispot.param.tracking_code.missing`, `.empty`, `.invalid` |

Source: [OTT Unified Measurement](https://developer.ispot.tv/documentation/unified-measurement) and the [pixel technical spec](https://developer.ispot.tv/sites/default/files/iSpot_Pixel_Technical_Documentation_1.pdf). Query extras such as `campaignid` are client-optional and not contracted.

## `vendor/ispot-conversion`

iSpot TV conversion GIFs on `pt.ispot.tv/v2/{tracking_code}.gif`. Level:
`official_vendor`. Impression GIFs stay `vendor/ispot`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `tracking_code` | Required in the path as `TC-{digits}-{digits}` | `vendor.ispot-conversion.param.tracking_code.missing`, `.empty`, `.invalid` |
| `type` | Recommended conversion event name | `vendor.ispot-conversion.param.type.missing`, `.empty` |

Source: [conversion type](https://developer.ispot.tv/documentation/api/product-use-cases/tv_conversions/conversion_type) and the [pixel technical spec](https://developer.ispot.tv/sites/default/files/iSpot_Pixel_Technical_Documentation_1.pdf).

## `vendor/chartbeat`

Chartbeat engagement pings on `ping.chartbeat.net/ping`. Level:
`official_vendor`. The `chartbeat.js` loader on `static.chartbeat.com` is
not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `h` | Required site id (dashboard host) | `vendor.chartbeat.param.h.missing`, `.empty` |
| `g` | Required numeric account UID | `vendor.chartbeat.param.g.missing`, `.empty`, `.invalid` |
| `p`, `d`, `t`, `g0`, `g1`, `i` | When present, not empty | `vendor.chartbeat.param.p.empty` and the same shape for the rest |

Those QA keys are format-when-present, not recommended, so a ping that only
sends `h` and `g` stays warning-free.

Source: [QA a web integration](https://docs.chartbeat.com/cbp/tracking/standard-websites/qa-web-integration).

## `vendor/heap`

Heap.js 5 configuration loader on `cdn.us.heap-api.com` and
`cdn.eu.heap-api.com`, path `/config/{envId}/heap_config.js`. Level:
`official_vendor`. Classic `heap-{id}.js` on `cdn.heapanalytics.com` is
`vendor/heap-classic`. Server-side `/api/track` is `vendor/heap-track`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `env_id` | Required numeric environment ID in the path | `vendor.heap.param.env_id.missing`, `.empty`, `.invalid` |

Source: [web installation](https://developers.heap.io/docs/web).

## `vendor/heap-classic`

Heap Classic loader on `cdn.heapanalytics.com/js/heap-{appId}.js`. Level:
`official_vendor`. Heap.js 5 stays `vendor/heap`. Server-side `/api/track` is
`vendor/heap-track`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `app_id` | Required numeric environment ID in the path | `vendor.heap-classic.param.app_id.missing`, `.empty`, `.invalid` |

Source: [install Heap.js](https://developers.heap.io/docs/install-heapjs).

## `vendor/heap-track`

Server-side custom events posted to `heapanalytics.com/api/track`. Level:
`official_vendor`. The Heap.js 5 config loader stays `vendor/heap`. Classic
`heap-{id}.js` stays `vendor/heap-classic`. Identify is `vendor/heap-identify`.
Add user properties is `vendor/heap-user-properties`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `app_id` | Required environment ID | `vendor.heap-track.body.app_id.missing`, `.empty` |
| `event` | Required event name | `vendor.heap-track.body.event.missing`, `.empty` |
| Identity | One of `identity` or `user_id` | `vendor.heap-track.body.identity_or_user_required` |
| Exclusive identity | Not both `identity` and `user_id` | `vendor.heap-track.body.identity_and_user_exclusive` |
| `timestamp` | ISO 8601 when present | `vendor.heap-track.body.timestamp.invalid` |

Source: [track](https://developers.heap.io/reference/track-1).

## `vendor/heap-identify`

Server-side identify posted to `heapanalytics.com/api/v1/identify`. Level:
`official_vendor`. Track stays `vendor/heap-track`. Add user properties is
`vendor/heap-user-properties`.

| Body field | Enforced | Rule ids |
| --- | --- | --- |
| `app_id` | Required environment ID | `vendor.heap-identify.body.app_id.missing`, `.empty` |
| `user_id` | Required numeric SDK id | `vendor.heap-identify.body.user_id.missing`, `.invalid` |
| `identity` | Required known identity | `vendor.heap-identify.body.identity.missing`, `.empty` |
| `timestamp` | ISO 8601 when present | `vendor.heap-identify.body.timestamp.invalid` |

Source: [identify](https://developers.heap.io/reference/identify-1).

## `vendor/heap-user-properties`

Server-side add user properties posted to
`heapanalytics.com/api/add_user_properties`. Level: `official_vendor`. Bulk
`users[]` is not contracted. Identify stays `vendor/heap-identify`.

| Body field | Enforced | Rule ids |
| --- | --- | --- |
| `app_id` | Required environment ID | `vendor.heap-user-properties.body.app_id.missing`, `.empty` |
| `identity` | Required known identity | `vendor.heap-user-properties.body.identity.missing`, `.empty` |
| `properties` | Recommended trait object | `vendor.heap-user-properties.body.properties.missing` |

Source: [add user properties](https://developers.heap.io/reference/add-user-properties).

## `vendor/mouseflow`

Mouseflow project script on `cdn.mouseflow.com/projects/{website_id}.js`.
Level: `official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `website_id` | Required in the path | `vendor.mouseflow.param.website_id.missing`, `.empty` |

Source: [custom variables](https://help.mouseflow.com/en/articles/4312070-custom-variables).

## `vendor/intercom`

Intercom Messenger loader on `widget.intercom.io/widget/{app_id}`. Level:
`official_vendor`. Data events on `api.intercom.io/events` are
`vendor/intercom-events`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `app_id` | Required workspace ID in the path | `vendor.intercom.param.app_id.missing`, `.empty` |

Source: [web installation](https://developers.intercom.com/installing-intercom/web/installation).

## `vendor/intercom-events`

Data events posted to `api.intercom.io/events`. Level: `official_vendor`. The
Messenger loader stays `vendor/intercom`.

| Body field or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_name` | Required | `vendor.intercom-events.body.event_name.missing`, `.empty` |
| `created_at` | Unix seconds, 10 digits | `vendor.intercom-events.body.created_at.missing`, `.invalid` |
| Contact | One of `user_id`, `email`, `id`, or `intercom_user_id` | `vendor.intercom-events.body.contact_identifier_required` |

Source: [create data event](https://developers.intercom.com/docs/references/rest-api/api.intercom.io/data-events/createdataevent).

## `vendor/nextdoor-conversions-api`

Server-side conversion events posted to
`ads.nextdoor.com/v2/api/conversions/track`. Level: `official_vendor`.
`event_time` and `pixel_id` are deprecated; the pack contracts
`event_time_epoch` and `data_source_id`.

| Parameter or rule | Enforced | Rule ids |
| --- | --- | --- |
| `event_name` | Required documented event | `vendor.nextdoor-conversions-api.body.event_name.missing`, `.invalid` |
| `action_source` | Required channel | `vendor.nextdoor-conversions-api.body.action_source.missing`, `.invalid` |
| `event_id` | Required dedup id | `vendor.nextdoor-conversions-api.body.event_id.missing` |
| `event_time_epoch` | Required Unix seconds, 10 digits | `vendor.nextdoor-conversions-api.body.event_time_epoch.missing`, `.invalid` |
| `data_source_id` | Required Pixel ID | `vendor.nextdoor-conversions-api.body.data_source_id.missing` |
| `customer` | Required | `vendor.nextdoor-conversions-api.body.customer.missing` |
| User identifiers | At least one of hashed `email`, hashed `phone_number`, or `click_id` | `vendor.nextdoor-conversions-api.body.user_needs_an_identifier` |
| `action_source_url` | Required on `website` | `vendor.nextdoor-conversions-api.body.website_requires_url` |
| `custom.order_value` | Required on `purchase` | `vendor.nextdoor-conversions-api.body.purchase_requires_order_value` |
| `customer.email` | SHA-256 hex when present; raw email is an error | `vendor.nextdoor-conversions-api.body.customer.email.invalid`, `.unhashed_email` |

Source: [conversions/track](https://developer.nextdoor.com/reference/conversions-track),
[data types](https://developer.nextdoor.com/reference/conversion-data-types).

## `vendor/liveramp-envelope`

LiveRamp ATS Envelope API on `api.rlcdn.com`. Level: `official_vendor`.
Cookie sync on `idsync.rlcdn.com` stays directory-only. Envelope refresh is
`vendor/liveramp-envelope-refresh`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `pid` | Required integer Placement ID | `vendor.liveramp-envelope.param.pid.missing`, `.invalid` |
| `it` | Required: `4` hashed email, `11` hashed phone, `15` custom ID | `vendor.liveramp-envelope.param.it.missing`, `.invalid` |
| `iv` | Required hashed identifier | `vendor.liveramp-envelope.param.iv.missing`, `.empty` |
| `ct` | `3` CCPA or `4` TCF v2 when present | `vendor.liveramp-envelope.param.ct.invalid` |
| `atype` | `1`–`4` when present | `vendor.liveramp-envelope.param.atype.invalid` |

Source: [ATS Envelope API](https://developers.liveramp.com/authenticatedtraffic-api/docs/4-call-the-ats-envelope-api).

## `vendor/liveramp-envelope-refresh`

LiveRamp ATS Envelope refresh on `api.rlcdn.com/api/identity/v2/envelope/refresh`.
Level: `official_vendor`. Retrieve stays `vendor/liveramp-envelope`. `it` is an
envelope type, not an identifier type.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `pid` | Required integer Placement ID | `vendor.liveramp-envelope-refresh.param.pid.missing`, `.invalid` |
| `it` | Required: `19` ATS or `24` Meta-scoped | `vendor.liveramp-envelope-refresh.param.it.missing`, `.invalid` |
| `iv` | Required envelope value | `vendor.liveramp-envelope-refresh.param.iv.missing`, `.empty` |
| `ct` | `3` CCPA or `4` TCF v2 when present | `vendor.liveramp-envelope-refresh.param.ct.invalid` |
| `atype` | `1`–`4` when present | `vendor.liveramp-envelope-refresh.param.atype.invalid` |

Source: [Refresh Envelope API](https://developers.liveramp.com/authenticatedtraffic-api/docs/7-implement-the-ats-refresh-envelope-api).

## `vendor/cookiebot`

Cookiebot CMP banner on `consent.cookiebot.com/uc.js`. Level:
`official_vendor`. Cookiebot documents `cbid` on the query, or in
`data-cbid` on the script tag. The Cookie Declaration is
`vendor/cookiebot-declaration`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `cbid` | Recommended UUID domain group ID | `vendor.cookiebot.param.cbid.missing`, `.empty`, `.invalid` |

Source: [developer resources](https://www.cookiebot.com/en/developer/).

## `vendor/cookiebot-declaration`

Cookiebot Cookie Declaration on `consent.cookiebot.com/{cbid}/cd.js`.
Level: `official_template`. The banner stays `vendor/cookiebot`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `cbid` | Required UUID in the path | `vendor.cookiebot-declaration.param.cbid.missing`, `.invalid` |

Source: [manual implementation](https://support.cookiebot.com/hc/en-us/articles/10714664673564-Manually-implementing-Cookiebot-CMP-Cookiebot-Admin).

## `vendor/onetrust`

OneTrust Auto-Blocking on
`cdn.cookielaw.org/consent/{domain-script-id}/OtAutoBlock.js`. Level:
`official_vendor`. `otSDKStub.js` with `data-domain-script` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `domain_script_id` | Required in the path | `vendor.onetrust.param.domain_script_id.missing`, `.empty` |

Source: [ecommerce AutoBlocking](https://developer.onetrust.com/onetrust/docs/ecommerce).

## `vendor/zendesk`

Zendesk Web Widget snippet on `static.zdassets.com/ekr/snippet.js`. Level:
`official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `key` | Required widget key | `vendor.zendesk.param.key.missing`, `.empty` |

Source: [Web Widget JavaScript APIs](https://developer.zendesk.com/documentation/classic-web-widget-sdks/web-widget/quickstart-tutorials/web-widget-javascript-apis/).

## `vendor/drift`

Drift widget loader on `js.driftt.com/include/{cacheWindow}/{embedId}.js`.
Level: `official_vendor`. Collect on `event.api.drift.com` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `embed_id` | Required embed ID in the path | `vendor.drift.param.embed_id.missing`, `.empty` |
| `cache_bust` | Required numeric cache window | `vendor.drift.param.cache_bust.missing`, `.invalid` |

Source: [installation](https://devdocs.drift.com/docs/installation).

## `vendor/mailchimp`

Mailchimp connected site script on
`chimpstatic.com/mcjs-connected/js/users/{user}/{site}.js`. Level:
`official_vendor`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `user_id` | Required user hash in the path | `vendor.mailchimp.param.user_id.missing`, `.empty` |
| `site_id` | Required connected-site hash | `vendor.mailchimp.param.site_id.missing`, `.empty` |

Source: [add connected site](https://mailchimp.com/developer/marketing/api/connected-sites/add-connected-site/).

## `vendor/pardot`

Salesforce Account Engagement Tracking and Consent loader on
`pi.pardot.com/pdt.js` and `pi.demand.salesforce.com/pdt.js`. Level:
`official_vendor`. Legacy `pd.js` puts `piAId` in JavaScript and is not
contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `aid` | Required integer account ID | `vendor.pardot.param.aid.missing`, `.empty`, `.invalid` |

Source: [Tracking and Consent JavaScript API](https://developer.salesforce.com/blogs/2022/02/how-to-work-with-pardots-new-tracking-consent-javascript-api).

## `vendor/nielsen`

Nielsen DCR SDK hello ping on `secure-dcr.imrworldwide.com/cgi-bin/cfg`.
Level: `official_vendor`. Measurement pings on `/cgi-bin/gn` are not
contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `apid` | Required Nielsen App ID | `vendor.nielsen.param.apid.missing`, `.empty` |
| `apn` | Recommended player or site name | `vendor.nielsen.param.apn.missing`, `.empty` |
| `sfcode` | Recommended `dcr` or `dcr-cert` | `vendor.nielsen.param.sfcode.missing`, `.invalid` |

Source: [DCR Static Browser SDK](https://engineeringportal.nielsen.com/wiki/DCR_Static_Browser_SDK_(5.1.1)).

## `vendor/didomi`

Didomi Web SDK loader on `sdk.privacy-center.org/{Public API Key}/loader.js`.
Level: `official_vendor`. Core and UI files under `/sdk/` are not contracted.
`api.privacy-center.org` events are not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `api_key` | Required Public API Key in the path | `vendor.didomi.param.api_key.missing`, `.empty` |

Source: [reverse proxy](https://developers.didomi.io/api-and-platform/domains/reverse-proxy).

## `vendor/lotame`

Lotame Lightning Tag on `tags.crwdcntrl.net/lt/c/{clientId}/lt.min.js`. Level:
`official_vendor`. Collect on `bcp.crwdcntrl.net` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `client_id` | Required integer client ID in the path | `vendor.lotame.param.client_id.missing`, `.empty`, `.invalid` |

Source: [LT.js basic implementation](https://my.lotame.com/t/83hxvnt/lt-js-basic-implementation).

## `vendor/ias`

IAS Signal display tag on
`pixel.adsafeprotected.com/rjss/st/{advertiserId}/{publisherId}/skeleton.js`.
Level: `ecosystem_reference`. Video tags are `vendor/ias-video`. Other IAS
paths are not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `advertiser_id` | Required integer advertiser ID in the path | `vendor.ias.param.advertiser_id.missing`, `.empty`, `.invalid` |
| `publisher_id` | Required integer publisher ID in the path | `vendor.ias.param.publisher_id.missing`, `.empty`, `.invalid` |

Source: [Connecting to third-party measurement partners](https://learn.microsoft.com/en-us/advertising/msa-help/hlp_ba_conc_connectingthirdparty).

## `vendor/ias-video`

IAS Signal video tag on
`unified.adsafeprotected.com/v2/{advertiserId}/{publisherId}`. Level:
`ecosystem_reference`. Display tags stay `vendor/ias`.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `advertiser_id` | Required integer advertiser ID in the path | `vendor.ias-video.param.advertiser_id.missing`, `.empty`, `.invalid` |
| `publisher_id` | Required integer publisher ID in the path | `vendor.ias-video.param.publisher_id.missing`, `.empty`, `.invalid` |

Source: [Connecting to third-party measurement partners](https://learn.microsoft.com/en-us/advertising/msa-help/hlp_ba_conc_connectingthirdparty).

## `vendor/mediamath`

MediaMath MathTag pixels on `pixel.mathtag.com/event/js` and `/event/img`.
Level: `official_vendor`. Cookie sync on `sync.mathtag.com` is not contracted.
Mobile `/event/mob` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `mt_id` | Required integer Pixel ID | `vendor.mediamath.param.mt_id.missing`, `.empty`, `.invalid` |
| `mt_adid` | Required integer Advertiser ID | `vendor.mediamath.param.mt_adid.missing`, `.empty`, `.invalid` |
| `mt_exem` | Optional SHA-256 hashed email | `vendor.mediamath.param.mt_exem.empty`, `.invalid` |
| `mt_excl` | Optional SHA-256 hashed account ID | `vendor.mediamath.param.mt_excl.empty`, `.invalid` |
| unhashed email | Forbidden | `vendor.mediamath.unhashed_email` |

Source: [Mobile Pixel SDK](https://apidocs.mediamath.com/guides/mobile-pixel-sdk).

## `vendor/oracle-bluekai`

Oracle BlueKai site tags on `tags.bluekai.com/site/{siteId}` and
`stags.bluekai.com/site/{siteId}`. Level: `official_vendor`. CoreTag on
`tags.bkrtx.com` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `site_id` | Required integer site ID in the path | `vendor.oracle-bluekai.param.site_id.missing`, `.empty`, `.invalid` |
| `e_id_s` | Optional SHA-256 email oHash | `vendor.oracle-bluekai.param.e_id_s.empty`, `.invalid` |
| unhashed email | Forbidden | `vendor.oracle-bluekai.unhashed_email` |

Source: [Sending oHashes to the Oracle Data Cloud Platform](https://docs.oracle.com/en/cloud/saas/data-cloud/data-cloud-help-center/IntegratingBlueKaiPlatform/IDManagement/sending_ohashes.html).

## `vendor/google-ad-manager`

Google Ad Manager ad requests on `pubads.g.doubleclick.net/gampad/ads`
and `securepubads.g.doubleclick.net/gampad/ads`. Level: `official_vendor`.
Floodlight activity tags stay `vendor/floodlight`. CM360 VAST events stay
`vendor/cm360-vast-event`. Display GPT and VAST tags share this hop; only
`iu` is contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `iu` | Required ad unit path `/network_code/.../ad_unit` | `vendor.google-ad-manager.param.iu.missing`, `.empty`, `.invalid` |

Source: [VAST ad tag parameters for web](https://support.google.com/admanager/answer/10655276).

## `vendor/trustarc`

TrustArc CCM Pro on `consent.trustarc.com/v2/notice/{cmId}`. Level:
`official_vendor`. CCM Advanced `/notice?domain=` is not contracted.
`consent-st.trustarc.com` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `notice_id` | Required Consent Manager ID in the path | `vendor.trustarc.param.notice_id.missing`, `.empty` |

Source: [Cookie Consent Manager Professional Implementation Guide](https://consent.trustarc.com/asset/TrustArc_Cookie_Consent_Manager_Implementation_Guide.pdf).

## `vendor/id5`

ID5 mobile in-app S2S on `api.id5-sync.com/ga/v1`. Level:
`official_vendor`. Cookie sync on `/i/` and `/s/` is not contracted. CTV
`/gc/v1` is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `partner` | Required integer Partner Number | `vendor.id5.body.partner.missing`, `.empty`, `.invalid` |
| `ts` | Required timestamp | `vendor.id5.body.ts.missing`, `.empty` |
| `bundle` | Required app identifier | `vendor.id5.body.bundle.missing`, `.empty` |
| `ver` | Required app version | `vendor.id5.body.ver.missing`, `.empty` |
| `ip` | Required IPv4 closest to the device | `vendor.id5.body.ip.missing`, `.empty` |
| `ua` | Required user agent | `vendor.id5.body.ua.missing`, `.empty` |
| `hem` | Optional SHA-256 hashed email | `vendor.id5.body.hem.empty`, `.invalid` |
| `phone` | Optional SHA-256 hashed phone | `vendor.id5.body.phone.empty`, `.invalid` |
| unhashed email | Forbidden | `vendor.id5.body.unhashed_email` |

Source: [Mobile In-App Integration](https://wiki.id5.io/docs/mobile-in-app-integration).

## `vendor/yahoo-conversions-api`

Yahoo Standard Conversion API on
`batch.datax.yahoo.com/v1/events/{pixelId}`. Level: `official_vendor`.
Dot image pixels stay `vendor/yahoo-dot`. Product CAPI, which requires
`eventId`, is not contracted.

| Parameter | Enforced | Rule ids |
| --- | --- | --- |
| `pixel_id` | Required integer pixel ID in the path | `vendor.yahoo-conversions-api.param.pixel_id.missing`, `.empty`, `.invalid` |
| `eventTs` | Required integer timestamp | `vendor.yahoo-conversions-api.body.eventTs.missing`, `.empty`, `.invalid` |
| `eventName` | Required event action name | `vendor.yahoo-conversions-api.body.eventName.missing`, `.empty` |
| `actionSource` | Required event source | `vendor.yahoo-conversions-api.body.actionSource.missing`, `.empty` |
| `userData` | Required user-data object | `vendor.yahoo-conversions-api.body.userData.missing`, `.empty` |
| `userData.email` / `email[]` | Optional SHA-256 hashed email | `vendor.yahoo-conversions-api.body.userData.email.invalid`, `vendor.yahoo-conversions-api.body.userData.email[].invalid` |
| match key | At least one of hashed email, phone, gpsaid, idfa, pxid, or clickData | `vendor.yahoo-conversions-api.body.user_needs_an_identifier` |
| unhashed email | Forbidden | `vendor.yahoo-conversions-api.body.unhashed_email` |

Source: [Standard Yahoo Conversion API](https://help.yahooinc.com/dsp-api/docs/standard-yahoo-conversion-api).

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

- Snap Pixel (`sc-static.net/scevent.min.js` and `tr.snapchat.com/p`). The
  loader has no pixel ID on the URL. Collection is a POST without a published
  query. Snap Conversions API is `vendor/snapchat`. Taboola unip events are
  `vendor/taboola-unip`. S2S is `vendor/taboola-s2s`. iSpot conversion GIFs are
  `vendor/ispot-conversion`. LiveRamp Envelope is
  `vendor/liveramp-envelope`. Refresh is `vendor/liveramp-envelope-refresh`.
  Cookie sync on `idsync.rlcdn.com` stays directory-only.
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
