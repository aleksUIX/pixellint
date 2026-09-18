# Changelog

All notable changes to Pixellint are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.31.4 - 2026-09-18

### Changed

- Same-hop vendor packs now contract more of their published HTTP tables.
  Partnerize conversion paths read colon-delimited path segments, including
  basket `[category/sku/value/quantity]` containers, and type-check country,
  customertype, fulfilment dates, device, context, and a raw email in any
  segment. Pinterest image tags type-check hashed `pd[em]` and
  `pd[external_id]`, `ed[event_id]`, and line-item price and quantity. Meta
  Pixel Advanced Matching type-checks hashed `ge`, `db`, `ct`, `st`, `zp`,
  and `country`. Adobe Analytics link hits (`pe=lnk_o`, `lnk_d`, or `lnk_e`)
  require `pev1` and `pev2`.
- `param_style: colon_path` reads `/key:value/` path segments, including
  Partnerize basket containers wrapped in `[...]`.

## 0.31.3 - 2026-09-17

### Changed

- Same-hop vendor packs now contract more of their published HTTP tables.
  Kochava post-install events require `kochava_device_id`, a device IP, a
  device user agent, and a `device_ver` key (empty is allowed), accepting those
  device fields at the root or inside `data`. Currency is ISO 4217 when
  present, and a hashed IP or user agent is an error. GA4 Measurement Protocol
  events recommend `session_id` and `engagement_time_msec`, type-check
  `validation_behavior` and `user_location.country_id`, reject a hashed
  `ip_override`, and forbid the reserved `user_properties.user_id` name.
  Awin conversion pixels type-check `amount` as a dot-decimal float and
  `parts` as `{group}:{amount}`, require `cks` when `tt=ss`, and accept
  `customeracquisition` only as `NEW` or `RETURNING`. Mixpanel `/import`
  type-checks `$insert_id` as at most 36 alphanumeric or hyphen characters,
  rejects Mixpanel's published placeholder identifiers, allows an empty
  `distinct_id`, and rejects a hashed `ip`. Branch Events API type-checks
  `event_data.currency` and `event_data.revenue` when present, rejects a
  hashed user agent, and requires DMA consent fields when `dma_eea` is true.
  Adjust S2S events type-check `ip_address` as IPv4, require `revenue` with
  `currency`, and reject a hashed user agent. impact.com conversions require
  `EventDate` (ISO 8601 or `NOW`) and at least one attribution key, reject a
  plaintext email as `CustomerId`, and reject a hashed `IpAddress`. Google Ads
  click uploads require `conversionValue` with `currencyCode` and reject a
  hashed `userIpAddress`. Microsoft Advertising CAPI type-checks `pageLoadId`
  as a UUID and `adStorageConsent` as `G` or `D`, requires `customData.value`
  with `currency`, and rejects a hashed IP or user agent. Amplitude HTTP V2
  rejects reserved `[Amplitude]` event names (`$identify` is allowed),
  type-checks `revenue`, uppercase ISO 4217 `currency`, and `session_id`
  (including `-1`), and rejects a hashed `ip` or `user_agent` (`$remote` is
  allowed). Mixpanel `/track` type-checks query `ip`, `verbose`, and `img` as
  `0` or `1`, and rejects a hashed `properties.ip`. Nextdoor CAPI type-checks
  `custom.order_value` as ISO 4217 then amount, requires a user agent on
  website events, type-checks `delivery_category`, and rejects a hashed IP or
  user agent. Klaviyo Create Event requires `properties`, type-checks metric
  names under 128 characters, ISO 8601 `time`, E.164 phone, and `value` with
  `value_currency`. Braze `/users/track` accepts only one primary identifier
  per object, type-checks purchase `price` and `quantity` (1 through 100), and
  forbids reserved property names such as `time`. PostHog capture accepts
  `distinct_id` at the top level or in `properties`, caps it at 200 characters,
  type-checks `historical_migration`, requires `alias` on `$create_alias` and
  group fields on `$groupidentify`, and rejects a hashed `$ip`. Google Ads
  call uploads require `conversionValue` with `currencyCode` and type-check
  `consent.adUserData`. Heap server-side track caps `event` at 1024 characters
  and `identity` at 255, type-checks numeric `user_id`, and forbids reserved
  keys inside `properties`. AppsFlyer S2S requires `eventValue` (empty is
  allowed), type-checks `att` as 0 through 3, UUID `advertising_id` and `idfa`,
  boolean `aie`, and `app_type` as `app_clip`. Google Ads conversion
  adjustments require `conversionDateTime` with `gclid`, type-check numeric
  `adjustedValue`, and forbid that value on `RETRACTION`. Segment and
  RudderStack batch `group` needs `groupId`, `alias` needs `previousId`, and a
  hashed `context.ip` is an error. Segment also caps `messageId` under 100
  characters. Mixpanel `/engage` type-checks `$time` as seconds since epoch and
  rejects a hashed `$ip`.
