# Snapchat Conversions API gains captured request checks.

The current v3 guide, migration guide, validation guide and pinned official server-side GTM template establish exact /v3/{asset}/events and /v3/{asset}/events/validate route families. Neighboring paths and versions remain uncontracted. The same published producer covers both routes.

Captured POST receives an explicit method check. JSON MIME, JSON entity and HTTPS receive advisory checks. Captured object bodies missing data receive a v3 envelope recommendation; supplied data types retain the existing native body checks without duplicate HTTP findings. Legacy bare-JSON and scalar/list unions remain unchanged.

CAPI uses the access_token query parameter. No Ads API Authorization-header requirement is transplanted into this endpoint. Existing query-token presence checks are preserved. Body-only and Bearer-only credentials do not satisfy that documented carrier.

Six controls execute the pinned official GTM template without source changes. GTM host APIs use deterministic stubs and sendHttpRequest is intercepted. Four requests cover production, validation, default milliseconds and a template token; missing token and asset fail before transport. This does not certify GTM host behavior or vendor acceptance.

Unavailable/redacted capture evidence and unsupported compressed entities stay unverified. Asset/account ownership, token validity, live consent and cross-channel deduplication remain external. All 182 prior native fixture definitions and expectations remain unchanged.

This round adds 66 source-reviewed golden cases. Primary sources:

- https://developers.snap.com/marketing-api/Conversions-API/UsingTheAPI
- https://developers.snap.com/marketing-api/Conversions-API/MigrationGuide
- https://developers.snap.com/marketing-api/Conversions-API/VerifySetUp
- https://raw.githubusercontent.com/Snapchat/capi-google-tag-manager-serverside-tag/0038f617ce4d5b41a4b7b6b059cac20596953bc6/template.tpl
