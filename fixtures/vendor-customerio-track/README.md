# Customer.io Track v1 events receives source-scoped checks.

Customer.io Track v1 event captures. Check event fields and deduplication identifiers while preserving official numeric anonymous IDs, optional identity and legacy invites.

This round adds 70 golden fixtures. 2 prior severity expectations are corrected; other prior expectations are unchanged.

- The current event guide says nonuppercase or malformed string ULIDs are replaced with new receiver-generated IDs. Format findings are warnings about lost deduplication. Native nonstring IDs keep their existing type errors; this round does not claim that the receiver accepts every type.
- The pinned official Node sender accepts finite string or number anonymous identifiers and forwards populated values unchanged. OpenAPI describes a string-only branch. Both published producer alternatives are retained without inventing receiver coercion.
- Optional and empty anonymous IDs, omitted Ruby nil invites, custom event types and historical timestamps retain their behavior. The current anonymous OpenAPI branch only requires name, so a universal anonymous-ID requirement would be incorrect.
- Ten actual pinned SDK calls are run with intercepted Request transport and actual utility/region/type modules. No vendor network, account, live SDK container or receiver is exercised.
- Route and host selection are unchanged. Generic bare JSON does not identify this vendor. Deduplication history, identity linkage, automation settings, credential validity and receiver behavior beyond the published contract remain external.

Primary sources read:

- https://docs.customer.io/messaging/people/events/
- https://docs.customer.io/files/journeys-track.json
- https://docs.customer.io/integrations/data-in/connections/classic-api/invalid-api-requests/
- https://raw.githubusercontent.com/customerio/customerio-node/4b9a980b790b937aea2a68d7d11d534c204edd13/docs/track.md
- https://raw.githubusercontent.com/customerio/customerio-ruby/57799ded05f69ba0d12f39b17439ab109a8ec6c0/lib/customerio/client.rb
- https://raw.githubusercontent.com/customerio/customerio-node/4b9a980b790b937aea2a68d7d11d534c204edd13/lib/track.ts
- https://raw.githubusercontent.com/customerio/customerio-node/4b9a980b790b937aea2a68d7d11d534c204edd13/lib/utils.ts
- https://raw.githubusercontent.com/customerio/customerio-node/4b9a980b790b937aea2a68d7d11d534c204edd13/lib/regions.ts
- https://raw.githubusercontent.com/customerio/customerio-node/4b9a980b790b937aea2a68d7d11d534c204edd13/lib/types.ts