- `forbid_value_pattern` now inspects values that look like `[macros]`, so
  reserved Amplitude event names are not skipped.

## 0.31.2 - 2026-09-17

### Changed

- Same-hop vendor packs now contract more of their published HTTP tables.
  Floodlight sales tags type-check `qty` as an integer of 1 or more and `cost`
  as a number, and require the pair together. Chartbeat recommends `p`, `d`,
  and `t` from the QA ping key table. Parse.ly collect requires `url`. Nielsen
  DCR cfg requires `apn` and `sfcode` from the initialize table. FreeWheel
  checks `caid`, `asid`, and `flag` when present. Adobe Analytics warns when a
  beacon has neither `pageName` nor `g`. Yahoo Dot accepts `projectId` as the
  JavaScript twin of `a`.
- `vendor/microsoft-uet` now follows Microsoft's published UET parameter table:
  required `ti`, `ver`, `evt` (`pageLoad` or `custom`), `mid`, and 6-digit
  `rn`; recommended `p` and `msclkid`; `ec`/`ea`/`el`/`ev` forbidden on
  `pageLoad`. Evidence level is `official_vendor`.
- Host-less JSON matching no longer claims SKAd-shaped `{ts, bundle, ver}` as
  ID5, AppsFlyer `{app_id, eventName}` as Heap identify or user-properties, or
  Meta `{event_name, user_data}` as Intercom. Meta CAPI also claims a flat
  event object, not only `data[]`. Yahoo CAPI requires `userData` on the batch
  array.

## 0.31.1 - 2026-09-15

### Changed

- Same-hop vendor packs now contract more of their published HTTP tables.
  Campaign Manager 360 tracking ads check `dc_lat`, `tfua`, and
  `tag_for_child_directed_treatment` as `0` or `1`, and generated empty slots
  stay quiet. Google Ads conversion pixels format-check `value` as a number
  with no currency symbol or comma. Xandr conversion pixels check `order_id`
  as letters and numbers up to 36 characters, `other` up to 20, and `redir`
  as a URL. Quantcast Measure checks `labels`, `orderid`, `revenue`, and
  `url` when present.

## 0.31.0 - 2026-09-15

### Added

- `vendor/doubleverify-event`, covering DoubleVerify video quartile and
  player-event beacons on `tps.doubleverify.com/event.png` and
  `tpsc-video-*.doubleverify.com/event.png`: required `vstevt`, and `dup`
  when present. Impression `visit.jpg` hops stay `vendor/doubleverify`.
  Amazon-hosted `/dv/` hops stay `vendor/amazon-vfw`.

### Changed

- `vendor/google-ad-manager` now contracts the rest of Google's published
  VAST ad tag table on `/gampad/ads`: required `sz`, `output`, `env`,
  `gdfp_req`, and `correlator`, recommended `description_url`, and the
  documented programmatic and player fields when present. `pagead/` and
  `pcs/view` stay directory-only.
- `vendor/amazon-vfw` checks `ctx`, `cmp`, `plc`, and `sid` when present on
  Firefly `/dv/` hops. Generated `/dv/proxy` tags send those names from the
  same `dvparams` Google documents on DoubleVerify wrappers.
- `vendor/doubleverify` checks `vstevt` when present on `visit.jpg`.

## 0.30.3 - 2026-09-13

### Changed

