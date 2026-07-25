# Fixture corpus

One directory per rulepack. `core/` holds artifacts for the built-in core pack;
each `vendor-*/` directory holds artifacts for the matching `vendor/*` pack.

Every directory has a `manifest.json` listing its cases:

```json
{
  "id": "missing-pixel-id",
  "kind": "url",
  "fixture": "missing-pixel-id.txt",
  "expected_plugins": ["core", "vendor/meta"],
  "expected_ok": false,
  "expected_errors": 1,
  "expected_warnings": 0,
  "expected_infos": 0,
  "expected_codes": ["vendor.meta.param.id.missing"]
}
```

Optional per-case fields: `expansion_state` (`unknown`, `template`, `fired`),
`claimed_vendor`, `rulepacks`, and `except_rulepacks`. The last two force pack
selection the way `--rulepack` and `--except` do on the CLI.

`expected_codes` is order-sensitive and lists findings across every rulepack
that ran, in report order. `core` sorts before any `vendor/*` pack.

`crates/pixellint-core/tests/rulepack_golden.rs` runs the whole corpus, and a
companion test fails if a built-in rulepack ships without a fixture directory.

To add a case: write the artifact as a `.txt` file, run

```bash
cargo run -p pixellint -- validate url @fixtures/<dir>/<file>.txt --json
```

and add the entry to that directory's `manifest.json` after checking the output
is what the rule should say.
