# Yahoo Conversion API receives captured-request checks.

Standard Yahoo CAPI /v1/events/{pixelId} captures gain transport and OAuth advice. Missing pixel slots retain diagnostics. Neighboring paths and Product CAPI are outside this contract.

This round adds 69 golden fixtures. All 82 prior native expectations are unchanged.

- Current Standard CAPI docs require OAuth, but the same error table retains an initial-release nonenforcement note. Missing and malformed Bearer captures are warnings. Opaque token strings, including the quoted documentation example, are not interpreted as JWTs or verified for expiry or issuance.
- POST is explicitly documented. MIME, JSON entity and HTTPS are advisory transport checks. Invalid or missing MIME owns its own advice; dependent entity advice is limited to known JSON MIME or an observably absent body. The generic query-only error does not establish a modeled alternate protocol.
- Only the standard /v1/events/{pixelId} route family is selected. Missing and empty pixel slots keep existing diagnostics. Deeper paths, suffixes and sibling versions remain outside this contract. Pixel IDs cannot be supplied by query parameters.
- Unavailable/redacted fields remain unknown, while observed method and header violations retain checks. Existing event, identity, privacy, time and metadata contracts remain unchanged. OAuth validity, advertiser allowlisting, source-ID issuance, receiver acceptance and traffic-level quotas require external context.

Primary sources retrieved:

- https://help.yahooinc.com/dsp-api/docs/standard-yahoo-conversion-api
- https://help.yahooinc.com/dsp-api/docs/authentication