- Same-hop vendor packs now contract more of their published HTTP tables.
  OpenAI image tag and Conversions API check event-to-data-shape pairing,
  `contents`/`plan_id` availability per shape, `custom_event_name` omitted on
  standard events, custom name charset, money fields, `oppref`, and unhashed
  email on the image query. CAPI also checks `opt_out`, `validate_only`, item
  `contents[]`, geographic match keys, empty `user`, GAID (including all-zero),
  `obref`, IP, hashed plaintext on geo/`user_agent`, and lowercase SHA-256 hex.
  `value_when` rules require a present field to carry a documented value.
  `forbidden_when_value` rules reject a present field when another field carries
  a documented value. Adobe Analytics checks `ch`, `pev1`, and `pev2` when
  present. Adobe Web SDK flags an empty `xdm.identityMap`. Matomo flags an
  empty `e_n`. GA4 Measurement Protocol checks `consent.ad_user_data` and
  `consent.ad_personalization` as `GRANTED` or `DENIED`. Alternative JSON
  envelopes pick the first scope that is present, so Adobe collect `events[]`
  is contracted rather than skipped for a missing interact `event` key.

## 0.30.2 - 2026-09-13

### Changed

- Parameter contracts can set `name_pattern` so a family of present keys shares
  one empty/format check. Floodlight covers `u1`–`u100`. Adobe Analytics covers
  `c1`–`c75`, `v1`–`v250`, `l1`–`l3`, and `xact`. Matomo covers `dimension1`–
  `dimension999`. Pinterest tag checkout and add-to-cart expect `ed[value]` and
  `ed[currency]`. Kochava `action` is `event`. GA4 Measurement Protocol checks
  `user_id` when present and requires ecommerce fields on `view_cart` and
  `add_payment_info`.

## 0.30.1 - 2026-09-13

### Changed

- Same-hop vendor packs now contract more of their published HTTP tables.
  Adobe Analytics checks `cc`, `referrer`, `purchaseID`, `vid`, `v0`/`campaign`,
  `pageType`/`gt`, and `server`/`sv` when present. Floodlight checks `npa` and
  `tfua` as `0` or `1`. Google Ads conversion pixels check `currency_code` and
  `ord`. Matomo recommends `rand` and `apiv=1`, and format-checks `e_v` and
  `cid`. Snap CAPI requires value and currency on `PURCHASE`. Pinterest CAPI
  warns when `checkout` omits value or currency. GA4 Measurement Protocol
  requires ecommerce fields on `view_item` and `add_to_wishlist`. Parameter
  contracts can set `allow_empty` so a blank Floodlight `npa`/`tfua` slot is
  not reported.

## 0.30.0 - 2026-09-12

### Added

- `--directory-file` and `Engine::merge_directory`, so a community or private
  overlay can attribute extra hosts without replacing the built-in directory.
  A host the built-in file already claims is rejected. CONTRIBUTING documents
  runtime overlays and first-party PRs to `build-directory.py`.

## 0.29.0 - 2026-09-12

### Added

- `Engine::validate_many` and `pixellint validate-many`, wrapping extracted
  artifacts into the document result in `docs/MULTI_ARTIFACT_SCHEMA.md`.
  Identical URLs validate once. Occurrences stay attached. Core still does not
  parse VAST, HTML, or GTM. A JSON array of URL strings is a `list`. npm
  `validateMany` wraps the same engine. MCP `validate_document` is still later.

## 0.28.0 - 2026-09-12

### Added

- `vendor/freewheel`, covering FreeWheel GET ad requests on
  `*.v.fwmrm.net/ad/g/`: required `nw`, one of `csid` or `ssid`, and
  recommended `prof`. FreeWheel documents `setNetwork` as required and
  `setServer` as `/ad/g/1`. Uplynk documents the constructed GET table.
  Slot sections after a semicolon, `/ad/p/`, StickyAds
  `cdn.stickyadstv.com`, impression hops, and RTB are not contracted.

## 0.27.0 - 2026-09-12

### Added

- `vendor/doubleverify`, covering DoubleVerify impression beacons on
  `tps.doubleverify.com/visit.jpg` and `tpsc-video-*.doubleverify.com/visit.jpg`:
  required `ctx`, `cmp`, `plc`, and `sid`. Google documents those names on the
  generated wrapper as `dvparams`. OMID `dvtp_src.js`, RTB, VAST wrappers,
  and `event.png` quartiles are not contracted. Amazon-hosted `/dv/` hops
  stay `vendor/amazon-vfw`.

## 0.26.0 - 2026-09-12

### Changed

- `html`, `js`, and `gtm` are not validation kinds. CLI exit 2, MCP `-32602`,
  WASM throws. Extract tracking URLs, then `validate url`. Pixellint does
  not parse HTML, JavaScript, or GTM containers.
