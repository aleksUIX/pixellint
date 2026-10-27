# The Intercom Events API profile has a bounded source contract.

Rich-link URL parser advice, case-insensitive HTTP scheme correction and explicitly heuristic monetary currency assignment advice.

This round adds 50 synthetic cases. All 5 prior fixture expectations are retained.

- Rich-link URL syntax uses the existing URL parser, with a separate HTTP(S) scheme gate. HTTP schemes are case-insensitive. This does not check availability, redirects or receiver acceptance.
- Currency assignment is explicitly heuristic advice when amount is present. The public money examples do not establish an exhaustive ISO 4217 receiver allowlist. Current, historical and lowercase assigned values are preserved.
- Metadata checks stay isolated per key. A monetary sibling cannot activate currency advice for a rich link, and plain metadata strings are not reinterpreted as URL objects.
- Older Intercom 2.0 prose documents both _date and _at metadata suffixes, while current and 2.14 documentation describes _date. No universal _at requirement is added across versioned APIs.
- Account defaults, metadata provisioning, contact lookup, authentication validity, version-specific acceptance and remote delivery remain external.

Sources read:

- https://developers.intercom.com/docs/references/rest-api/api.intercom.io/data-events/createdataevent
- https://developers.intercom.com/docs/references/2.14/rest-api/api.intercom.io/data-events/createdataevent
- https://developers.intercom.com/docs/references/2.0/rest-api/data-events/submitting-events
- https://url.spec.whatwg.org/
