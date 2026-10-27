# The Segment HTTP Tracking API profile has a bounded source contract.

Optional object and boolean preference-map advice, with per-record scoping and open category IDs.

This round adds 56 regression cases. All 35 prior fixture expectations are retained.

- New consent fields are optional shape advice. Missing blocks, empty objects and false values retain their documented distinctions; no missing-category defaults are fabricated.
- Category IDs are configured and case-sensitive. Unknown names and extra fields remain open. Provider mappings, destination routing and actual consent enforcement require external state.
- A source example is stamped on a synthetic event wrapper. The Unify event example is retained directly from primary documentation. SDK execution and receiver acceptance are not claimed.

Sources read:

- https://www.twilio.com/docs/segment/privacy/consent-management/consent-in-segment-connections
- https://www.twilio.com/docs/segment/privacy/consent-management/consent-in-unify