- `vendor/tiktok` matches only `/i18n/pixel/events.js` on
  `analytics.tiktok.com` and `analytics.us.tiktok.com`. Collect POST
  `/api/v2/pixel` is not contracted.
- `vendor/reddit`: `event` warns unless it is a documented standard name
  (PageVisit, Purchase, Custom, …).
- `vendor/linkedin`: `eventId` when present, from LinkedIn's dedup image URL.
- `vendor/x`: `p_id` expected `Twitter`; `tw_sale_amount` is a number with no
  `$`; `tw_order_quantity` is integer when present.

## 0.25.0 - 2026-09-11

### Added

- `vendor/iqm`, covering IQM Pixel conversion scripts on
  `pxl.iqm.com/i/pixel/{uuid}` and `pxl.stage.iqm.com/i/pixel/{uuid}`:
  required conversion ID in the path. IQM documents that generated
  snippet. Postback conversions are not contracted.
- `vendor/mediamath-mobile`, covering MediaMath mobile pixels on
  `pixel.mathtag.com/event/mob`: required integer `mt_adid` and
  `mt_id`, required `mt_uuid`, and `mt_idt` as `idfa`, `aaid`, or
  `waid`. SHA-256 `mt_exem` when present. Browser `/event/js` and
  `/event/img` stay `vendor/mediamath`. Cookie sync is not contracted.
- `vendor/trustarc-notice`, covering TrustArc CCM Advanced on
  `consent.trustarc.com/notice?domain=`: required `domain`. CCM Pro
  `/v2/notice/{cmId}` stays `vendor/trustarc`. `consent-st.trustarc.com`
  is not contracted.

## 0.24.0 - 2026-09-11

### Added

- `vendor/id5-ctv`, covering ID5 CTV S2S `api.id5-sync.com/gc/v1`:
  required `partner`, `ts`, `appid`, `ver`, `ip`, and `ua`. SHA-256
  `hem` when present. Mobile in-app `/ga/v1` stays `vendor/id5`.
  Cookie sync on `/i/` and `/s/` is not contracted.
- `vendor/nielsen-audit`, covering Nielsen ads audit pings on
  `audit.imrworldwide.com/cgi-bin/gn`: required `prd=audit`, `intid`,
  `sessionid`, `product`, and Unix `createtm`. DCR cfg stays
  `vendor/nielsen`. DCR measurement `/cgi-bin/gn` is not contracted.
- `vendor/flashtalking`, covering Flashtalking OneTag containers on
  `servedby.flashtalking.com/container/{advertiserId};{spotlightId};{spotlightGroupId};`:
  required integer path IDs. Tealium documents that generated snippet
  shape. Other Flashtalking hosts are not contracted.

## 0.23.0 - 2026-09-11

### Added

- `vendor/xandr`, covering Microsoft Monetize conversion pixels on
  `ib.adnxs.com/px` and `secure.adnxs.com/px`: required integer `id`,
  and `t` as `1` (JavaScript) or `2` (image). Numerical `value` when
  present. Cookie sync and RTB on the same hosts are not contracted.
- `vendor/xandr-sspx`, covering server-side conversion
  `sspx-router.adnxs.com/sspx` and `secure.adnxs.com/sspx`: required
  integer `id` and `sspdata`. Browser `/px` stays `vendor/xandr`.
- `vendor/kevel`, covering Kevel `e-{networkId}.adzerk.net/i.gif` and
  `/e.gif`: required shim `e` and signature `s`. Click redirects on
  `/r` are not contracted.
- `vendor/kantar`, covering Kantar
  `secure.insightexpressai.com/adServer/adServerESI.aspx`: required
  integer `bannerID`. ART19 documents the generated tag shape.

## 0.22.0 - 2026-09-11

### Added

- `vendor/google-ad-manager`, covering Ad Manager
  `pubads.g.doubleclick.net/gampad/ads` and
  `securepubads.g.doubleclick.net/gampad/ads`: required `iu` as
  `/network_code/.../ad_unit`. Floodlight stays `vendor/floodlight`.
- `vendor/trustarc`, covering CCM Pro
  `consent.trustarc.com/v2/notice/{cmId}`: required Consent Manager ID in
  the path. CCM Advanced `/notice?domain=` is not contracted.
