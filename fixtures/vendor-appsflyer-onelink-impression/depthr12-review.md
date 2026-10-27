# AppsFlyer OneLink impression URLs gains documented checks.

The shared vendor table explicitly applies af_ua to S2S clicks and impressions, requires URL and header delivery, and recommends identical values. The pack adds advisory delivery and equality checks only when a nonempty scalar af_ua establishes that profile.

The decoded URL value is compared to the captured unencoded header after standard HTTP whitespace normalization. Repeated query or header values do not establish scalar equality. Macros, missing/redacted/unavailable evidence and empty comparison values stay unverified.

No af_ua is required on C2S/browser links. This capture cannot establish the real originating device or whether an otherwise valid URL was used in a C2S integration. No User-Agent parser, Client Hints inference, af_ip provenance or forwarding-chain priority is invented.

The existing long-link URL field, window, privacy and template contracts are preserved. No new OneLink host or path support is claimed. The two HTTP fields supply comparison context and intentionally add no duplicate blank-value diagnostics.

This round adds 27 golden cases. Primary sources:

- https://support.appsflyer.com/hc/en-us/articles/207447163-About-link-structure-and-parameters (direct retrieval blocked)
- https://support.appsflyer.com/hc/en-us/articles/207447163-About-link-structure-and-parameters
