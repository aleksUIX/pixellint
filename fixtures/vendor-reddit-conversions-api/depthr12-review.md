# Reddit Conversions API gains documented checks.

The official v3 OpenAPI resolves data.events, required parent envelopes, one to 1,000 events, optional test_id and referrer_url, order-ID truncation, nonnegative numeric metadata, 0..32767 screen dimensions and required per-product IDs. v4 is not inferred. Unknown extension keys remain open despite additionalProperties:false in the published model.

The guide requires events within seven days. The historical boundary is inclusive at millisecond precision; future policy and unsupported timestamp spellings are not inferred. Existing 13-digit native timestamp checks retain ownership of syntax. Historical fixtures now carry an explicit clock.

Custom event names accept all UTF-8 characters through 64 Unicode scalar values, including line breaks. The current schema describes avoiding standard names as a recommendation. Its existing finding code is retained at warning severity.

The exact v3 conversion route owns POST, JSON entity and OAuth Bearer checks. HTTPS remains advice. Token issuance, expiry, adsconversions scope, account permissions, quota windows and receiver acceptance require external context. Neighboring paths and versions remain outside this owner.

Unavailable/redacted captures and unsupported compressed entities remain unknown. Native forced-only malformed JSON fixtures retain selection and expected errors; public npm automatic-mode controls explicitly retain Core-only behavior where no vendor discriminator exists.

Required product-ID presence adds a finding to four pre-existing type mutations whose product objects already lacked IDs. Their original type findings are preserved. The reserved-name expectation changes severity. The other 65 prior expectations remain unchanged apart from pinned validation clocks.

This round adds 95 golden cases. Primary sources:

- https://ads-api.reddit.com/api/v3/openapi.json
- https://ads-api.reddit.com/docs/v3/guides/programs/capi/direct-integration
- https://ads-api.reddit.com/docs/v3/guides/programs/capi/migration
