import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { validateSessions } from "./index.mjs";
const cjs = createRequire(import.meta.url)("./index.cjs");
const request = {
  document: { document_kind: "tracking_urls", artifacts: [
    { artifact: "https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=a" },
    { artifact: "https://unified.adsafeprotected.com/vevent/complete/1111111/66666666?xsId=b" },
    { artifact: "https://unified.adsafeprotected.com/vevent/midpoint/1111111/66666666?xsId=a" },
  ] },
  sessions: [{ session_id: "ad-1", artifact_indexes: [0, 1] }, { session_id: "ad-2", artifact_indexes: [2] }],
};
test("ESM and CJS expose the authored session relationship contract", () => {
  const report = validateSessions(request, { at: 1791590400 });
  assert.deepEqual(report, cjs.validateSessions(JSON.stringify(request), { at: 1791590400 }));
  assert.deepEqual(report.findings.map((f) => f.code), [
    "vendor.ias-video-pixel.session.xsid.inconsistent",
    "vendor.ias-video-pixel.session.xsid.reused",
  ]);
  assert.equal(report.document.summary.errors, 0);
  assert.equal(report.summary.errors, 2);
  assert.equal(report.relationship_summary.errors, 2);
  assert.equal(report.coverage.status, "partially_evaluated");
  assert.deepEqual(report.findings[1].targets.map((t) => t.artifact_index), [0, 2]);
});
test("both wrappers reject unsafe clocks before JSON serialization", () => {
  for (const validate of [validateSessions, cjs.validateSessions]) {
    for (const at of [NaN, Infinity, -Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1, -Number.MAX_SAFE_INTEGER - 1, null]) {
      assert.throws(() => validate(request, { at }), /safe integer/);
    }
    assert.throws(() => validate(request, { inferSessions: true }), /Unknown session option/);
    assert.throws(() => validate(request, null), /must be an object/);
  }
});
test("pack selection leaves visible disabled relationship coverage", () => {
  const report = validateSessions(request, { at: 1791590400, rulepacks: ["core"] });
  assert.equal(report.relationship_summary.errors, 0);
  assert.equal(report.coverage.status, "not_evaluated");
  assert(report.checks.flatMap((c) => c.skipped).every((s) => s.reason === "pack_not_selected"));
});
