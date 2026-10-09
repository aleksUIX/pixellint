import vm from 'node:vm';
import fs from 'node:fs';
import crypto from 'node:crypto';
const file = process.argv[2];
const source = fs.readFileSync(file, 'utf8');
const expected = '77c8c6a48bdfa3444ff0128161ab436693e3ae6be5409e269e873b743baf8bd6';
const sha256 = crypto.createHash('sha256').update(source).digest('hex');
if (sha256 !== expected) throw new Error('SDK source differs from reviewed snapshot');
const cases = [];
for (const mobile of [false, true]) {
  const requests = [];
  class FixedDate extends Date {
    constructor(...args) { super(...(args.length ? args : [1800000000000])); }
    getTimezoneOffset() { return 0; }
  }
  class CaptureXHR {
    constructor() { this.withCredentials = false; this.headers = {}; }
    open(method, url) { this.method = method; this.url = url; }
    setRequestHeader(name, value) { this.headers[name] = value; }
    send(body) { requests.push({url:this.url, method:this.method, headers:this.headers, body}); }
  }
  const navigator = {
    platform:mobile ? 'iPhone' : 'Linux x86_64', language:'en-GB', appCodeName:'Mozilla',
    maxTouchPoints:mobile ? 5 : 0, plugins:[], mimeTypes:[],
    userAgent:mobile ? 'Synthetic iPhone browser' : 'Synthetic desktop browser',
  };
  const screen = {width:mobile ? 390 : 1920, height:mobile ? 844 : 1080, colorDepth:24};
  const context = {
    Date:FixedDate, URL, XMLHttpRequest:CaptureXHR, navigator, screen,
    btoa:(text)=>Buffer.from(text, 'latin1').toString('base64'),
    document:{currentScript:{id:'synthetic-legacy-loader'}, referrer:'https://referrer.example/path', body:{},
      getElementById:()=>null},
    D9v:{UserID:'synthetic-user', CampID:'synthetic-campaign', CCampID:'synthetic-ccampaign'},
    D9r:mobile ? {DeviceID:true, callback:()=>{}} : {SingleDeviceID:true},
    devicePixelRatio:mobile ? 3 : 1,
    location:{hostname:'publisher.example', ancestorOrigins:[]},
    localStorage:{}, sessionStorage:{}, indexedDB:{},
  };
  context.window = context;
  vm.runInNewContext(source, context, {timeout:1000, filename:'d9core'});
  if (requests.length !== 1) throw new Error('Unexpected SDK request count');
  const request = requests[0];
  const encoded = new URLSearchParams(request.body).get('tbx');
  const device = JSON.parse(decodeURIComponent(encoded));
  cases.push({id:mobile ? 'sdk-mobile' : 'sdk-desktop', input:{mobile, navigator, screen,
    timestamp_milliseconds:1800000000000, timezone_offset_minutes:0}, request, device});
}
console.log(JSON.stringify({source_url:'https://d9.flashtalking.com/d9core', source_sha256:sha256,
  fetched_at:'2026-10-09', execution:'Node VM, network constructors captured, no network requests executed', cases}, null, 2));