- `vendor/id5`, covering mobile in-app S2S `api.id5-sync.com/ga/v1`:
  required `partner`, `ts`, `bundle`, `ver`, `ip`, and `ua`. SHA-256
  `hem` when present. Cookie sync on `/i/` and `/s/` is not contracted.
- `vendor/yahoo-conversions-api`, covering Standard CAPI
  `batch.datax.yahoo.com/v1/events/{pixelId}`: required integer pixel ID
  in the path, and `eventTs`, `eventName`, `actionSource`, and `userData`
  in the body, with at least one match key. Dot pixels stay
  `vendor/yahoo-dot`.

## 0.21.0 - 2026-09-11

### Added

- `vendor/didomi`, covering Didomi `sdk.privacy-center.org/{Public API Key}/loader.js`:
  required Public API Key in the path. Core and UI SDK files are not
  contracted.
- `vendor/lotame`, covering Lightning Tag
  `tags.crwdcntrl.net/lt/c/{clientId}/lt.min.js`: required integer client ID
  in the path. `bcp.crwdcntrl.net` stays directory-only.
- `vendor/ias`, covering IAS Signal display
  `pixel.adsafeprotected.com/rjss/st/{advertiserId}/{publisherId}/skeleton.js`:
  required integer advertiser and publisher IDs in the path.
- `vendor/ias-video`, covering IAS Signal video
  `unified.adsafeprotected.com/v2/{advertiserId}/{publisherId}`: the same two
  IDs. Display stays `vendor/ias`.
- `vendor/mediamath`, covering MathTag `pixel.mathtag.com/event/js` and
  `/event/img`: required integer `mt_id` and `mt_adid`. Hashed `mt_exem` /
  `mt_excl` when present. `sync.mathtag.com` stays directory-only.
- `vendor/oracle-bluekai`, covering `tags.bluekai.com/site/{siteId}` and
  `stags.bluekai.com/site/{siteId}`: required integer site ID. SHA-256
  `e_id_s` when present.

## 0.20.0 - 2026-09-11

### Added

- `vendor/adobe-ecid`, covering Adobe Visitor ID Service `dpm.demdex.net/id`:
  required `d_ver=2`, and either `d_orgid` to mint an ECID or `d_mid` to reuse
  one. Audience Manager `/event` stays directory-only.
- `vendor/heap-identify`, covering `heapanalytics.com/api/v1/identify`:
  required `app_id`, numeric SDK `user_id`, and `identity`. Track stays
  `vendor/heap-track`.
- `vendor/heap-user-properties`, covering
  `heapanalytics.com/api/add_user_properties`: required `app_id` and
  `identity`. `properties` is recommended. Bulk `users[]` is not contracted.
- `vendor/cookiebot`, covering `consent.cookiebot.com/uc.js`: recommended
  domain group `cbid` as a UUID. Cookiebot also accepts `data-cbid` on the
  script tag.
- `vendor/cookiebot-declaration`, covering `consent.cookiebot.com/{cbid}/cd.js`:
  required UUID in the path. The banner stays `vendor/cookiebot`.
- `vendor/onetrust`, covering
  `cdn.cookielaw.org/consent/{id}/OtAutoBlock.js`: required domain script ID
  in the path. `otSDKStub.js` with `data-domain-script` is not contracted.
- `vendor/zendesk`, covering `static.zdassets.com/ekr/snippet.js`: required
  widget `key`.
- `vendor/drift`, covering `js.driftt.com/include/{cacheWindow}/{embedId}.js`:
  required cache window and embed ID in the path.
- `vendor/mailchimp`, covering
  `chimpstatic.com/mcjs-connected/js/users/{user}/{site}.js`: required user
  hash and connected-site hash in the path.
- `vendor/pardot`, covering Account Engagement `pdt.js?aid=`: required
  integer account ID. Legacy `pd.js` stays unpacked.
- `vendor/nielsen`, covering DCR `secure-dcr.imrworldwide.com/cgi-bin/cfg`:
  required `apid`. Recommended `apn` and `sfcode` of `dcr` or `dcr-cert`.
- `vendor/ispot-conversion`, covering `pt.ispot.tv/v2/{TC-####-#}.gif`:
  required Site ID in the path. Recommended `type` for the conversion event.
  Impression GIFs stay `vendor/ispot`.

## 0.19.0 - 2026-09-11

### Added

