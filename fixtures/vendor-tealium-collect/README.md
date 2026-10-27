# Tealium Collect HTTP API v1 has a bounded source profile.

Warn about literal periods in nested JSON member names and the documented override that prevents TAPID cookie deletion.

This round adds 37 cases. 23 prior cases are retained; 0 prior expectations change for documented reasons.

- Nested literal-period key findings are warnings based on the source JSON-key restriction. Nested object flattening, storage, accepted custom value types and receiver transformations remain outside this profile.
- Cookie deletion advice applies to decoded POST bodies and complete GET captures. Bare URLs do not prove the HTTP method and receive no cookie-precedence finding. Unknown override macros defer the advice.
- Account configuration, AudienceStream identity requirements, TAPID state, actual cookie deletion, bot filtering, account ownership and receiver acceptance remain external.
- The source 1 MB wording and combined account/profile/datalayer_id rule remain ambiguous. No byte or aggregate-size limit is guessed. Host and path coverage is unchanged.

Sources:

- https://docs.tealium.com/platforms/http-api/endpoint/
