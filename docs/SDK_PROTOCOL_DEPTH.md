# SDK protocol checks follow a pinned source profile.

FTrack's legacy `/lgc` pack compares `D9_33` with `D9_34` under the public d9core indexed fingerprint profile. The decoder retains Latin-1 bytes from browser `btoa`, removes only indexed slot prefixes, preserves embedded delimiters, and computes unsigned MurmurHash3 with seed 31. Android may omit `e` and `h`; iPhone and iPad omit `f`. Multiple possible slot boundaries, unknown layouts and the local decoder resource bound produce an information finding instead of a guessed hash mismatch.

The two reviewed [d9core](https://d9.flashtalking.com/d9core) snapshots have SHA-256 `77c8c6a48bdfa3444ff0128161ab436693e3ae6be5409e269e873b743baf8bd6` and `85e48b46c5da3f5e3a0e86763880dc197b9af2a016514e837ab11c76c7702ccb`. Their fingerprint algorithms are identical. The only source change is the opaque `D9_61` literal. That literal does not establish an SDK or server version. The collector envelope has no authoritative producer revision marker, so parity remains advice for this source profile. The SDK collects indexed and unindexed inputs separately; changing browser state can also produce a mismatch. No server rejection policy is inferred.

The pack explicitly declares `http_headerless_form`. An active pack matching its endpoint can decode a form entity when `Content-Type` is observed absent. A present empty or text MIME header does not invoke this hint. Capture metadata marking MIME, compression or body data unknown or redacted blocks inference. Compressed entities remain unvalidated. The official XDomainRequest branch emits the same form body without setting a MIME header; its inspectable `tbx` fields receive the existing device contracts.

Matomo's existing PHP8 bulk profile now models a bounded scalar `parse_str` representation before applying field readers. It decodes one form layer, normalizes spaces and dots in scalar keys, and lets later assignments shadow earlier values after normalization. Numeric reader defaults come from the pinned [Request table](https://raw.githubusercontent.com/matomo-org/matomo/1e9169ddd9eba7c982dc67b8bd3f9310a7a312c6/core/Tracker/Request.php) and [Common implementation](https://raw.githubusercontent.com/matomo-org/matomo/1e9169ddd9eba7c982dc67b8bd3f9310a7a312c6/core/Common.php). Macros retain their original values so conversion cannot manufacture an event error before expansion.

Native float handling follows `Common`'s validation and cast separately. `1,5` validates as a comma-normalized number but PHP casts the original string to `1`; `-,5` becomes `0`; `1,2,3` uses the numeric default. Prefixes outside portable PHP32/PHP64 integers and unsupported containers remain unvalidated. These distinctions are proved by executing the pinned source in official PHP 8.3.35, with a 64-bit integer runtime, precision 14, `arg_separator.input=&`, and `max_input_vars=1000`.

Full PHP equivalence remains outside this profile. Bracket trees, sanitizer-sensitive input, non-UTF-8 query bytes, raw semicolon separator candidates and inputs exceeding the pinned default variable limit produce an information finding. Observable transport checks still run. Other deployed PHP, Matomo and plugin versions, runtime limits and server state require their own context.

The new evidence directories contain exact expected reports, source-produced captures and reproducible oracle scripts:

- `fixtures/vendor-flashtalking-ftrack-depth`: 30 actual SDK captures, two labeled producer mutations and 12 independently authored boundaries.
- `fixtures/vendor-matomo-php-scalar-depth`: 27 PHP query oracles, 27 native float oracles and five independently authored controls.

The two existing FTrack fixtures with `D9_34=0` and `D9_34=4294967295` retain their valid uint32 syntax. Their unchanged indexed fingerprint belongs to a different hash, so they now expect a producer mismatch warning. No existing Matomo fixture expectations changed.
