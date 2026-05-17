# Multi-Artifact Output Schema

This document defines the recommended output shape for validating documents that contain multiple pixel URLs or other measurement artifacts.

The goal is to preserve Pixellint's current deterministic single-artifact core while giving callers a stable document-level result model.

## Design Decision

Pixellint core stays single-artifact.

Document-specific callers perform:

- extraction
- normalization
- deduplication
- provenance tracking
- aggregation

That means:

- Pixellint validates extracted artifacts.
- The caller explains where each artifact came from.

## Why This Split Exists

Different document formats need different extractors:

- VAST needs XML-aware extraction with XPath or node provenance.
- HTML needs DOM-aware extraction.
- JavaScript snippets may need parser-aware extraction.
- GTM templates need template-aware extraction.

Trying to bake all of that into `pixellint-core` would mix grammar-specific search with artifact validation and make rulepacks harder to keep deterministic.

## Compatibility Goal

The current single-artifact `ValidationSummary` remains valid.

Multi-artifact validation should wrap, not replace, the current `reports` shape.

## Recommended Top-Level Shape

```json
{
  "document_kind": "vast",
  "extractor": {
    "id": "vastlint",
    "version": "0.4.16"
  },
  "summary": {
    "artifacts_total": 7,
    "unique_artifacts": 5,
    "errors": 2,
    "warnings": 4,
    "infos": 0
  },
  "artifacts": []
}
```

## Artifact Result Shape

Each unique extracted artifact should produce one aggregated artifact result.

```json
{
  "artifact_id": "artifact-3",
  "dedupe_key": "sha256:...",
  "artifact_kind": "url",
  "raw_artifact": "https://example.com/pixel?id=1#frag",
  "normalized_artifact": "https://example.com/pixel?id=1#frag",
  "ok": true,
  "summary": {
    "errors": 0,
    "warnings": 1,
    "infos": 0
  },
  "reports": [],
  "occurrences": []
}
```

Recommended fields:

- `artifact_id`: stable ID within the response
- `dedupe_key`: how identical artifacts are coalesced
- `artifact_kind`: `url`, `vast`, `postback`, etc.
- `raw_artifact`: original extracted value
- `normalized_artifact`: optional normalized value used for dedupe or downstream comparison
- `ok`: whether any error-severity finding exists for this artifact
- `summary`: per-artifact counts
- `reports`: current Pixellint `ValidationReport[]`
- `occurrences`: every place the artifact appeared in the source document

## Occurrence Shape

Multiple identical pixels should not be validated repeatedly, but every occurrence should still be reported.

```json
{
  "occurrence_id": "occ-7",
  "source_kind": "xpath",
  "path": "/VAST/Ad[1]/InLine/Impression[2]",
  "line": 42,
  "column": 7,
  "context_label": "second inline impression"
}
```

Recommended fields:

- `occurrence_id`: stable ID within the response
- `source_kind`: `xpath`, `css-selector`, `line-column`, `json-pointer`, etc.
- `path`: location in the source document
- `line` / `column`: optional editor-friendly location
- `context_label`: optional human-readable description

## Per-Finding Targeting

Yes: Pixellint should highlight the problematic part of each pixel, not just the whole pixel.

The current `Violation` type now supports a `targets` array in addition to the coarse `field` string.

Current target shape:

```json
{
  "component": "query_param",
  "name": "gpp",
  "value": "DBAB...",
  "start": 31,
  "end": 57
}
```

Recommended component kinds:

- `whole_url`
- `scheme`
- `host`
- `path`
- `userinfo`
- `query_param`
- `fragment`

For each violation, `targets` lets Pixellint or its caller explain exactly what is wrong and where it appears.

## Who Should Fire The Check

The caller that already understands the document should fire the multi-artifact check.

Examples:

- Vastlint should extract VAST tracking URLs and call Pixellint.
- An HTML adapter should extract pixel image URLs or tracking snippet URLs and call Pixellint.
- A GTM adapter should extract relevant network endpoints and call Pixellint.

Pixellint itself should stay neutral about extraction in the core.

## Recommended API Evolution

1. Keep `validate(request)` unchanged.
2. Add a future `validate_many(requests)` wrapper at the library or surface layer.
3. Add a future `validate_document` MCP tool only after the extractor contract is stable.

## Recommended MCP / CLI Behavior

For document-level validation, future surfaces should return:

- one document summary
- one aggregated artifact result per unique artifact
- one occurrence list per artifact
- one per-finding target list when available

That gives both machine-readable dedupe and human-readable debugging.

## Minimal Example

```json
{
  "document_kind": "vast",
  "summary": {
    "artifacts_total": 3,
    "unique_artifacts": 2,
    "errors": 1,
    "warnings": 1,
    "infos": 0
  },
  "artifacts": [
    {
      "artifact_id": "artifact-1",
      "artifact_kind": "url",
      "raw_artifact": "https://example.com/pixel?id=1#frag",
      "ok": true,
      "summary": { "errors": 0, "warnings": 1, "infos": 0 },
      "reports": [
        {
          "plugin_id": "core",
          "detected_vendor": null,
          "violations": [
            {
              "code": "core.url.fragment_ignored",
              "message": "URL fragments are not transmitted to the server and cannot carry measurement parameters.",
              "severity": "Warning",
              "field": "url",
              "fix_hint": "Move tracking data into the query string or request body.",
              "source": {
                "level": "Normative",
                "name": "RFC 3986 URI generic syntax",
                "reference": "https://www.rfc-editor.org/rfc/rfc3986"
              }
            }
          ]
        }
      ],
      "occurrences": [
        {
          "occurrence_id": "occ-1",
          "source_kind": "xpath",
          "path": "/VAST/Ad[1]/InLine/Impression[1]"
        },
        {
          "occurrence_id": "occ-2",
          "source_kind": "xpath",
          "path": "/VAST/Ad[1]/InLine/Tracking[3]"
        }
      ]
    }
  ]
}
```

## Recommendation

Adopt this schema design before building parameter-heavy vendor rulepacks.

Without per-artifact provenance and per-finding component targeting, future rules will be hard to debug and hard to trust in multi-pixel documents.