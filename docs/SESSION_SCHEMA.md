# Session relationships use caller-declared groups.

Pixellint 0.41 adds optional relationship checks around the unchanged document validator. The caller supplies original tracking URLs and explicitly declares which rows belong to an ad session. Pixellint does not infer groups from URL values, vendor names, timestamps, VAST documents, HAR pages or corpus adjacency. The first built-in session profile covers IAS Unified Video Pixel URLs on `unified.adsafeprotected.com/vevent/`.

The [IAS Video Solutions Guide](https://assets.ctfassets.net/o1orzsgogjpz/4Q8TPW9OcTv2lg35DRf3DX/7a0bd2d4564e4fea7cff97cb6137088e/IAS_Video_Solutions_Guide_.pdf), page 5, requires a populated external session identifier, with the same identity across a session's UVP URLs and a distinct identity per ad session. UUIDv4 is recommended. Pixellint accepts other nonblank opaque IDs. These checks do not establish event completeness, order, absence of retries, historical uniqueness outside this input, or destination acceptance.

## Input indexes refer to original rows.

```json
{
  "document": {
    "document_kind": "tracking_urls",
    "artifacts": [
      {"artifact": "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=a"},
      {"artifact": "https://unified.adsafeprotected.com/vevent/complete/1111111/66666666?xsId=b"},
      {"artifact": "https://unified.adsafeprotected.com/vevent/midpoint/1111111/66666666?xsId=a"}
    ]
  },
  "sessions": [
    {"session_id": "ad-1", "artifact_indexes": [0, 1]},
    {"session_id": "ad-2", "artifact_indexes": [2]}
  ]
}
```

This input proves one within-session mismatch and one reused identity across sessions. The second group's within-session comparison is not evaluated because it has only one observation. Proven findings and partial coverage can coexist.

`document` retains [MULTI_ARTIFACT_SCHEMA.md](MULTI_ARTIFACT_SCHEMA.md). The new root and group objects reject unknown or duplicate fields in raw JSON. `sessions` is required and may be empty. Group labels are opaque, case-sensitive strings. Blank labels, duplicate labels, duplicate indexes within a group, overlapping membership and out-of-range indexes are input errors. Empty groups are allowed and receive not-evaluated coverage. Ungrouped rows still receive ordinary document validation.

Membership is recorded before global document dedupe. Equal URL strings in distinct original rows can belong to different declared sessions, even when the ordinary document output contains one unique artifact. A repeated firing within one session can be represented as another row and is not itself a relationship violation. Per-row expansion state and original occurrence metadata remain available to relationship checks.

## Direct URL rows have precise query spans.

Initial relationships use direct URL-like `url`, `vast`, `postback`, `unknown` and raw-URL `request` rows. Serialized JSON/HTTP request wrappers, including raw HTTP request lines and headers, receive `unsupported_artifact` relationship skips. Their ordinary validation remains unchanged. A raw URL under `request` remains eligible for relationships even if the ordinary request-envelope checks report an error. Extraction of a wrapped URL is deliberately outside this initial coordinate contract. HTML, JavaScript and GTM snippet kinds retain the ordinary document input error.

Each participant must match its actual registered owner plugin. Explicit pack selection does not bypass this matcher. Selecting or excluding the owner also selects or excludes its relationships. Directory attribution does not establish a match. If selection leaves no ordinary plugins, the existing engine error is preserved.

Query segmentation occurs before decoding. Only literal ampersands split fields, and fragments are excluded. Parameter names and values are decoded exactly once, with literal plus converted to space. Parameter names are case-sensitive. Nonmatching malformed names or values do not suppress a valid contracted field. Matching malformed percent escapes or invalid UTF-8 receive typed skips instead of being compared through replacement characters.

Decoded identities remain case-sensitive and opaque. No Unicode normalization, recursive decoding, number parsing or clock inference occurs. Nonblank strings are not trimmed. A whitespace-only decoded value is classified as empty without modifying other identities. Percent-encoded NUL remains part of the identity. A literal plus differs from a percent-encoded plus after query decoding.

Recognized macros are unresolved in unknown, template and fired rows. Fired macro findings from ordinary validation remain intact. The IAS installation stub has its own skip reason and ordinary error. Repeated equal parameter values supply one identity and retain every original span. Conflicting repeats, or a repeat containing an empty, invalid or unresolved value, are deferred rather than choosing a value arbitrarily.

## Reports include combined counts and explicit coverage.

`SessionReport` has these fields:

- `document`: unchanged `DocumentReport`, including ordinary dedupe and counts.
- `summary`: document counts plus relationship errors, warnings and infos. Artifact totals remain the document totals.
- `relationship_summary`: relationship-only counts, without artifact total fields.
- `findings`: new relationship findings.
- `checks`: per-rule coverage, with original-row skips.
- `coverage`: aggregate status, group/member/profile totals and ungrouped indexes.

Each finding carries `plugin_id`, nullable `detected_vendor`, normal violation fields and source provenance, explicit `session_ids`, and typed original-row `targets`. A target has `artifact_index`, aggregate `artifact_id`, caller `session_id`, `query_spans`, and the original row's `occurrences`. A query span has `component: "query_param"`, exact `name`, decoded `value`, and original artifact UTF-8 byte `start` and exclusive `end`. Leading input whitespace is included in these coordinates. Equal repeated fields share one row target with multiple query spans. Occurrence IDs are never rewritten in relationship targets or skips, and absent metadata remains an empty array.

Within-session checks emit once per declared group. Across-session checks emit once per profile rule. A reused identity produces one finding across all relevant sessions, avoiding pairwise report expansion. Targets follow original row order; session labels follow caller group order.

Statuses are `evaluated`, `partially_evaluated` and `not_evaluated`. A within-session check needs two resolved rows. An across-session check needs resolved observations in two distinct groups. Below the threshold, `reason` is `insufficient_observations`. At or above it, skipped rows make coverage partial. A known mismatch or reuse can still fire when other rows are unresolved.

Skip reasons are `pack_not_selected`, `unsupported_artifact`, `invalid_url`, `endpoint_mismatch`, `missing_parameter`, `empty_parameter`, `installation_placeholder`, `unresolved_macro`, `conflicting_values`, `invalid_percent_encoding` and `invalid_utf8`. Skips do not manufacture validation findings or inflate counts. Display coverage alongside errors. A zero-error report can have no evaluated relationships.

## Interfaces are additive.

Rust uses `session_request_from_json`, `Engine::validate_sessions` and `Engine::validate_sessions_at`. The `_at` method takes an explicit Unix-seconds `i64` clock and existing `ValidationOptions`. New `SessionError` values describe grouping, resource, engine and document errors. `SessionReport::is_ok()` tests combined error counts, independently of coverage.

CLI uses `pixellint validate-sessions <inline-json|@file|-> --json --at <unix-seconds>`. Existing `--rulepack` and `--except` selection options apply. Text output includes relationship rows and coverage. Exit is 0 without errors, 1 for ordinary or relationship errors, and 2 for invalid input or evaluation errors.

WASM adds `validate_sessions(session_json, options_json?)`. Options are `{at?, rulepacks?, except_rulepacks?}`. Raw JSON clocks must be integer numeric tokens within the JavaScript safe-integer range. Explicit null, duplicates, the negative-zero spelling `-0`, decimal spellings such as `1.0`, exponent spellings such as `1e0`, fractional values and unknown option fields are rejected. Omission uses the browser clock.

npm ESM/CJS adds `validateSessions(request, {at?, rulepacks?, exceptRulepacks?})` and new TypeScript report types. `request` may be an object or raw JSON string. The wrapper checks `Number.isSafeInteger` before serialization, including NaN/Infinity rejection. Existing `isOk` continues to accept `ValidationSummary`; use `report.summary.errors === 0` for the new report while showing its coverage.

MCP adds `validate_sessions` with `request`, optional `at`, `rulepacks` and `except_rulepacks`. It returns full `structuredContent` and matching text JSON. Failed grouping or evaluation returns `isError`. The existing outer RPC parser produces an object before tool parsing, so raw duplicate-key detection applies to core/CLI/WASM/npm JSON input, while MCP preserves its established object-parser boundary.

## Custom profiles require an explicit owner registration.

`SessionRulesManifest` is separate from existing rulepack manifests and enums. It contains `owner_plugin_id`, `source_level`, `docs` and `rules`. Rules have `code`, `kind`, `param`, `severity`, `message`, optional `fix_hint` and `placeholder_values`. Supported kinds are `consistent_parameter` and `unique_parameter_across_sessions`. Codes are unique within the owner's `.session.` namespace. Unknown fields/kinds and empty required metadata are rejected. Exact placeholders compile into lookup sets.

`Engine::new()` has no built-in session profile. Register an owner, then explicitly call `register_session_manifest`, `register_session_manifest_json` or `register_session_manifest_path`. `Engine::default()` registers only the built-in IAS profile. Replacing an owner clears its old session profile, preventing implicit attachment to a different plugin that reuses an ID. Re-registration after replacement is deliberate opt-in. Existing plugin traits, manifest literal types and ordinary `rulepacks()` metadata remain unchanged.

## Local bounds cover generated provenance and text.

Input limits are 64 MiB JSON and aggregate owned text, 50,000 rows and memberships, 10,000 groups, 100,000 occurrences, 4,096 UTF-8 bytes per label, and 100,000 projected query spans. Session profile text has a 64 MiB bound. Rule/member inspection has a 2,000,000-unit bound.

Before ordinary validation, a conservative checked projection limits new session output to 1,000,000 collection items and 64 MiB of retained UTF-8 string values plus constant allowances. It counts every registered rule, including disabled rules, and repeated provenance, labels, query spans/values, owner metadata, codes, messages, fix hints and source links. Within rules project at most one finding per group; across rules project at most one finding per member. This prevents small custom inputs from multiplying large provenance or messages into unbounded reports. Arithmetic overflow also fails locally.

These are independent collection/text policies rather than a serialized JSON wire-size guarantee. They cover new session-profile output. The unchanged document validator and arbitrary output from legacy custom plugins retain their existing contract. Resource errors occur before document validation or new report allocation and are never vendor findings or clean partial reports.
