"""Check that every shipped vendor manifest has a current source review."""

import hashlib
import json
from functools import cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
AUDIT = ROOT / "docs/RULEPACK_DEPTH_AUDIT.json"


@cache
def directory_names(directory):
    return {entry.name for entry in directory.iterdir()}


def has_exact_case(relative_path):
    directory = ROOT
    for name in Path(relative_path).parts:
        if name not in directory_names(directory):
            return False
        directory /= name
    return True


def main():
    review = json.loads(AUDIT.read_text())
    entries = {entry["id"]: entry for entry in review["packs"]}
    if len(entries) != len(review["packs"]):
        raise SystemExit("Duplicate pack reviews.")
    shipped = {}
    for path in sorted((ROOT / "crates/pixellint-core/rulepacks/vendor").glob("*.json")):
        manifest = json.loads(path.read_text())
        shipped[manifest["id"]] = path
    if set(entries) != set(shipped):
        raise SystemExit(f"Review inventory differs: {sorted(set(entries) ^ set(shipped))}")
    failures = []
    for pack_id, path in shipped.items():
        entry = entries[pack_id]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if entry.get("manifest_sha256") != digest:
            failures.append(f"{pack_id}: manifest changed after source review")
        artifacts = entry.get("reviewed_artifacts", {})
        if not artifacts:
            failures.append(f"{pack_id}: missing reviewed fixture artifacts")
        for relative_path, expected in artifacts.items():
            artifact = (ROOT / relative_path).resolve()
            if not artifact.is_relative_to(ROOT) or not artifact.is_file():
                failures.append(f"{pack_id}: reviewed artifact missing: {relative_path}")
            elif not has_exact_case(relative_path):
                failures.append(f"{pack_id}: reviewed artifact filename case differs: {relative_path}")
            elif hashlib.sha256(artifact.read_bytes()).hexdigest() != expected:
                failures.append(f"{pack_id}: artifact changed after review: {relative_path}")
        sources = (
            entry.get("sources_read")
            or entry.get("sources")
            or entry.get("source_access_attempts")
        )
        if not sources:
            failures.append(f"{pack_id}: missing source-access inventory")
        if "spec_complete" not in entry:
            failures.append(f"{pack_id}: missing explicit completeness status")
        if entry.get("spec_complete") and entry.get("missing_requirements"):
            failures.append(f"{pack_id}: completeness conflicts with remaining requirements")
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"{len(shipped)} vendor packs have current source reviews. "
          "Review freshness does not certify complete vendor coverage.")


if __name__ == "__main__":
    main()
