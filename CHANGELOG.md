# Changelog

All notable changes to Pixellint are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.0 - 2026-07-25

First release.

### Added

- `pixellint-core`: validation engine with rulepack plugins, stable rule ids,
  typed severities, evidence levels, documentation citations, and byte-offset
  targets on findings
- `core` rulepack with ten spec-backed and baseline rules covering URL
  validity, transport, credentials, fragments, empty input, and ad-tech macro
  handling
- Declarative rulepack manifests, with load-time validation of matchers,
  parameter contracts, regular expressions, rule codes, cross-references, and
  documentation citations
- First-party vendor packs: `vendor/meta`, `vendor/google-analytics`,
  `vendor/floodlight`, `vendor/tiktok`, `vendor/linkedin`
- Custom rulepack loading from disk via `--rulepack-file` and
  `Engine::register_manifest_path`
- `pixellint` CLI: `validate` and `list-rulepacks`, JSON output, inline, file,
  and stdin input, rulepack selection, vendor hints, and documented exit codes
- `pixellint-mcp`: MCP server over stdio exposing `list_rulepacks` and
  `validate_artifact`, with live rulepack ids in the tool schema and detected
  vendors in every response
- Golden fixture corpus with one directory per rulepack, plus integration tests
  that drive the real CLI binary and the real MCP transport
