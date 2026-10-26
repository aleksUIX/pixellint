# Teads Conversions API uses a bounded source profile.

Teads Conversions API endpoint and locally checkable payload contracts.

40 fixtures cover source examples, native alternatives, mutations and scope controls. A fixture count is not a percentage of vendor specification coverage.

- Direct documentation specifies Token authorization; the current official GTM sender emits Bearer. Both syntaxes are accepted; Bearer gets an information advisory pending receiver-profile confirmation.
- Direct docs specify seconds and reception-time override; the official sender emits milliseconds. Event time is checked as an integer without a guessed unit or rejection window.
- Direct examples use a numeric pixel ID and price. The configurable sender also emits strings. These native alternatives are retained.
- Conversion type existence, token/pixel ownership, attribution, session state and deduplication cannot be established offline.
- The studio-t.teads.tv callbacks in the retained corpus are a separate undocumented protocol. Only ca.teads.tv /v1/event and /v1/test/event are selected.
