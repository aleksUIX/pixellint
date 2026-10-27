# Braze user track receives source-scoped checks.

Braze standard /users/track uploads. Combined-array budget advice preserves legacy per-array limits. Bulk, sync and status routes use separate contracts and are not selected.

This round adds 73 golden fixtures. 0 prior severity expectations are corrected; other prior expectations are unchanged.

- The current standard contract limits attributes, events and purchases to 75 combined objects. Legacy contracts allow 75 per array. The aggregate finding is a warning because account policy is unavailable locally; it never asserts universal receiver rejection.
- The standard URL owner selects exact /users/track only. /users/track/bulk, /sync, /status, suffixes and prefixed neighbors remain outside this contract. Bare JSON keeps the existing discriminator and cannot reveal a destination route.
- Only immediate native array elements contribute to the combined budget. Nested properties and unrelated root keys do not add objects. Known arrays can prove an excessive lower bound even when a different field has an invalid type.
- Existing per-record contracts, identifier alternatives and three per-array 75-object limits remain unchanged. Account-specific policies, current rate usage, prior profile state and receiver acceptance remain external.

Primary sources read:

- https://www.braze.com/docs/api/endpoints/user_data/post_user_track/
- https://www.braze.com/docs/api/api_limits/
- https://www.braze.com/docs/api/endpoints/user_data/post_user_track_bulk/
