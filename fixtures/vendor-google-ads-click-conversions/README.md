# Google Ads click conversion upload receives deeper service-specific checks.

Reject service-inapplicable personalization consent while preserving supported user-data consent. Add canonical resource spelling and current or historical currency assignment advice.

This round adds 109 regression cases. All 14 prior golden expectations are unchanged. One supplementary numeric-enum example drops an unsupported personalization field; its original payload is retained as a new negative regression.

- The current Consent reference limits ad_personalization to OfflineUserDataJobService and UserDataService. These two services cannot set a non-default value. Implicit zero defaults and null-unset semantics are retained.
- Resource-name checks provide canonical spelling advice. They do not check account existence, resource ownership, conversion-action eligibility, numeric range or equality with the upload customer. The conversion reference says the customer component is ignored for action lookup.
- Currency assignment is advisory and accepts current and historical codes without forcing case. Both built-in code sets were independently compared with freshly fetched primary SIX XML. Account eligibility and actual Google currency support remain external.
- Existing ProtoJSON aliases, null handling, timestamp, value and identity checks are retained. Simultaneous duplicate aliases and raw duplicate keys retain the existing parser behavior; this round does not model receiver selection among duplicates.
- Host, path and bare-JSON selectors, the directory and Rust engine source are unchanged. No new stored-sample reach or live Google acceptance is claimed.

Sources read:

- https://developers.google.com/google-ads/api/reference/rpc/v25/Consent
- https://developers.google.com/google-ads/api/reference/rpc/v25/ConversionAction
- https://developers.google.com/google-ads/api/reference/rpc/v25/ClickConversion
- https://protobuf.dev/programming-guides/json/
- https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-one.xml
- https://www.six-group.com/dam/download/financial-information/data-center/iso-currrency/lists/list-three.xml

The primary proto3 enum has implicit presence. UNSPECIFIED and numeric zero are allowed defaults. Non-default values receive the service-specific error.
