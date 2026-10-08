"""Generate the implemented vendor inventory from the shipped manifests."""

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / "docs/STANDARDS.md"
START = "<!-- BEGIN GENERATED VENDOR CONTRACTS -->"
END = "<!-- END GENERATED VENDOR CONTRACTS -->"


def cell(value):
    if not isinstance(value, str):
        value = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    return value.replace("|", "\\|").replace("\n", " ")


def contract_table(params, default_source, default_doc):
    if not params:
        return []
    lines = [
        "| Field | Requirement | Implemented checks | Authority |",
        "| --- | --- | --- | --- |",
    ]
    omitted = {"name", "requirement", "doc", "description", "fix_hint", "source_level"}
    for param in params:
        checks = {key: value for key, value in param.items() if key not in omitted}
        source = param.get("source_level", default_source)
        doc = param.get("doc", default_doc)
        authority = f"[{source}]({doc})" if doc else source
        lines.append(
            f"| `{cell(param['name'] or '(current scope)')}` | "
            f"{param.get('requirement', 'optional')} | `{cell(checks)}` | {authority} |"
        )
    return lines + [""]


def rule_table(rules, default_source, default_doc):
    if not rules:
        return []
    lines = [
        "| Rule | Assertion and condition | Severity | Authority |",
        "| --- | --- | --- | --- |",
    ]
    omitted = {"code", "severity", "message", "fix_hint", "doc", "source_level"}
    for rule in rules:
        assertion = {key: value for key, value in rule.items() if key not in omitted}
        source = rule.get("source_level", default_source)
        doc = rule.get("doc", default_doc)
        authority = f"[{source}]({doc})" if doc else source
        lines.append(
            f"| `{cell(rule['code'])}` | `{cell(assertion)}` | "
            f"{rule['severity']} | {authority} |"
        )
    return lines + [""]


def generate():
    packs = [json.loads(path.read_text()) for path in sorted(
        (ROOT / "crates/pixellint-core/rulepacks/vendor").glob("*.json")
    )]
    lines = [START, "", "## The vendor inventory records implemented contracts.", "",
             f"This generated inventory covers {len(packs)} shipped vendor packs. "
             "It does not certify complete vendor specification coverage. "
             "The per-pack source review and remaining requirements are recorded in "
             "[RULEPACK_DEPTH_AUDIT.json](RULEPACK_DEPTH_AUDIT.json).", "",
             "Regenerate with `python3 tools/update-rulepack-reference.py`. "
             "Use `--check` to detect stale inventory.", ""]
    for pack in packs:
        source = pack.get("source_level", "official_vendor")
        doc = pack.get("docs")
        lines += [f"## `{pack['id']}`", "", pack["description"], "",
                  f"Manifest: [source](../crates/pixellint-core/rulepacks/{pack['id']}.json).", "",
                  f"Matcher: `{cell(pack['match'])}`.", ""]
        if "path_pattern" in pack:
            lines += [f"Path captures: `{cell(pack['path_pattern'])}`. "
                      "Captured values take precedence over query keys with the same name.", ""]
        if pack.get("client_fragment_params"):
            lines += [f"Browser fragment keys: `{cell(pack['client_fragment_params'])}`.", ""]
        if pack.get("gdpr_non_applicable_values"):
            lines += [f"Documented non-applicable GDPR values: `{cell(pack['gdpr_non_applicable_values'])}`. "
                      "Core accepts these only for a matching selected endpoint.", ""]
        if pack.get("params") or pack.get("rules"):
            lines += ["### URL parameters", ""]
            lines += contract_table(pack.get("params", []), source, doc)
            lines += rule_table(pack.get("rules", []), source, doc)
        for index, query in enumerate(pack.get("query_scopes", []), 1):
            context = {key: value for key, value in query.items() if key not in {"params", "rules"}}
            lines += [f"### Query group contract {index}", "", f"Context: `{cell(context)}`.", ""]
            lines += contract_table(query.get("params", []), source, doc)
            lines += rule_table(query.get("rules", []), source, doc)
        bodies = pack.get("body", [])
        if isinstance(bodies, dict):
            bodies = [bodies]
        for index, body in enumerate(bodies, 1):
            context = {key: value for key, value in body.items() if key not in {"params", "rules"}}
            lines += [f"### JSON contract {index}", "", f"Context: `{cell(context)}`.", ""]
            lines += contract_table(body.get("params", []), source, doc)
            lines += rule_table(body.get("rules", []), source, doc)
    return "\n".join(lines + [END, ""])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    existing = REFERENCE.read_text()
    prefix = existing.split(START, 1)[0] if START in existing else existing.split("## `vendor/", 1)[0]
    prefix = prefix.replace(
        "Every rule Pixellint ships, what it enforces, and where its authority comes\n"
        "from. Nothing here is aspirational: if a rule is listed, it is implemented and\n"
        "covered by a fixture.",
        "The core checks and implemented vendor contracts, with their stated authority.\n"
        "The vendor inventory is generated from manifests. Specification coverage and\n"
        "remaining requirements are reviewed separately in RULEPACK_DEPTH_AUDIT.json.",
    )
    result = prefix.rstrip() + "\n\n" + generate()
    if args.check:
        if existing != result:
            raise SystemExit("Vendor reference is stale. Run tools/update-rulepack-reference.py.")
    else:
        REFERENCE.write_text(result)


if __name__ == "__main__":
    main()