- `vendor/tiktok-events-2`, covering Events API 2.0 `/event/track/`: required
  `event_source`, `event_source_id`, `data[].event`, and 10-digit
  `event_time`. Hashed `user.email`/`phone`/`external_id`. Revenue events
  need value and currency. Events 1.0 stays `vendor/tiktok-events-api`.
- `vendor/heap-track`, covering `heapanalytics.com/api/track`: required
  `app_id`, `event`, and exactly one of `identity` or `user_id`. The Heap.js 5
  loader stays `vendor/heap`.
- `vendor/intercom-events`, covering `api.intercom.io/events`: required
  `event_name`, 10-digit `created_at`, and a contact identifier. The Messenger
  loader stays `vendor/intercom`.
- `vendor/google-ads-call-conversions`, covering `UploadCallConversions`:
  required E.164 `callerId`, `callStartDateTime`, and `conversionDateTime`.
  Click uploads stay `vendor/google-ads-click-conversions`.
- `vendor/impact-conversions`, covering
  `api.impact.com/Advertisers/{AccountSID}/Conversions`: required `CampaignId`
  and one of `ActionTrackerId`/`EventTypeId`/`EventTypeCode`. The UTT loader
  stays `vendor/impact`.
- `vendor/yandex-watch`, covering `mc.yandex.ru/watch/{counter_id}`: required
  numeric tag ID in the path. Measurement Protocol stays
  `vendor/yandex-metrica`.
- `vendor/mixpanel-import`, covering Mixpanel `/import`: required `event`,
  `properties.time`, `properties.distinct_id`, and `$insert_id`. `/track`
  stays `vendor/mixpanel`.
- `vendor/adobe-web-sdk`, covering Edge Network `/interact` and `/collect`:
  required `datastreamId` and ISO 8601 `xdm.timestamp`. AppMeasurement
  `/b/ss/` stays `vendor/adobe-analytics`.
- `vendor/parsely-collect`, covering `p1.parsely.com` collect: required
  `idsite`, recommended `url` and `action`. The loader stays `vendor/parsely`.
- `vendor/awin-mastertag`, covering `www.dwin1.com/{advertiserId}.js`:
  required numeric advertiser ID in the path. Conversion pixels stay
  `vendor/awin`.
- `vendor/heap-classic`, covering `cdn.heapanalytics.com/js/heap-{appId}.js`:
  required numeric environment ID. Heap.js 5 stays `vendor/heap`.
- `vendor/google-ads-conversion-adjustments`, covering
  `UploadConversionAdjustments`: required `adjustmentType`,
  `adjustmentDateTime`, and `orderId` or `gclidDateTimePair.gclid`.
  `RESTATEMENT` needs `restatementValue.adjustedValue`. Click uploads stay
  `vendor/google-ads-click-conversions`.
- `vendor/taboola-s2s`, covering `trc.taboola.com/actions-handler/log/3/s2s-action`:
  required `click-id` and Realize `name`. `currency` is the documented
  three-letter list when present. The loader stays `vendor/taboola`.
- `vendor/taboola-s2s-bulk`, covering
  `trc.taboola.com/{account-id}/log/3/bulk-s2s-action`: required numeric
  account ID in the path, plus `actions[].click-id`, millisecond
  `timestamp`, and `name`. Single postbacks stay `vendor/taboola-s2s`.
- `vendor/cloudflare`, covering `static.cloudflareinsights.com/beacon.min.js`:
  recommended site `token` on the query. Automatic injection puts the token
  in `data-cf-beacon` instead.
- `vendor/mixpanel-engage`, covering Mixpanel `/engage`: required `$token`,
  `$distinct_id`, and one profile operation (`$set` and the rest). `/track`
  stays `vendor/mixpanel`.
- `vendor/mixpanel-groups`, covering Mixpanel `/groups`: required `$token`,
  `$group_key`, `$group_id`, and one group operation (`$set` and the rest).
  `/engage` stays `vendor/mixpanel-engage`.
- `vendor/liveramp-envelope`, covering
  `api.rlcdn.com/api/identity/v2/envelope`: required integer `pid`,
  identifier type `it` (`4` hashed email, `11` hashed phone, `15` custom ID),
  and nonempty `iv`. Cookie sync on `idsync.rlcdn.com` stays directory-only.
- `vendor/liveramp-envelope-refresh`, covering
  `api.rlcdn.com/api/identity/v2/envelope/refresh`: required integer `pid`,
  envelope type `it` (`19` ATS, `24` Meta-scoped), and envelope value `iv`.
  Retrieve stays `vendor/liveramp-envelope`.
