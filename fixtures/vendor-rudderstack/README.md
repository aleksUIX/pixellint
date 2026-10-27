# The RudderStack HTTP and Pixel API profile has a bounded source contract.

Per-record page/screen name advice for the explicit documented HTTP producer profile, with route inference and SDK scope preserved.

This round adds 48 synthetic cases. All 18 prior fixture expectations are retained.

- The HTTP API page/screen table marks name required, while Ruby SDK documentation marks it optional. The new warning applies only to the explicit context.library.name=http profile, never as a universal SDK rejection rule.
- Typed events receive body advice. Exact page/screen routes infer advice only when type is absent. This avoids duplicate findings and leaves unresolved template type or library values deferred.
- Names and library markers remain record-local in batches. A root or sibling name or library marker cannot satisfy or activate another record.
- Unmarked clients and SDK-specific profiles are outside the new advisory. A marked HTTP profile still needs caller confirmation if its actual producer contract differs from the documented example.
- Source credentials, receiver enrichment, configured transformations, delivery and destination-specific requirements remain external. Existing Basic authentication and event/identity contracts are retained.

Sources read:

- https://www.rudderstack.com/docs/api/http-api/
- https://www.rudderstack.com/docs/sources/event-streams/sdks/rudderstack-ruby-sdk/
