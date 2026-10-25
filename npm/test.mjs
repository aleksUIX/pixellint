/**
 * Smoke tests for the npm package, run against the committed WASM build.
 * Node's built-in test assertions keep this dependency-free: `npm test`.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

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

assert.equal(rulepacks().filter((pack) => pack.id.startsWith("vendor/")).length, 206, "every shipped vendor pack should be listed");
assert.ok(vendors().length >= 80, "the vendor directory should be present");
assert.equal(vendorForHost("pixel.mathtag.com")?.vendor, "mediamath");
assert.equal(vendorForHost("nobody.example"), null);

// Both npm entry points must use the refreshed engine for live VAST findings.
const cjs = createRequire(import.meta.url)("./index.cjs");
for (const entry of [{ validate, validateMany, vendorForHost }, cjs]) {
  // Indexed affiliate fields and optional product mappings retain their own scope.
  for (const [artifact, code] of [
    ["https://www.emjcd.com/u?CID=123&TYPE=456&OID=synthetic&METHOD=S2S&CJEVENT=abc123&ITEM1=sku&AMT2=1&QTY1=1", "vendor.cj.items_incomplete"],
    ["https://track.linksynergy.com/ep?mid=123&ord=synthetic&skulist=one&qlist=1%7C0&amtlist=100", "vendor.rakuten.numeric_lists_misaligned"],
    ["https://px.mountain.com/sti?shaid=123&cb=opaque&shpp=money", "vendor.mntn-image.param.shpp.invalid"],
  ]) assert.deepEqual(entry.validate(artifact).reports.flatMap(report => report.violations.map(finding => finding.code)), [code]);
  for (const artifact of [
    "https://www.emjcd.com/u?CID=123&TYPE=456&OID=legacy%20order&ITEM3=legacy",
    "https://px.mountain.com/sti?shaid=123&cb=opaque&conv=1&shpp=null&shpi=null",
    "https://track.linksynergy.com/ep?mid=123&ord=synthetic&skulist=sku%7Cencoded%7CDISCOUNT&qlist=1%7C0&amtlist=100%7C-20",
  ]) assert.deepEqual(entry.validate(artifact).reports.flatMap(report => report.violations), []);

  // SDK aliases, initialization and fallback retain separate transport contracts.
  for (const [artifact, pack, code] of [
    ["https://prreqcroab.icu/pixel;a=p-synthetic;r=-1", "vendor/quantcast", "vendor.quantcast.param.r.invalid"],
    ["https://trc.taboola.com/1234567/log/3/unip?en=Custom&tim=clock", "vendor/taboola-unip", "vendor.taboola-unip.param.tim.invalid"],
    ["https://insight.adsrvr.org/track/cei?merchant_id=synthetic&ref=https%3A%2F%2Fexample.test&upv=3.0.0&cookie_sync=2", "vendor/the-trade-desk", "vendor.the-trade-desk.param.cookie_sync.invalid"],
    ["https://insight.adsrvr.org/track/rt?adv=synthetic&pixel_ids=opaque&value=money", "vendor/the-trade-desk-realtime-fallback", "vendor.the-trade-desk-realtime-fallback.param.value.invalid"],
  ]) {
    const result = entry.validate(artifact);
    assert.deepEqual(result.coverage.vendor_rulepacks, [pack]);
    assert.deepEqual(result.reports.flatMap(report => report.violations.map(finding => finding.code)), [code]);
  }
  assert.equal(entry.vendorForHost("pixel.quantcount.com")?.vendor, "quantcast");
  const nullableTtd = entry.validate(JSON.stringify({url:"https://insight.adsrvr.org/track/realtimeconversion",method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({data:[{adv:"synthetic",pixel_ids:["opaque"],data_processing_option:null}]})}), {kind:"request"});
  assert.deepEqual(nullableTtd.reports.flatMap(report => report.violations), []);

  for (const [artifact, pack, code] of [
    ["https://googleads4.g.doubleclick.net/pcs/view?xai=x&sai=y", "vendor/google-cm360-pcs-view", "vendor.google-cm360-pcs-view.param.sig.missing"],
    ["https://track.adctv.com/", "vendor/adctv-tracker", "vendor.adctv-tracker.param.event.missing"],
    ["https://00px.net/pixel/token/e.gif?t=INSERIR+CACHEBUSTER", "vendor/adxspace-pixel", "vendor.adxspace-pixel.cachebuster.installation_stub"],
    ["https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=[TIMESTAMP]", "vendor/ias-video-pixel", "vendor.ias-video-pixel.xsid.unstable_macro"],
  ]) {
    const report = entry.validate(artifact, { kind: "vast", state: "template" }).reports.find(report => report.plugin_id === pack);
    assert.ok(report?.violations.some(finding => finding.code === code && finding.severity === "warning"));
  }
  for (const artifact of ["https://00px.net/vast/pixel/token", "https://pagead2.googlesyndication.com/pcs/view"]) {
    assert.ok(entry.validate(artifact, { kind: "vast" }).reports.every(report => !report.plugin_id.startsWith("vendor/")), "undocumented siblings stay outside pack scope");
  }
  const generatedTracker = entry.validate("https://00px.net/tracking/token/starts", { kind: "vast", state: "template" });
  assert.deepEqual(generatedTracker.coverage.vendor_rulepacks, ["vendor/adxspace-vast-tracking"]);
  assert.ok(generatedTracker.reports.flatMap(report => report.violations).some(finding =>
    finding.code === "vendor.adxspace-vast-tracking.param.s.missing" && finding.severity === "warning"));
  assert.equal(entry.vendorForHost("track.activemetering.com")?.vendor, "disqo");
  const uvp = entry.validate("https://unified.adsafeprotected.com/vevent/start/1111111/66666666?xsId=[PLEASE_IMPLEMENT_UNIQUE_ADSERVER_IMPRESSION_ID_HERE]", { kind: "vast", state: "template" });
  assert.ok(uvp.reports.flatMap((report) => report.violations).some((finding) => finding.code === "vendor.ias-video-pixel.xsid.placeholder" && finding.severity === "error"));
  assert.equal(entry.vendorForHost("00px.net")?.vendor, "adxspace");
  assert.equal(entry.vendorForHost("t.stredeo.com")?.vendor, "stredeo");
  assert.equal(entry.vendorForHost("ads283.adtelligent.com")?.vendor, "adtelligent");
  assert.equal(entry.vendorForHost("va-trk.smadex.com")?.vendor, "smadex");
  assert.equal(entry.vendorForHost("n.adv.lijit.com")?.vendor, "sovrn");
  const segment = entry.validate(JSON.stringify({ writeKey: "YOUR_WRITEKEY", type: "track", event: "Order Completed", userId: "synthetic-user" }), { kind: "json" });
  assert.ok(segment.reports.flatMap(report => report.violations).some(finding => finding.code === "vendor.segment.body.write_key_placeholder" && finding.severity === "warning" && finding.source.level === "heuristic"));
  const redacted = entry.validate(JSON.stringify({ api_key: "synthetic-project", event: "$pageview", distinct_id: "a".repeat(64), properties: { $current_url: null, $ip: null } }), { kind: "json" });
  assert.ok(redacted.reports.some(report => report.plugin_id === "vendor/posthog"));
  assert.ok(redacted.reports.every(report => report.violations.every(finding => finding.severity !== "error")));
  const hashedIp = entry.validate(JSON.stringify({ api_key: "synthetic-project", event: "$pageview", distinct_id: "synthetic-user", properties: { $ip: "a".repeat(64) } }), { kind: "json" });
  assert.ok(hashedIp.reports.flatMap(report => report.violations).some(finding => finding.severity === "error" && finding.code === "vendor.posthog.body.hashed_plaintext_field"));

  // Coverage describes the contracts that ran, independently of findings.
  for (const [artifact, pack] of [
    ["https://img.byspotify.com/?key=synthetic&a=init", "vendor/spotify-image"],
    ["https://tags.srv.stackadapt.com/conv?cid=synthetic", "vendor/stackadapt-pixel"],
    ["https://q.quora.com/_/ad/synthetic/pixel?tag=CustomEvent", "vendor/quora-pixel"],
    ["https://yandex.com/ads/adfox/123456/getCode?pr=1", "vendor/yandex-adfox-links"],
    ["https://securepubads.g.doubleclick.net/live/pcs/view?xai=x&sai=s&sig=g", "vendor/google-ima-live-pcs"],
    ["https://tpsc-video.doubleverify.com/event.png?vstevt=4&dup=synthetic-impression", "vendor/doubleverify-event"],
    ["https://tpsc-video.doubleverify.com/visit.jpg?ctx=12345&cmp=12345&plc=12345&sid=12345", "vendor/doubleverify"],
    ["https://pubads.g.doubleclick.net/pagead/live/interaction?ai=synthetic-ai&sigh=synthetic-signature&label=synthetic-event", "vendor/google-ima-interaction"],
  ]) {
    const result = entry.validate(artifact, { kind: "vast" });
    assert.deepEqual(result.coverage, { status: "vendor_checked", core_checks_ran: true, vendor_rulepacks: [pack] });
    assert.equal(isOk(result), true);
    assert.deepEqual(result.reports.flatMap(report => report.violations), [], "covered clean endpoints can have no findings");
  }
  assert.equal(entry.vendorForHost("tpsc-video.doubleverify.com")?.vendor, "doubleverify");
  const dvMissing = entry.validate("https://tpsc-video.doubleverify.com/event.png", { kind: "vast" });
  assert.equal(dvMissing.coverage.status, "vendor_checked");
  assert.ok(dvMissing.reports.flatMap(report => report.violations).some(finding => finding.code === "vendor.doubleverify-event.param.vstevt.missing"));

  const knownCoreOnly = "https://unified.adsafeprotected.com/pixel.png";
  const unknownCoreOnly = "https://unrecognized.example/pixel";
  for (const [artifact, status] of [[knownCoreOnly, "recognized_uncontracted"], [unknownCoreOnly, "unrecognized"]]) {
    const result = entry.validate(artifact);
    assert.deepEqual(result.coverage, { status, core_checks_ran: true, vendor_rulepacks: [] });
    assert.equal(isOk(result), true, "coverage does not invent validation errors");
    const invalidPrivacy = entry.validate(`${artifact}?gdpr=invalid`);
    assert.equal(invalidPrivacy.coverage.status, status, "coverage is independent of validation success");
    assert.ok(invalidPrivacy.reports.flatMap(report => report.violations).some(finding => finding.code === "core.privacy.gdpr_invalid"));
    assert.equal(isOk(invalidPrivacy), false);
  }
  const capturedUnknown = entry.validate(JSON.stringify({ url: unknownCoreOnly, method: "GET", headers: {} }), { kind: "request" });
  assert.deepEqual(capturedUnknown.coverage, { status: "unrecognized", core_checks_ran: true, vendor_rulepacks: [] });

  const endpointDocument = entry.validateMany({ artifacts: [
    { artifact: unknownCoreOnly },
    { artifact: knownCoreOnly },
    { artifact: "https://tpsc-video.doubleverify.com/event.png?vstevt=4" },
  ] });
  assert.deepEqual(endpointDocument.artifacts.map(artifact => artifact.coverage), [
    { status: "unrecognized", core_checks_ran: true, vendor_rulepacks: [] },
    { status: "recognized_uncontracted", core_checks_ran: true, vendor_rulepacks: [] },
    { status: "vendor_checked", core_checks_ran: true, vendor_rulepacks: ["vendor/doubleverify-event"] },
  ], "document artifacts preserve each endpoint's coverage");

  assert.equal(entry.vendorForHost("yandex.ru"), null, "path-scoped Adfox attribution requires a URL");
  const adfox = entry.validate("https://yandex.ru/adfox/123456/event");
  assert.equal(adfox.coverage.status, "recognized_uncontracted");
  assert.equal(adfox.reports.find(report => report.plugin_id === "directory")?.detected_vendor, "yandex-adfox");
  const normalizedAdfox = entry.validate("https://yandex.ru/other/../adfox/123456/event");
  assert.equal(normalizedAdfox.coverage.status, "recognized_uncontracted", "Adfox attribution uses the requested normalized path");
  for (const artifact of [
    "https://yandex.ru/search",
    "https://yandex.ru/search?redirect=/adfox/123456/event",
    "https://yandex.ru/adfoxtest/123456/event",
    "https://yandex.ru/adfox/../search",
    "https://yandex.ru/adfox/%2e%2e/search",
  ]) {
    const result = entry.validate(artifact);
    assert.equal(result.coverage.status, "unrecognized", "Adfox attribution stays within its path boundary");
    assert.ok(result.reports.every(report => report.plugin_id !== "directory"));
  }

  const amazonTelemetry = entry.validate("https://vfw.amazon-adsystem.com/dv/vtrk.doubleverify.com/?tid=synthetic&ec=vast&ea=complete&cid=synthetic-impression", { kind: "vast" });
  assert.deepEqual(amazonTelemetry.reports.map(report => report.plugin_id), ["core", "directory"]);
  assert.deepEqual(amazonTelemetry.coverage, { status: "recognized_uncontracted", core_checks_ran: true, vendor_rulepacks: [] });
  assert.deepEqual(amazonTelemetry.reports.flatMap(report => report.violations.map(finding => finding.code)), ["directory.no_rulepack_coverage"]);
}
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

for (const entry of [{ validate }, cjs]) {
  for (const [artifact, options, owner, finding] of [
    ["https://flask.nextdoor.com/pixel?ev=purchase&pl=https%3A%2F%2Fexample.test%2F", {}, "vendor/nextdoor-pixel", "vendor.nextdoor-pixel.param.pid.missing"],
    ["https://px.mountain.com/sti?shaid=synthetic", {}, "vendor/mntn-image", "vendor.mntn-image.param.cb.missing"],
    ["https://pixel.quantserve.com/pixel/bad.gif?pcode=p-synthetic", {}, "vendor/quantcast-image", "vendor.quantcast-image.param.pcode.invalid"],
    [JSON.stringify({url:"https://pixels.spotify.com/v1/ingest",method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({batch:[{pixel_id:"synthetic",events:[{action:"purchase",body:{value:"bad"}}]}]})}), {kind:"request"}, "vendor/spotify-ingest", "vendor.spotify-ingest.body.body.value.invalid"],
    [JSON.stringify({url:"https://pixel.quantserve.com/conversion?a=p-synthetic",method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify([{name:"Purchase",user:{}}])}), {kind:"request"}, "vendor/quantcast-conversions-api", "vendor.quantcast-conversions-api.body.user_identity"],
  ]) {
    const result = entry.validate(artifact, options);
    assert.deepEqual(result.coverage.vendor_rulepacks, [owner]);
    assert.ok(result.reports.flatMap(report => report.violations).some(v => v.code === finding), finding);
  }
  const image = entry.validate("https://pixel.quantserve.com/pixel/p-synthetic.gif?revenue=12.5");
  assert.deepEqual(image.coverage.vendor_rulepacks, ["vendor/quantcast-image"]);
  assert.equal(image.reports.flatMap(report => report.violations).length, 0);
  assert.equal(entry.validate("https://pixel.quantserve.com/pixels").coverage.status, "recognized_uncontracted");
}

console.log(`pixellint ${version()}: npm smoke tests passed`);

await import("./test-har.mjs");

await import("./sessions.test.mjs");

// New collectors and repaired browser forms must agree through both package entry points.
for (const entry of [{ validate, isOk }, cjs]) {
  for (const [url, code] of [
    ["https://s.amazon-adsystem.com/iu3?event=Purchase", "vendor.amazon-ad-tag.param.pid.missing"],
    ["https://aax-eu.amazon-adsystem.com/s/iu3?pid=synthetic", "vendor.amazon-ad-tag-regional.param.event.missing"],
    ["https://d.adroll.com/ipixel/ADV/PIX", "vendor.adroll-image.param.name.missing"],
    ["https://d.adroll.com/pixel/ADV/PIX?conv_value=free", "vendor.adroll-pixel.param.conv_value.invalid"],
    ["https://alb.reddit.com/rp.gif?id=t2_synthetic&event=Purchase&em=raw", "vendor.reddit.param.em.invalid"],
  ]) assert.ok(entry.validate(url,{kind:"url"}).reports.flatMap(r=>r.violations).some(v=>v.code===code));
  const beacon={url:"https://tr.outbrain.com/unifiedPixel?marketerId=synthetic&ob_click_id=synthetic&name=Purchase&sb=1",method:"POST",headers:{}};
  assert.ok(entry.isOk(entry.validate(JSON.stringify(beacon),{kind:"request"})));
  assert.ok(entry.isOk(entry.validate("https://d.adroll.com/pixel/ADV/PIX?conv_value=1250&adroll_currency=USC",{kind:"url"})));
}

for (const entry of [{ validate }, cjs]) {
  for (const [directory, file, owner, finding] of [
    ["optimizely-events", "depth049-event-type-timestamp-1792560000000.txt", "vendor/optimizely-events", "vendor.optimizely-events.body.timestamp.invalid"],
    ["tiktok-browser", "depth049-bad-property-value.txt", "vendor/tiktok-browser", "vendor.tiktok-browser.body.properties.value.invalid"],
    ["microsoft-clarity-collect", "depth049-bad-envelope-8.txt", "vendor/microsoft-clarity-collect", "vendor.microsoft-clarity-collect.body.e[8].invalid"],
  ]) {
    const artifact = readFileSync(new URL(`../fixtures/vendor-${directory}/${file}`, import.meta.url), "utf8");
    const result = entry.validate(artifact, { kind: "request" });
    assert.deepEqual(result.coverage.vendor_rulepacks, [owner]);
    assert.ok(result.reports.flatMap(r => r.violations).some(v => v.code === finding));
  }
  const configured = entry.validate("https://sb.scorecardresearch.com/b?c1=2&c2=opaque&cs_cfg=0000110");
  assert.deepEqual(configured.coverage.vendor_rulepacks, ["vendor/comscore-modern"]);
  assert.equal(configured.reports.flatMap(r => r.violations).length, 0);
}

// Collector routing and multipart NDJSON must agree in both package entry points.
for (const entry of [{validate}, cjs]) {
  for (const [slug, fixture, finding] of [
    ["piwik-pro", "bulk-second-row", "vendor.piwik-pro.param.h.invalid"],
    ["woopra", "push-second-record", "vendor.woopra.body.params.event.invalid"],
    ["yahoo-browser", "hash-array-item", "vendor.yahoo-browser.body.userData.ids.EMAIL[].invalid"],
    ["x", "event-later-tuple", "vendor.x.body.[1].invalid"],
  ]) {
    const artifact = readFileSync(new URL(`../fixtures/vendor-${slug}/depth050-${fixture}.txt`, import.meta.url), "utf8");
    const result = entry.validate(artifact, {kind: slug === "x" ? "url" : "request"});
    assert.ok(result.reports.flatMap(r => r.violations).some(v => v.code === finding));
  }
}

// Existing browser collectors retain their independent wire formats.
for (const entry of [{validate}, cjs]) {
  const clean = entry.validate("https://px.ads.linkedin.com/collect?pid=123,456&fmt=js");
  assert.equal(clean.reports.flatMap(r => r.violations).length, 0);
  for (const [url, finding] of [
    ["https://ct.pinterest.com/v3/?tid=opaque&event=Custom&ed=%7B%22value%22%3A%22money%22%7D", "vendor.pinterest.body.value.invalid"],
    ["https://q.quora.com/_/ad/opaque/pixel?tag=DwellTime&dwt=0&ive=blur", "vendor.quora-pixel.param.dwt.invalid"],
    ["https://ping.chartbeat.net/ping?h=example.test&g=123&p=/a&d=example.test&t=opaque&R=1&W=1&I=0", "vendor.chartbeat.activity_flags_conflict"],
  ]) assert.ok(entry.validate(url).reports.flatMap(r => r.violations).some(v => v.code === finding));
}
