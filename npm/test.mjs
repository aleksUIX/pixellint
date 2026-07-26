/**
 * Smoke tests for the npm package, run against the committed WASM build.
 * Node's built-in test assertions keep this dependency-free: `npm test`.
 */

import assert from "node:assert/strict";

import { isOk, rulepacks, validate, vendorForHost, vendors, version } from "./index.mjs";

const clean = validate("https://www.facebook.com/tr?id=1234567890123456&ev=PageView");
assert.equal(isOk(clean), true, "a clean Meta pixel should pass");
assert.ok(
  clean.reports.some((report) => report.plugin_id === "vendor/meta"),
  "the Meta pack should run",
);

const broken = validate("https://www.facebook.com/tr?ev=PageView");
assert.equal(isOk(broken), false, "a pixel without an id should fail");
const codes = broken.reports.flatMap((report) =>
  report.violations.map((violation) => violation.code),
);
assert.deepEqual(codes, ["vendor.meta.param.id.missing"]);

const consent = validate("https://example.com/px?gdpr=1");
assert.deepEqual(
  consent.reports.flatMap((report) => report.violations.map((violation) => violation.code)),
  ["core.privacy.gdpr_consent_missing"],
  "IAB consent rules should run in WASM too",
);

const template = validate("https://example.com/px?cb=[CACHEBUSTING]", { state: "template" });
assert.equal(isOk(template), true, "templates keep their macros");

const attributed = validate("https://trc.taboola.com/actions?a=1");
assert.equal(
  attributed.reports.find((report) => report.plugin_id === "directory")?.detected_vendor,
  "taboola",
);

// A conversion API body is validated per event, not as a URL.
const payload = validate(
  JSON.stringify({
    data: [
      { event_name: "Purchase", event_time: 1770000000000, action_source: "website", user_data: {} },
    ],
  }),
  { kind: "json" },
);
const payloadCodes = payload.reports.flatMap((report) =>
  report.violations.map((violation) => violation.code),
);
assert.ok(
  payloadCodes.includes("vendor.meta-conversions-api.body.event_time.invalid"),
  "millisecond timestamps should be caught in the body",
);
assert.ok(
  payloadCodes.includes("vendor.meta-conversions-api.body.purchase_requires_value_and_currency"),
  "cross-field body rules should run",
);

const brokenJson = validate('{"data":[{"event_name":}]}', { kind: "json" });
assert.deepEqual(
  brokenJson.reports.flatMap((report) => report.violations.map((violation) => violation.code)),
  ["core.json.parse_error"],
);

assert.ok(rulepacks().length >= 15, "every rulepack should be listed");
assert.ok(vendors().length >= 80, "the vendor directory should be present");
assert.equal(vendorForHost("pixel.mathtag.com")?.vendor, "mediamath");
assert.equal(vendorForHost("nobody.example"), null);
assert.match(version(), /^\d+\.\d+\.\d+$/);

console.log(`pixellint ${version()}: npm smoke tests passed`);
