# The mParticle historical batches profile has a bounded source contract.

Typed contracts for the 17 published input event branches, per-record crash breadcrumb scopes, and correction of unsupported $ref sibling-enum errors. Source-defined nullable and untyped alternatives remain accepted.

This round adds 410 cases. Prior fixture expectations are retained.

- The input schema publishes 17 event branches. Three reuse common event data without additional branch fields. pre_attribution and push_reaction are enum-only values without a complete input branch; their event-specific payloads remain uncontracted.
- Draft 7 ignores siblings of $ref. Optional OS/orientation, push application_state and push_message_behavior sibling enums are not promoted to receiver rejection rules. A version-specific receiver contract would be needed to tighten them.
- Source containers without an explicit native type retain source-permitted scalar or null alternatives. Descendant properties are checked only when the submitted value is an object.
- Closed-key policies, source-only annotations, duplicate raw keys, account identity resolution, credentials and receiver transformations remain outside this profile.
- The 128kb batch unit is ambiguous. Historical bulk docs still conflict with a single-object example. Existing explicit profile advice and the 72-hour top-level event window are retained.
- Nested crash breadcrumbs are checked per element under their own crash parent. They are not assigned an undocumented independent historical timestamp requirement.
