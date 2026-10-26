# Customer.io Track v1 events uses a bounded source profile.

Customer.io Track v1 events endpoint and locally checkable payload contracts.

36 fixtures cover source examples, native alternatives, mutations and scope controls. A fixture count is not a percentage of vendor specification coverage.

- Track v1 event routes only. Track v2 entity/batch, Pipelines, identification and account APIs are not selected.
- Generic id/name JSON shapes do not identify Customer.io. Automatic selection requires an endpoint capture or URL; body contracts run on complete captures.
- Compact data JSON exceeding 100000 bytes is provably oversized. Whitespace-sensitive receiver accounting and final stored size are not fully modeled.
- The OpenAPI screen branch lists anonymous_id as required on an identified route while prose and other branches do not. No universal anonymous-ID requirement is imposed.
- Unknown event types default to event according to prose. No closed type enum is imposed. Historical timestamps are valid input and do not imply automation eligibility.
- Workspace identity policy, old anonymous-name trimming behavior, deduplication history, credential validity and automation settings require external context.
