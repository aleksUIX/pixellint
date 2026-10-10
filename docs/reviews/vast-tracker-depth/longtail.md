# The reported video trackers need published contracts before vendor checks can be claimed.

Reviewed on October 9, 2026, against Pixellint 0.37.0 and public primary sources.
This review uses the supplied host inventory and authored synthetic fixtures.
It does not publish customer tags, identifiers or tracking query strings.

## The 00px counting-pixel template supports a narrow advisory pack.

The [official counting-pixel guide](https://wiki.00px.com.br/en/technologies/pixel)
directly identifies `00px.net` and publishes `/pixel/{token}/e.gif` with a
`t=INSERIR+CACHEBUSTER` installation example. It does not publish a required
parameter table, token grammar, cachebuster type or collector response contract.

The new `vendor/adxspace-pixel` pack checks that exact counting-pixel route. An
empty token slot or the exact published cachebuster stub produces a warning
with `official_template` evidence. The token stays opaque. A short token,
non-base64 token, omitted `t`, empty `t`, nonnumeric cachebuster and known IAB
cachebuster macro remain accepted. Query fields cannot fill an empty path slot.
The pack does not declare that the collector will accept a tag remotely.

The reported `/tracking/{id}/starts`, `/tracking/{id}/firstquartiles`,
`/tracking/{id}/midpoints`, `/tracking/{id}/completes` and `/vast/pixel/` routes
remain outside this pack. Neither their generated identifiers nor their event
enumeration is documented by the counting-pixel guide. Do not apply its query
field to those routes.

The [traffic-tag guide](https://wiki.00px.com.br/en/technologies/tag) independently
uses `cdn.00px.net`. The [VAST 2.0](https://wiki.00px.com.br/en/technologies/vast2),
[VAST 3.0](https://wiki.00px.com.br/en/technologies/vast3),
[VAST 4](https://wiki.00px.com.br/en/technologies/vast4) and
[VPAID](https://wiki.00px.com.br/en/technologies/vpaid) guides describe ad-request
routes and player parameters. Those requirements are not tracking-pixel
contracts. The [analytics API page](https://wiki.00px.com.br/en/technologies/api)
describes reporting aggregates, not how event collectors receive requests.
The JavaScript producer asset linked by the tag guide returned HTTP 404 during
this review, so it supplies no verifiable producer contract.

## Stredeo, Krush Media, Smadex and AdWrap remain outside vendor tracker contracts.

The [Stredeo site](https://www.stredeo.com/) identifies its CTV/OTT ad-insertion
platform. Its public page has no tracker parameter or event specification for
`t.stredeo.com/v1/...`. Directory attribution remains useful, and core's
`gdpr=NaN` error remains active. A Stredeo pack repeating that core rule would
not establish destination-specific validation.

[Krush's technology](https://krushmedia.com/technology/),
[advertiser solutions](https://krushmedia.com/advertiser-solutions/) and
[privacy information](https://krushmedia.com/privacy-policy/) identify its
advertising business and support domain attribution. They do not define the
root tracking URL contracts on `ads106.krushmedia.com` or
`ads133.krushmedia.com`. Its bid-request, supply-chain and consent claims do
not justify mandatory fields on an impression pixel.

The [Smadex documentation index](https://docs.smadex.com/llms.txt) links MMP
setup and CTV attribution guides. Its integration requirements apply to partner
tracking links, onboarding or postback settings. They do not specify
`/hyperad/tracking/action/{event}` on the six observed Smadex hosts. Those hosts
keep directory attribution and core validation. No closed quartile enum,
mandatory event query, UUID format or unconditional consent field is inferred.

No independently documented collector contract for
`track.adwrap.io/api/track/{event}` was found in the examined sources. The
public apex `adwrap.io` did not resolve during the review. Similar names on
other corporate domains were excluded because their identity was not
established. Existing attribution from the supplied inventory is retained.

## ActiveMetering's exact host belongs to DISQO in platform partner documentation.

[Roku's official measurement guide](https://help.ads.roku.com/en/articles/10386309-measuring-campaigns-with-third-party-solutions)
explicitly maps `track.activemetering.com` to Disqo. The
[Edmunds accepted-vendor list](https://www.edmunds.com/media-kit/ad-specs/third-party.html)
independently maps the same exact host to DISQO. This supports an exact
directory-only DISQO entry, instead of inventing an ActiveMetering vendor pack.

[DISQO's developer portal](https://developer.disqo.com/) describes survey and
registration APIs. Its
[ad-measurement test-automation article](https://developer.disqo.com/blog/test-automation-for-ad-measurement/)
discusses engineering practices, rather than the collector's input contract.
Neither source specifies `/pixel/v1/all/pixel.gif`. Roku's own HTTPS, space,
macro and count restrictions govern submissions to Roku Ads Manager; they
must not become universal DISQO endpoint requirements.

## RZR's corporate identity is clear but the reported host contract is unresolved.

[RZR's official advertising privacy policy](https://www.rzr.com/legal/advertising-privacy-policy)
names RZR Global Inc. The [official RZR site](https://www.rzr.com/) identifies
the Aarki rebrand, and the current
[IAB Global Vendor List](https://vendor-list.consensu.org/v3/vendor-list.json)
names vendor 806 as RZR Global Inc., retaining Aarki privacy and device-storage
disclosure URLs. These establish corporate continuity, rather than the
contract or ownership of every similarly named domain.

All 32 Markdown pages linked by the
[RZR documentation index](https://help.rzr.com/llms.txt) were inspected. None
mentions `rzrglobal.com`, `/af-impression/`, `track.gif` or `rm.aarki.net`.
The MMP and CTV guides define external attribution links, while reporting
uses Aarki infrastructure. `pixel.rzrglobal.com` resolves, but the corporate
apex has no A record and `www.rzrglobal.com` returned NXDOMAIN. DNS resolution
alone cannot establish a vendor contract. This exact host remains deferred
pending direct vendor documentation or an independently attributable endpoint.

Talpa's development mock host remains a custom endpoint. A shared VAST event
name cannot turn it into a public vendor contract.

## The authored controls preserve meaningful coverage boundaries.

The 00px addition supplies 21 golden fixtures and source-case annotations,
plus three targeted integration tests. Positive and negative controls cover
cachebuster substitution, warning evidence, omitted inputs, opaque tokens,
query shadowing, neighboring routes, host lookalikes and unreviewed subdomains.
The two reported 00px video-route families stay outside vendor pack selection.
Core rejection of `gdpr=NaN` continues on those routes.

Public source retrieval metadata and unresolved requirements are recorded in
`longtail.sources.json`. No public source was treated as a full specification
because it repeated sampled URL shapes or described a corporate privacy policy.
