/** Independent public API checks. Run after rebuilding this version's WASM. */
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { importHar, validateHar } from "./index.mjs";

const require = createRequire(import.meta.url);
const cjs = require("./index.cjs");
const fixture = (name) => readFileSync(new URL(`../crates/pixellint-core/tests/data/har/${name}.har`, import.meta.url), "utf8");
const codes = (report) => report.artifacts.flatMap((artifact) => artifact.reports.flatMap((pack) => pack.violations.map((finding) => finding.code)));
const clean = validateHar(fixture("known-clean"), { headerPolicy: "complete" });
assert.equal(clean.summary.artifacts_total, 2);
assert.equal(clean.summary.unique_artifacts, 1);
assert.equal(clean.summary.errors, 0);
assert.equal(clean.reference_time_override, null);
assert.equal(clean.artifacts[0].occurrences.length, 2);
assert.deepEqual(codes(clean), []);
assert.deepEqual(cjs.validateHar(fixture("known-clean"), { headerPolicy: "complete" }), clean);

for (const name of ["credential-omitted", "body-unavailable", "params-only"]) {
  const report = validateHar(fixture(name));
  assert.equal(report.summary.errors, 0, name);
  assert.deepEqual(codes(report), ["core.request.capture_incomplete"], name);
}
const complete = validateHar(fixture("credential-omitted"), { headerPolicy: "complete" });
assert.deepEqual(codes(complete), ["vendor.meta-conversions-api.http.authentication_required"]);
const imported = importHar(fixture("credential-omitted"), { headerPolicy: "chrome_sanitized" });
assert.equal(imported.document.document_kind, "har");
assert.deepEqual(imported.entries[0].capture.redacted_headers, ["authorization", "cookie"]);
assert.deepEqual(cjs.importHar(fixture("credential-omitted"), { headerPolicy: "chrome_sanitized" }), imported);

const missingClock = JSON.parse(fixture("known-clean"));
for (const entry of missingClock.log.entries) delete entry.startedDateTime;
assert.throws(() => validateHar(missingClock), /explicit reference-time override/);
const overridden = validateHar(missingClock, { at: 1770000060 });
assert.equal(overridden.summary.errors, 0);
assert.equal(overridden.reference_time_override, 1770000060);
for (const at of [NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
  assert.throws(() => validateHar(missingClock, { at }), /safe integer/);
}
assert.throws(() => importHar("{}"), /invalid HAR/);
assert.throws(() => importHar(fixture("known-clean"), { headerPolicy: "guess" }), /header policy/);
console.log("HAR public API checks passed.");
