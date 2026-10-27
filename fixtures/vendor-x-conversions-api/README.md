# X conversion API has a bounded source profile.

Support current official sender timestamp and event-name alternatives, validate batch containers, retain signed numeric output, and distinguish both documented authentication routes.

This round adds 69 cases. 16 prior cases are retained; 1 prior expectations change for documented reasons.

- Current direct documentation and a pinned official GTM sender support this local profile. The full linked receiver reference still redirects to an overview. No complete receiver-schema claim is made.
- The sender uses conversion_timestamp in milliseconds and event_name when Event ID is unset. Their native shapes are advisory. Existing supplied legacy fields stay checked when alternatives also exist.
- IP plus user agent receives warning-level advice because the direct guide permits the pair while the GTM guide recommends a strong identifier. Pixel-cookie transmission is supported by the pinned sender; its matching success is not established.
- Container shape checks, signed and exponent numeric compatibility, and POST transport advice do not certify delivery, authentication, normalization, attribution, deduplication, quotas, batch limits or cookie provenance.
- Existing host and URL selectors are retained. One distinctive conversion_timestamp bare-JSON discriminator is added. No fresh stored-sample reach measurement is claimed.
- The source sender is executed unmodified in Node vm with deterministic mocked GTM capabilities. No live GTM container, vendor SDK environment or receiver is exercised.

Sources:

- https://docs.x.com/x-ads-api/measurement/web-conversions.md
- https://docs.x.com/x-ads-api/measurement/gtm-server-side.md
- https://raw.githubusercontent.com/twitter/x-ads-conversion-api-gtm-template/b54fdd1baac33fa2772e0081c6d5380dd85d54ab/template.tpl
