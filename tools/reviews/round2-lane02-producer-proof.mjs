import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import vm from 'node:vm';
import assert from 'node:assert/strict';

const root = process.argv[2];
const sources = process.argv[3];
assert(root && sources, 'Pass checkout and pinned measurement-sources paths.');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const full = fs.readFileSync(path.join(sources, 'fullstory-sdk-decoded.js'), 'utf8');
const clip = fs.readFileSync(path.join(sources, 'clipflip-vpaid.raw'), 'utf8');
assert.equal(hash(full), '11258843fcf3fb75f1ad6a3e12b158b773775bcac8affa5b21c1360500aacfe5');
assert.equal(hash(clip), '3354196f3ad63bca0fbf63d80a2ae6266519be1c629ec5f067fef50d8207667f');
const out = { source_hashes: { fullstory: hash(full), clipflip: hash(clip) }, network: 'All source fragments execute offline with synthetic values. fetch is intercepted.', settings: [], page: [], overlay: [] };
const save = (slug, name, value) => fs.writeFileSync(path.join(root, 'fixtures', `vendor-${slug}`, `${name}.json`), JSON.stringify(value) + '\n');
const capture = (url, body, method = 'POST', mime = 'text/plain') => ({ url, method, headers: { 'Content-Type': mime }, body: JSON.stringify(body) });

// Evaluate the exact constructor expression without starting the SDK.
const objectStart = full.indexOf('T={OrgId:') + 2;
const objectEnd = full.indexOf('},u&&(T.ReplayFlags=u)', objectStart) + 1;
assert(objectStart > 1 && objectEnd > objectStart);
const objectExpression = full.slice(objectStart, objectEnd);
const environment = {
  m: 'opaque-user', d: { sessionId: 'opaque-session', pageId: 'opaque-page' },
  A: '', k: '', c: 1280, a: 720, w: undefined, v: 1440, l: 900,
  I: '', _: false, C: '', y: undefined, E: undefined, S: false,
  jr: { Ver: 'synthetic-build', TS: 1791603930 },
};
for (const [name, override] of [['source-page-text-plain', {}], ['privacy-and-undefined-omitted', { A: undefined, k: undefined, I: undefined, c: undefined, a: undefined, v: undefined, l: undefined }]]) {
  const body = vm.runInNewContext(`(function(){return (${objectExpression})}).call({Nh:'synthetic-org',C:{options:{snippetVersion:undefined}}})`, { ...environment, ...override });
  assert.equal(body.SessionKeyHash, '');
  assert.equal(body.PageKeyHash, '');
  const serialized = JSON.parse(JSON.stringify(body));
  assert(!('PreviewMode' in serialized));
  assert(!('TabId' in serialized));
  save('fullstory-page', name, capture('https://rs.fullstory.com/rec/page', body));
  out.page.push({ name, keys: Object.keys(serialized), empty_key_hashes: true, omitted_preview_and_tab: true });
}
assert(full.includes('fi.TextPlain="text/plain"'));
assert(full.includes('"/rec/page",{type:"string",data:xr(this.C.wdx,t)}'));
assert(full.includes('n=t.previewMode?Ja(this.C):this.Yu'));
assert(full.includes('case 4:return[2,Qa(this.C,this.Ju,Ja(this.C),t)]'));
const settingsStart = full.indexOf('function Qa(');
const settingsEnd = full.indexOf('function Ja(', settingsStart);
const settings = full.slice(settingsStart, settingsEnd);
const setupStart = settings.indexOf('r=null!');
const setupEnd = settings.indexOf(',[4,Va(', setupStart);
const setup = settings.slice(setupStart, setupEnd);
const routeStart = settings.indexOf('"/s/settings/"');
const routeEnd = settings.indexOf(')];case 1:', routeStart);
const route = settings.slice(routeStart, routeEnd);
for (const s of [{ orgId: 'synthetic-org' }, { orgId: 'synthetic-org', previewMode: true }, { orgId: 'synthetic-org', version: 'future.version' }]) {
  const result = vm.runInNewContext(`(function(){var r,o,h;${setup};return ${route}})()`, { s });
  assert(result.startsWith('/s/settings/synthetic-org/'));
  out.settings.push({ input: s, path: result });
}

// Execute only the tracking function with fetch replaced by a local recorder.
const clipStart = clip.indexOf('let calculateProgressPercentage=');
const clipEnd = clip.indexOf(';"undefined"!=typeof module', clipStart);
assert(clipStart >= 0 && clipEnd > clipStart);
const fragment = clip.slice(clipStart, clipEnd) + ';globalThis.send=sendTrackingEvent;';
for (const [name, event, overlay, player] of [
  ['source-overlay-post', 'future-event', 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['source-slider-navigate', 'custom-slider-event', 'opaque?sliderNavigate=next', { currentTime: 0, duration: 100 }],
  ['source-engaged', 'hover', 'opaque-overlay', { currentTime: -10, duration: 100 }],
  ['source-nonfinite-null', 'future-event', 'opaque-overlay', { currentTime: NaN, duration: 100 }],
  ['source-null-event', null, 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['source-number-event', 42, 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['source-object-event', { x: 1 }, 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['source-array-event', [], 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['source-false-event', false, 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['source-undefined-event-omitted', undefined, 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['empty-event-open', '', 'opaque-overlay', { currentTime: 125, duration: 100 }],
  ['falsy-overlay-get-control', 'overlay-click', null, null],
  ['source-empty-overlay-get', 'overlay-click', '', null],
  ['source-zero-overlay-get', 'future-event', 0, null],
  ['source-false-overlay-get', 'future-event', false, null],
]) {
  const calls = [];
  const context = vm.createContext({ fetch: async (url, init) => { calls.push({ url, ...init }); return {}; }, console: { log() {} } });
  vm.runInContext(fragment, context);
  let postEmissionError = null;
  try { await context.send('future', 'app', 'synthetic-creative', event, overlay, player); }
  catch (error) { postEmissionError = error.name; }
  if (['source-null-event', 'source-number-event', 'source-object-event', 'source-undefined-event-omitted', 'source-false-event'].includes(name)) {
    assert.equal(postEmissionError, 'TypeError');
    assert.equal(calls.length, 1);
  } else assert.equal(postEmissionError, null);
  assert(calls.length > 0);
  const call = name === 'source-engaged' ? calls[1] : calls[0];
  assert(call);
  const value = { url: call.url, method: call.method, headers: call.headers || {} };
  if (call.body !== undefined) value.body = call.body;
  if (!overlay) assert.equal(call.method, 'GET');
  if (name === 'source-overlay-post') assert.equal(JSON.parse(call.body).data.progress, 125);
  if (name === 'source-engaged') assert.equal(JSON.parse(call.body).data.eventType, 'engaged');
  if (name === 'source-nonfinite-null') assert.equal(JSON.parse(call.body).data.progress, null);
  save('clipflip-vast-tracking', name, value);
  out.overlay.push({ name, calls: calls.length, post_emission_error: postEmissionError, method: value.method, body_fields: call.body ? Object.keys(JSON.parse(call.body).data) : [] });
}
fs.writeFileSync(path.join(root, 'fixtures/vendor-fullstory-page/source-producer-proof.json'), JSON.stringify(out, null, 2) + '\n');
console.log(JSON.stringify({ settings: out.settings.length, page: out.page.length, overlay: out.overlay.length, intercepted: true }));
