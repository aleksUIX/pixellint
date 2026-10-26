# The pack covers the current VPAID error sender.

The inspected first-party PMAdMgr.js artifact has SHA256
`8acdd9de15825f4e55eecac4172296c459fcbec793c3fbb0ba593552f1c18b54`.
Its exact getErrorURL builder unconditionally emits `operId=7` and `vadsId`.
Both markers are required for automatic selection, including a blank vadsId.
The retained legacy notices have no vadsId, so this new profile claims no
saved-corpus gain. The broader operation 7 protocol is not established here.

Run the offline source replay:

```sh
node fixtures/vendor-pubmatic-vpaid-error/source-replay.mjs /path/to/PMAdMgr.js
```

The harness extracts the unmodified reviewed constructor, URL builder,
error-macro substitution and image-transport methods. Full-source and extracted
function hashes are pinned. DOM image loads are local sinks. Logging and
unrelated timestamp/HTTPS helpers are explicit deterministic stubs. This
does not claim whole-runtime equivalence or receiver acceptance.

Twenty source cases preserve native zero/blank identifiers, negative timestamps,
the numeric consent sentinel versus its string representation, omitted falsy
configuration, string 0 optional values, strict numeric isAk gating, equivalent
numeric network values and explicit event/error alternatives. The manifest
checks timestamp spelling without a clock window, not account identity or
provisioning. The wc presence advisory covers canonical wa=243 only; other
JavaScript numeric equivalents remain accepted without an invented exhaustive
coercion parser. Unknown event/error values and dual carriers remain open.

The pack does not cover AdDisplayTrackerServlet, feed-notif video routes,
regional click routes, notices without the current marker, unknown operation
values or aliases. A directory pointer is an attribution/reference aid and
does not extend the manifest selector to other PubMatic hosts or services.
