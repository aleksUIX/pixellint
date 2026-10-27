# TikTok Events API receives captured-request checks.

Legacy TikTok pixel track and batch profiles gain captured transport and route-specific envelope advice. Current official SDK v1.3 routes and the published v1.2 track profile are scoped separately from Events API 2.0.

This round adds 78 golden fixtures. All 52 prior native expectations are unchanged.

- The pinned official EventCallback SDK explicitly requires access_token and writes Access-Token even though its generated Authorization section says No authorization required. POST, JSON MIME and token presence are producer-profile warnings, without claiming universal receiver enforcement.
- The SDK body argument is optional even though its body model has required fields. Absent entities therefore receive advice. Track-route objects require a root event recommendation; batch-route objects require batch. Existing submitted body types and min-item checks are retained without duplicate route-level type checks.
- Owned routes are official SDK v1.3 track/batch and the primary indexed v1.2 track example, with one optional trailing slash. Other versions, v1.2 batch, suffixes and prefixed routes remain uncontracted by this pack. Events API 2.0 is a separate owner.
- Ten controls execute the actual pinned SDK class with intercepted transport: eight emitted captures and two missing-token exceptions. six.iteritems is a compatibility shim. No vendor network, real token, account or receiver is exercised.
- The directly fetched legacy guide remains a JavaScript shell. SDK models establish event/batch field presence but not exhaustive receiver limits. Bare JSON keeps its existing discriminator and cannot establish a track-versus-batch URL.
- Five pre-existing forced-selection fixtures across the two packs have no automatic vendor discriminator. Native tests retain forced selection and unchanged expectations. The public npm wrappers receive explicit core-only auto-mode controls for these inputs; no forced public validation capability is claimed.

Primary sources retrieved:

- https://ads.tiktok.com/gateway/docs/index?doc_id=1724255493685249&identify_key=2b9b4278e47b275f36e7c39a4af4ba067d088e031d5f5fe45d381559ac89ba48&language=ENGLISH (direct shell only; primary search index read separately)
- https://raw.githubusercontent.com/tiktok/tiktok-business-api-sdk/f809c396520df2d7b201a9ccc5378d822b728ed3/python_sdk/docs/PixelTrackBody.md
- https://raw.githubusercontent.com/tiktok/tiktok-business-api-sdk/f809c396520df2d7b201a9ccc5378d822b728ed3/python_sdk/docs/PixelBatchBody.md
- https://raw.githubusercontent.com/tiktok/tiktok-business-api-sdk/f809c396520df2d7b201a9ccc5378d822b728ed3/python_sdk/business_api_client/models/pixel_track_body.py
- https://raw.githubusercontent.com/tiktok/tiktok-business-api-sdk/f809c396520df2d7b201a9ccc5378d822b728ed3/python_sdk/business_api_client/models/pixel_batch_body.py
- https://raw.githubusercontent.com/tiktok/tiktok-business-api-sdk/f809c396520df2d7b201a9ccc5378d822b728ed3/python_sdk/business_api_client/api/event_callback_api.py
- https://raw.githubusercontent.com/tiktok/tiktok-business-api-sdk/f809c396520df2d7b201a9ccc5378d822b728ed3/python_sdk/docs/EventCallbackApi.md
