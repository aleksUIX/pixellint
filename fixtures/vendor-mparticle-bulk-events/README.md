# mParticle bulk events uses a bounded source profile.

mParticle bulk events endpoint and locally checkable payload contracts.

269 fixtures cover source examples, native alternatives, mutations and scope controls. A fixture count is not a percentage of vendor specification coverage.

- The primary inbound JSON Schema takes precedence over the older OpenAPI where native types and defaults disagree. Custom attribution is additionally preserved from the public JSON reference; unknown environment values remain advisory.
- The typed inventory covers batch metadata, common event data and the custom, screen and commerce branches. Other event-specific data contracts remain partial.
- Endpoint captures select single, bulk and historical profiles. Generic standalone JSON cannot identify the intended endpoint and is not automatically claimed.
- The 128kb batch-size unit is ambiguous and per-user batch identity requires account context. No guessed size conversion or user-equality rule is imposed.
- Historical docs specify a bulk array but show one object. Both are inspected; the object receives an information advisory. Every historical event requires a timestamp at least 72 hours old.
- Closed-key enforcement, all receiver transformations, identity resolution, deduplication history, credentials, forwarding and retention state remain unvalidated.
