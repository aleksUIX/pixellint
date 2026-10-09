/**
 * Smoke tests for the npm package, run against the committed WASM build.
 * Node's built-in test assertions keep this dependency-free: `npm test`.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { isOk, rulepacks, validate, validateMany, vendorForHost, vendors, version } from "./index.mjs";

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

const captured = {
  url: "https://plausible.io/api/event",
  method: "POST",
  headers: { "Content-Type": "application/json; charset=UTF-8", "User-Agent": "Mozilla/5.0" },
  body: JSON.stringify({ name: "pageview", url: "https://example.org/", domain: "example.org" }),
};
const capturedResult = validate(JSON.stringify(captured), { kind: "request" });
assert.equal(isOk(capturedResult), true, "complete HTTP requests run on WASM's supplied clock");
assert.deepEqual(capturedResult.reports.map(report => report.plugin_id), ["core", "vendor/plausible"]);
const invalidMethod = validate(JSON.stringify({ ...captured, method: "GET" }), { kind: "request" });
assert.ok(invalidMethod.reports.flatMap(report => report.violations).some(finding => finding.code === "vendor.plausible.http.method.invalid"));
const malformedCapture = validate(JSON.stringify({ url: captured.url, method: "POST" }), { kind: "request" });
assert.deepEqual(malformedCapture.reports.flatMap(report => report.violations.map(finding => finding.code)), ["core.request.invalid_envelope"]);

assert.equal(rulepacks().filter((pack) => pack.id.startsWith("vendor/")).length, 159, "every shipped vendor pack should be listed");
assert.ok(vendors().length >= 80, "the vendor directory should be present");
assert.equal(vendorForHost("pixel.mathtag.com")?.vendor, "mediamath");
assert.equal(vendorForHost("nobody.example"), null);
assert.match(version(), /^\d+\.\d+\.\d+$/);
assert.equal(version(), JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")).version, "the bundled engine must match the package version");

const validGpp = "DBABLA~CAAAVVVVVVRA.QA";
assert.equal(isOk(validate(`https://example.com/px?gpp=${validGpp}&gpp_sid=7`)), true);
assert.ok(validate(`https://example.com/px?gpp=${validGpp}&gpp_sid=8`).reports.flatMap((report) => report.violations).some((finding) => finding.code === "core.privacy.gpp_sid_sections_mismatch"));
assert.ok(validate("https://example.com/px?gpp=DBACNYA&gpp_sid=2").reports.flatMap((report) => report.violations).some((finding) => finding.code === "core.privacy.gpp_sections_invalid"));
const conflictedTcfExample = "CQSbk4AQSbk4ANwAAAENAwCgAAAAAAAAAAYgACPAAAAA.IDKQA4AAgAKAGQAygAAA.YAAAAAAAAAAA";
assert.ok(validate(`https://example.com/px?gdpr=1&gdpr_consent=${conflictedTcfExample}`).reports.flatMap((report) => report.violations).some((finding) => finding.code === "core.privacy.tc_string_policy_warning" && finding.severity === "warning"));

assert.throws(
  () => validate("<script src=https://example.com/px.js></script>", { kind: "html" }),
  /not a validation kind/,
);

const many = validateMany({
  document_kind: "vast",
  artifacts: [
    { artifact: "https://example.com/pixel?id=1#frag" },
    { artifact: "https://example.com/pixel?id=1#frag" },
    { artifact: "https://www.facebook.com/tr?ev=PageView" },
  ],
});
assert.equal(many.summary.artifacts_total, 3);
assert.equal(many.summary.unique_artifacts, 2);
assert.equal(many.summary.errors, 1);
assert.equal(many.artifacts[0].occurrences.length, 2);

const loader = validate("https://cdn-gl.imrworldwide.com/conf/P12345678-1234-1234-1234-123456789012.js#name=nlsnInstance&ns=NOLBUNDLE");
assert.equal(isOk(loader), true);
assert.ok(loader.reports.some((report) => report.plugin_id === "vendor/nielsen-config"));
assert.ok(loader.reports.every((report) => report.violations.every((finding) => finding.code !== "core.url.fragment_ignored")));

const freewheel = validate("https://demo.v.fwmrm.net/ad/g/1?nw=1&csid=site&prof=profile;;ptgt=a&slid=pre&slau=preroll&tpos=0;ptgt=a&slid=mid");
const slotFindings = freewheel.reports.flatMap((report) => report.violations);
assert.ok(slotFindings.some((finding) => finding.code === "vendor.freewheel.query.slau.missing" && finding.field === "query[3].slau"), "a later slot cannot borrow its ad unit");
assert.ok(slotFindings.some((finding) => finding.code === "vendor.freewheel.query.tpos.missing" && finding.field === "query[3].tpos"), "a later slot cannot borrow its start time");

const emailHash = "a".repeat(32);
const phoneHash = "b".repeat(40);
const mixed = `https://api.rlcdn.com/api/identity/v2/envelope?pid=14&it=4&iv=${emailHash}&it=11&iv=${phoneHash}`;
assert.equal(isOk(validate(mixed)), true, "mixed identity families use their own paired format");
assert.ok(validate(mixed.replace(phoneHash, emailHash)).reports.flatMap((report) => report.violations).some((finding) => finding.code === "vendor.liveramp-envelope.phone_hash_format"));

const originalNow = Date.now;
let clockReads = 0;
const referenceSeconds = 1_800_000_000;
Date.now = () => { clockReads += 1; return referenceSeconds * 1000; };
try {
  const conversion = (time) => JSON.stringify({ events: [{ id: "evt_clock", type: "page_viewed", timestamp_ms: time, action_source: "web", source_url: "https://shop.example/", data: { type: "contents" } }] });
  const current = validate(conversion(referenceSeconds * 1000), { kind: "json" });
  assert.equal(isOk(current), true, "WASM uses the supplied JavaScript clock");
  assert.equal(clockReads, 1, "single validation reads its clock once");
  const old = validate(conversion((referenceSeconds - 8 * 86400) * 1000), { kind: "json" });
  assert.ok(old.reports.flatMap((report) => report.violations).some((finding) => finding.code === "vendor.openai-conversions-api.body.timestamp_ms_time_window"));
  const before = clockReads;
  validateMany({ artifacts: [{ artifact: mixed }, { artifact: "https://www.facebook.com/tr?id=123&ev=PageView" }] });
  assert.equal(clockReads - before, 1, "document validation shares one clock across artifacts");
} finally {
  Date.now = originalNow;
}

console.log(`pixellint ${version()}: npm smoke tests passed`);
