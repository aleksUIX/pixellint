# Standards

This document lists the hard standards Pixellint currently implements in the `core` rulepack.

Scope note:

- This is the normative baseline implemented today.
- It does not include planned vendor rulepacks.
- It does not include heuristic-only checks except where explicitly called out.

## Core Rulepack

`core` currently applies to URL-like measurement artifacts:

- `url`
- `vast`
- `postback`

The implementation lives in `crates/pixellint-core/src/lib.rs`.

## Normative Standards Implemented Today

| Standard | What Pixellint enforces today | Rule IDs |
| --- | --- | --- |
| [WHATWG URL Standard](https://url.spec.whatwg.org/) | URL must parse as a valid absolute URL and include a host for network-delivered artifacts | `core.url.invalid`, `core.url.host_missing` |
| [W3C Beacon](https://www.w3.org/TR/beacon/) transport baseline | Network-delivered tracking artifacts must use `http` or `https` transport | `core.url.unsupported_scheme` |
| [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986) URI Generic Syntax | Embedded credentials in tracker URLs are deprecated and URL fragments do not reach the server | `core.url.userinfo_deprecated`, `core.url.fragment_ignored` |

## Heuristic Baseline

The `core` rulepack also includes one non-normative input guard:

| Check | What Pixellint enforces today | Rule ID |
| --- | --- | --- |
| Input baseline | Empty artifacts are rejected before URL-like validation runs | `core.input.empty` |

This rule is intentionally heuristic, not a standards citation.

## Best-Practice Baseline

The `core` rulepack also carries one ecosystem best-practice warning for network-delivered measurement artifacts:

| Check | What Pixellint enforces today | Rule ID |
| --- | --- | --- |
| Secure transport | Plain `http` tracking endpoints are flagged so callers can upgrade them to `https` for secure playback and delivery environments | `core.url.insecure_transport` |

## Template Baseline

The `core` rulepack also includes a generic macro-template baseline for ad-tech URLs:

| Check | What Pixellint enforces today | Rule ID |
| --- | --- | --- |
| Unexpanded macros in fired URLs | A fired or observed URL should not still contain unresolved template macros | `core.macro.unexpanded_in_fired_url` |
| Unsafe macro placement | Macros in scheme, authority, host, port, or userinfo are rejected because they prevent reliable endpoint resolution | `core.macro.unsafe_position` |
| Mixed macro syntax | Multiple macro syntaxes in one artifact are warned because they make trafficking behavior harder to predict | `core.macro.mixed_syntax` |

## Evidence

The current standards baseline is backed by two test layers:

- rule-level unit tests in `crates/pixellint-core/src/lib.rs`
- a golden fixture corpus in `fixtures/core/`, enforced by `crates/pixellint-core/tests/core_rulepack_golden.rs`

The golden corpus currently covers:

- clean HTTPS pixel URL
- clean HTTPS postback URL
- clean HTTPS VAST tracking URL
- insecure HTTP transport
- empty input
- invalid URL
- unsupported scheme
- missing host
- embedded credentials
- ignored fragment
- combined warning cases
- macro templates in query values
- unresolved macros in fired URLs
- mixed macro syntax and unsafe macro placement

## Not Implemented Yet

The following categories are intentionally out of scope for the current `core` rulepack:

- vendor-specific parameter contracts
- privacy and consent framework enforcement
- macro correctness
- duplicate detection across documents
- document extraction and search
- VAST XML semantics beyond passing extracted tracker URLs into Pixellint

Those belong in future vendor rulepacks or in document-specific callers such as Vastlint.