# Core Golden Corpus

This directory defines the golden fixture corpus for the `core` Pixellint rulepack.

- `manifest.json` is the source of truth for expected outcomes.
- `*.txt` files are the concrete artifacts validated by the test suite.
- `crates/pixellint-core/tests/core_rulepack_golden.rs` loads this corpus and asserts exact violation codes and severity counts.

The corpus currently covers:

- clean pixel URLs
- clean postbacks
- clean VAST tracking URLs
- trimmed URL inputs from files or copied snippets
- localhost and IPv6 endpoints used in QA and staging
- insecure HTTP transport warnings
- combined warning cases on realistic tracker URLs
- empty input
- invalid URLs
- unsupported schemes
- missing hosts
- missing-host edge variants such as query-only and port-only inputs
- embedded credentials
- ignored fragments
- combined warning cases
- template URLs that still contain ad-tech macros
- fired URLs with unresolved macros
- mixed macro syntax and unsafe macro placement in host components