- `vendor/amplitude-identify`, covering Amplitude `/identify`: required
  `api_key` and `identification`. HTTP V2 stays `vendor/amplitude`.
- `vendor/amplitude-group-identify`, covering Amplitude `/groupidentify`:
  required `api_key` and `identification`. User identify stays
  `vendor/amplitude-identify`.
- `vendor/taboola-unip`, covering Taboola `/log/3/unip` event pixels:
  required `en`. The `tfa.js` loader stays `vendor/taboola`.

### Changed

- `vendor/adobe-analytics` contracts Adobe's query-parameter table: `g` as
  a URL, nonempty `pageName`/`events`/`products`, `pe` as `lnk_o`/`lnk_d`/
  `lnk_e`/`tnt`, and `AQB` requiring `AQE`.
- `vendor/matomo` recommends `action_name`, `url`, and 16-hex `_id`. An event
  category without an action, or an order without `revenue`, is an error.
- `vendor/chartbeat` format-checks QA ping keys `p`, `d`, `t`, `g0`, `g1`,
  and `i` when they are present.
- `vendor/floodlight` format-checks `qty`, `cost`, `dc_lat`, and custom
  variables `u1`–`u20` when they are present.
- `vendor/meta` format-checks Advanced Matching `em`/`ph`/`fn`/`ln`/
  `external_id` as SHA-256 hex and `fbc`/`fbp` as `fb.N.timestamp.value`.
- `vendor/reddit-conversions-api` requires a match key, `custom_event_name`
  on `CUSTOM`, `event_source_url` on `WEBSITE` (warning), and
  `metadata.value` on `PURCHASE` (warning).
- `vendor/tiktok-events-api` warns when an event has no hashed identifier or
  IP, and errors when `CompletePayment` or `PlaceAnOrder` omits value and
  currency.
- `vendor/mixpanel` recommends `distinct_id` and `$insert_id`, and
  format-checks `time` as an integer. `/import` is now
  `vendor/mixpanel-import`.
- `vendor/google-ads-click-conversions` excludes `conversionAdjustments` so
  an adjustment payload is not also claimed as a click upload.

## 0.18.0 - 2026-09-11

### Added

- `vendor/microsoft-conversions-api`, covering Microsoft Advertising CAPI on
  `capi.uet.microsoft.com` `/v1/{tagId}/events`: required `eventType`,
  `eventTime` in seconds, `userData` with at least one identifier, and
  `eventSourceUrl` on `pageLoad`. Browser UET stays `vendor/microsoft-uet`.
- `vendor/nextdoor-conversions-api`, covering `/v2/api/conversions/track`:
  required `event_name`, `action_source`, `event_id`, `event_time_epoch`,
  `data_source_id`, and a `customer` match key. `action_source_url` on
  website events. `custom.order_value` on purchase.
- `vendor/partnerize`, covering `prf.hn/conversion`: required `campaign`,
  `clickref`, and ISO 4217 `currency` in colon-delimited path segments. The
  Performance Horizon loader stays directory-only.
- `vendor/chartbeat`, covering `ping.chartbeat.net/ping`: required site id
  `h` and numeric account UID `g`. `chartbeat.js` stays directory-only.
- `vendor/heap`, covering Heap.js 5 `heap_config.js` on `cdn.us.heap-api.com`
  and `cdn.eu.heap-api.com`: required numeric environment ID in the path.
  Classic `heap-{id}.js` stays directory-only.
- `vendor/microsoft-clarity`, covering `www.clarity.ms/tag/{projectId}`:
  required project ID in the path. Collect POSTs stay directory-only.
- `vendor/mouseflow`, covering `cdn.mouseflow.com/projects/{website_id}.js`:
  required website ID in the path.
- `vendor/intercom`, covering `widget.intercom.io/widget/{app_id}`: required
  workspace ID in the path. The IAM API host stays directory-only.

### Changed

- Vendor directory splits Microsoft by product: UET on `bat.bing.com`, CAPI
  on `capi.uet.microsoft.com`, and Clarity on `clarity.ms` pointing at
  `vendor/microsoft-clarity`.
- Heap directory row adds `heap-api.com` and points at `vendor/heap`.

## 0.17.2 - 2026-09-04

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
