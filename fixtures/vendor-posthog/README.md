# The PostHog capture profile has a bounded source contract.

Documented modern operation methods and populated event entities on captured requests, preserving legacy, preflight and incomplete-capture boundaries.

This round adds 65 synthetic cases. All 19 prior fixture expectations are retained.

- Only exact modern /i/v0/e and /batch operation paths receive the new method and populated entity checks. Legacy /e/ and /capture/ behavior is retained. OPTIONS is excluded from these event checks; this does not establish complete CORS support.
- The existing automatic matcher does not claim a bare /batch without its trailing slash. Endpoint selectors remain unchanged; no additional sample reach is claimed.
- No universal JSON media type, Bearer header, compression format or project-specific token validation is inferred from curl examples. Generic unavailable or undecodable body checks remain deferred.
- A populated locally decoded entity is checked, not receiver delivery. Project configuration, authorized tokens, acceptance, quotas and billing behavior remain external.

Sources read:

- https://raw.githubusercontent.com/PostHog/posthog.com/master/contents/docs/api/capture.mdx
- https://posthog.com/docs/api/capture
