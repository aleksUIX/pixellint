# Roadmap

What Pixellint does today and where it is heading. Not a promise of dates.

## Shipped

- Rust core with a plugin engine, stable rule ids, typed severities, evidence
  levels, documentation citations, and byte-offset targets on findings
- `core` rulepack: URL validity and host presence, transport scheme, embedded
  credentials, ignored fragments, insecure transport, generic ad-tech macro
  handling, and IAB consent strings decoded against TCF v2, US Privacy, and GPP
- Declarative rulepack manifests: host and path matchers, parameter contracts,
  JSON body contracts, value formats, cross-parameter rules, and load-time
  validation including a citation requirement for vendor-documented rules
- Thirty-five first-party vendor packs: Meta Pixel and Conversions API, GA4
  Measurement Protocol and the browser `/g/collect` transport, Google Tag
  Manager and gtag.js, Google Ads conversion pixels and click conversion
  uploads, Campaign Manager Floodlight, Adobe Analytics, Pinterest Tag and
  Conversions API, Snapchat Conversions API, TikTok Pixel and Events API,
  LinkedIn image pixel and Conversions API, Microsoft UET, Reddit Pixel and
  Conversions API, X conversion API, Yahoo Dot, Yandex Metrica Measurement
  Protocol, OpenAI Ads image tag and Conversions API, Amplitude, Mixpanel,
  PostHog, Segment, Klaviyo, Braze, AppsFlyer S2S, Adjust S2S, Branch Events
  API, Kochava S2S, and Singular S2S EVENT
- Vendor endpoint directory: hosts attributed by vendor, so endpoints without a
  matching pack still get identified, and vendors that do have a pack are
  marked as covered
- Parameters carried on the path, so endpoints that put their identifier in a
  path segment can be contracted like any other
- Custom rulepacks from disk, the same format the first-party packs use
- CLI with JSON output, stdin and file input, rulepack selection, and CI-ready
  exit codes
- MCP server over stdio with `validate_artifact`, `list_rulepacks`, and
  `list_vendors`
- WASM bindings, an npm package, and the playground at
  [pixellint.org](https://pixellint.org)
- GitHub Action in this repo (`uses: aleksUIX/pixellint@<tag>`). Prebuilt musl
  / macOS CLI tarballs on GitHub Releases

## Next

- Promote directory entries to full rulepacks as parameter contracts surface:
  Criteo, Taboola, Outbrain, Amazon Ads, Snap Pixel, the X website tag, Baidu
  Tongji, and Kwai stay attributed until a citable HTTP contract exists
- Community-contributed directory entries and corrections
- Document-level results: the wrapper described in
  [docs/MULTI_ARTIFACT_SCHEMA.md](docs/MULTI_ARTIFACT_SCHEMA.md), so callers
  that extract many artifacts get one coherent report
- Deterministic autofix for the mechanical findings, behind an explicit flag
- Duplicate and conflict detection across a set of artifacts

## Later

- Vendor packs contributed and maintained by the vendors themselves
- Pre-commit hook, Homebrew tap, Docker image
- Editor integration that lints tracking URLs in place
- Network mode: replay a HAR or captured request stream and lint what fired
- Python and Go bindings over the Rust core

## Non-goals

- Firing requests to check whether an endpoint responds
- Enforcing a vendor's business policy beyond what it documents
- Redistributing vendor documentation text; rulepacks carry structured
  metadata and links, not prose
- Owning every vendor forever. First-party packs cover the common families;
  the long tail belongs in custom packs
