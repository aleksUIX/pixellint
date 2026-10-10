# These programmatic VAST trackers still lack public URL contracts.

Reviewed on October 9, 2026 against Pixellint 0.37.0. The supplied tracker
inventory identifies observed hosts and path families. It does not establish
which URL fields a destination requires. Directory attribution remains separate
from vendor validation. Core URL, macro and populated privacy checks still run.

This review found no source-supported vendor rule to add for the five observed
tracker families below. It records the exact missing contracts so a later review
can close them without guessing from pixel frequency or a sample event name.
The accompanying `programmatic.json` contains source URLs, pinned source commits
and per-destination follow-up questions. No private payloads are included.

## Adtelligent's tracker contract is separate from its ad request requirements.

Observed scope: `ads228.adtelligent.com` and `ads283.adtelligent.com`, with
`/t/e/` and `/tre/imp/` paths. Both hosts already receive directory attribution.

The [third-party tracking guide](https://support.adtelligent.com/850849-3rd-Party-Tracking)
lists events available when a user configures an external image pixel. It does
not define an event-name vocabulary for Adtelligent's own tracking URL.
The [client-side integration guide](https://support.adtelligent.com/481859-Client-side-Tag-to-Tag-Flow--Metrics)
places input validation before VAST wrapper delivery and describes impression
tracking as a later step. Its incoming request fields cannot be required on the
returned tracker. The [notification and billing guide](https://support.adtelligent.com/536457-nURL-and-bURL-support)
also distinguishes auction wins from billable impressions.

The [required-parameter guide](https://support.adtelligent.com/347898-Required-Parameters-for-different-traffic-types),
[platform macro manual](https://tools.adtelligent.com/ssp/) and
[video request/response examples](https://support.adtelligent.com/517612-RequestResponse-samples-video)
were reviewed. None specifies these two pixel routes. A general macro or privacy
definition does not establish that a tracking endpoint accepts the corresponding
query field.

The vendor's [Android SDK repository](https://github.com/Adtelligent/In_App_Ads_SDK_Android_Public)
is empty. The [Swift SDK package](https://github.com/Adtelligent/AdtelligentSwiftSDK/blob/da796524b5b8dcb8442c80d7a8eefa79e7dc57b4/Package.swift)
publishes a binary SDK; its [integration README](https://github.com/Adtelligent/AdtelligentSwiftSDK/blob/da796524b5b8dcb8442c80d7a8eefa79e7dc57b4/README.MD)
describes callbacks rather than HTTP tracker construction.

Required next evidence: supported route shapes, token ownership and encoding,
which fields are generated or replaceable, duplicate-query behavior, expiration,
and whether any event or privacy fields are mandatory on these trackers.

## Beeswax's event log schema does not define its event URL encoding.

Observed scope: `us-east-1.event.prod.bidr.io/log/act/svr` and `/log/imp/svr`.
The exact host already receives Beeswax directory attribution.

FreeWheel's [cookie-sync guide](https://api-docs.freewheel.tv/advertiser/docs/cookie-syncing)
corroborates Beeswax's `prod.bidr.io` infrastructure. Its documented
`match.prod.bidr.io/cookie-sync/` contract is a different endpoint.
The [bid lifecycle guide](https://api-docs.freewheel.tv/advertiser/v0.5/docs/beeswax-architecture-life-of-a-bid)
describes generated impression, click and video-event URLs reaching an event
server. The [macro manual](https://api-docs.freewheel.tv/advertiser/docs/macros)
identifies template macros which expand to those URLs. Neither page specifies
the generated URL's query contract or the meaning of the `/svr` suffix.

The official [streaming log schema](https://github.com/BeeswaxIO/beeswax-api/blob/06122d3c95adb16082a860f24785bc260439920f/beeswax/logs/streaming/ad_log.proto)
defines downstream activity records and event enums. It explicitly treats string
auction identifiers as opaque. The schema cannot establish that a URL token
serializes the same message, or turn optional record fields into mandatory pixel
fields. The [bid API schema](https://github.com/BeeswaxIO/beeswax-api/blob/06122d3c95adb16082a860f24785bc260439920f/beeswax/bid/request.proto)
describes additional creative pixels rather than generated tracker decoding.

Required next evidence: the wire schema for these exact endpoints, token
encoding and versioning, mandatory fields, event representation and signature
or expiry rules. No Base64, protobuf, auction-ID-format or event-enum check is
inferred from an observed token.

## Aarki's public integration guides describe MMP links rather than this pixel.

Observed scope: `rm.aarki.net/v1/api/{id}/track.gif`. The host already receives
Aarki directory attribution.

The [Aarki help entry point](https://help.aarki.com/) and its documentation index
now redirect to the [RZR help center](https://help.rzr.com/). Its
[CTV integration guide](https://help.rzr.com/readme/ctv-integration-guide.md),
[AppsFlyer CTV guide](https://help.rzr.com/readme/ctv-integration-guide/appsflyer-integration.md)
and creative requirements were reviewed. The AppsFlyer example is an
`impressions.onelink.me` URL with `pid=aarki_int`; it does not document the
`rm.aarki.net` tracker contract. Appsflyer attribution parameters and Beeswax
template macros must not be imposed on this route.

The [public Aarki repository inventory](https://github.com/orgs/aarki/repositories)
contains infrastructure libraries, but no published implementation of this
tracker. The documentation redirect supports a brand-transition lead. It does
not independently establish ownership of `pixel.rzrglobal.com`.

Required next evidence: the path ID's meaning and supported syntax, the query
schema, which parameters are mutable or required, event representation, and
whether the GIF route requires method, signature or privacy constraints.

## Sovrn documents VAST impression placement without documenting this route.

Observed scope: `n.adv.lijit.com/a/{id}`. The exact host already receives Sovrn
directory attribution. Its [Lijit domain documentation](https://knowledge.sovrn.com/kb/can-i-remove-the-lijit-com-lines-from-my-ads-txt-file)
corroborates ownership of the parent ad-serving domain.

The [impression methods guide](https://knowledge.sovrn.com/kb/what-impression-tracking-methods-does-sovrn-support)
states that video impression pixels belong inside VAST's Impression event.
The [bill-on-render update](https://knowledge.sovrn.com/kb/ad-exchange-fall-2025-impression-tracking-update)
applies to display inventory and explicitly excludes video and CTV. Neither
defines `/a/{id}` fields. These delivery rules also require surrounding ad
markup or runtime observations, which a standalone pixel does not contain.

The [developer documentation index](https://developer.sovrn.com/llms.txt)
contains commerce, curation and reporting APIs rather than this tracker. The
[bad-ad guide](https://knowledge.sovrn.com/kb/how-do-i-report-a-bad-ad)
warns that merely seeing a Sovrn-related domain does not prove it served the
impression. Directory attribution therefore must not be presented as proof of
auction participation, billing or successful firing.

Required next evidence: `/a/{id}` token semantics and supported path versions,
required query fields, replaceable macros, signature and expiry behavior, and
any distinct video versus display contract.

## The rtactivate host has no independently confirmed Index attribution.

Observed scope: `idxgm.rtactivate.com/tagid/{id}/`. The hostname prefix and
numeric-looking sample path cannot establish vendor ownership or an ID format.
The host is presently unattributed and receives core checks.

The [Index Exchange OpenRTB field guide](https://kb.indexexchange.com/dsps/open-rtb/list_of_supported_openrtb_bid_request_fields_dsp.htm)
does not identify this domain. An independently encountered publisher consent
notice associated the host with Scoota, which conflicts with the supplied Index
label. That notice is a research lead, not vendor confirmation. Scoota's
[privacy page](https://www.scoota.com/privacy) describes its advertising services
but does not confirm this domain or a `/tagid/` URL contract.

Required next evidence: an operator-controlled domain declaration or published
SDK that independently identifies this exact host, then a URL schema covering
tag IDs and query fields. Keep both vendor attribution and vendor validation
unclaimed until that evidence is available.
