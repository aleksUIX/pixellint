# Tealium Collect HTTP API v1 uses a bounded source profile.

Tealium Collect HTTP API v1 endpoint and locally checkable payload contracts.

23 fixtures cover source examples, native alternatives, mutations and scope controls. A fixture count is not a percentage of vendor specification coverage.

- AudienceStream versus EventStream and known versus anonymous visitor identity require caller context. No universal visitor-ID presence or GUID grammar is enforced.
- The 1 MB and combined account/profile/datalayer_id wording is ambiguous. No guessed byte or combined-field bound is imposed.
- Only the documented v1 Collect host and exact /event route are covered. v3, other hosts, bot filtering, cookie state and remote account validity remain outside this profile.
- Top-level custom key periods are checked. Nested attribute interpretation and receiver-side transformations remain unverified.
