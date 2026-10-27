# The Google Analytics 4 Measurement Protocol profile has a bounded source contract.

Documented minimum location recommendations and correction of checks on an ignored IP override.

This round adds 73 regression cases. All 23 prior fixture expectations are retained.

- Google recommends country_id and region_id when a user_location object is supplied. Their absence is warning-level advice; existing supplied-value checks retain their severity.
- A structured user_location object suppresses ip_override validation because Google documents that the override is ignored. A malformed non-object location retains the independent IP check.
- This does not establish actual country/city membership, US city classification, configured geographic collection, location accuracy or receiver acceptance.

Sources read:

- https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference
