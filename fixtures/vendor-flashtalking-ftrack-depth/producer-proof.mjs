import vm from 'node:vm';
import fs from 'node:fs';
import crypto from 'node:crypto';
const source = fs.readFileSync(process.argv[2], 'utf8');
const sha256 = crypto.createHash('sha256').update(source).digest('hex');
const reviewed = ['77c8c6a48bdfa3444ff0128161ab436693e3ae6be5409e269e873b743baf8bd6',
  '85e48b46c5da3f5e3a0e86763880dc197b9af2a016514e837ab11c76c7702ccb'];
if (!reviewed.includes(sha256)) throw new Error('SDK source differs from reviewed snapshots');
const variants = [
  {id:'desktop'}, {id:'iphone',platform:'iPhone'}, {id:'ipad',platform:'iPad'},
  {id:'android',platform:'Linux armv8l',ua:'synthetic android',touch:5},
  {id:'android-ua-desktop-platform',ua:'synthetic android'},
  {id:'android-ua-iphone-platform',platform:'iPhone',ua:'synthetic android'},
  {id:'absent-storage',storage:false}, {id:'security-error-storage',storage:'throw'},
  {id:'negative-timezone',timezone:-330}, {id:'positive-timezone',timezone:345},
  {id:'typeof-function',behavior:'function',openDatabase:true},
  {id:'latin1',language:'é-CA',cpu:'café',platform:'Winÿ'},
  {id:'colons',language:'a:en-GB',cpu:'x:y',platform:'Win:arm'},
  {id:'embedded-delimiter',cpu:'arm###foo',platform:'Win###other'},
  {id:'ambiguous-index-marker',cpu:'x###j:other'},
];
const cases = [];
for (const transport of ['xhr','xdr']) for (const variant of variants) {
  const requests = [];
  class FixedDate extends Date {
    constructor(...args) { super(...(args.length ? args : [1800000000000])); }
    getTimezoneOffset() { return variant.timezone ?? 0; }
  }
  class CaptureRequest {
    constructor() { this.headers = {}; if (transport === 'xhr') this.withCredentials = false; }
    open(method,url) { this.method=method;this.url=url; }
    setRequestHeader(name,value) { this.headers[name]=value; }
    send(body) { requests.push({url:this.url,method:this.method,headers:this.headers,body}); }
  }
  const navigator={platform:variant.platform??'Linux x86_64',language:variant.language??'en-GB',
    appCodeName:'Mozilla',maxTouchPoints:variant.touch??0,plugins:[],mimeTypes:[],
    userAgent:variant.ua??'synthetic desktop browser'};
  if (variant.cpu !== undefined) navigator.cpuClass=variant.cpu;
  const context={Date:FixedDate,URL,navigator,screen:{width:1920,height:1080,colorDepth:24},
    btoa:text=>{for (const char of text) if (char.charCodeAt(0)>255) throw new Error('InvalidCharacterError');
      return Buffer.from(text,'latin1').toString('base64');},
    document:{currentScript:{id:'synthetic-legacy-loader'},referrer:'https://referrer.example/',
      body:variant.behavior?{addBehavior:()=>{}}:{},getElementById:()=>null},
    D9v:{UserID:'synthetic-user'},D9r:{SingleDeviceID:true},devicePixelRatio:1,
    location:{hostname:'publisher.example',ancestorOrigins:[]}};
  context[transport==='xhr'?'XMLHttpRequest':'XDomainRequest']=CaptureRequest;
  if (variant.openDatabase) context.openDatabase=()=>{};
  for (const name of ['localStorage','sessionStorage','indexedDB']) {
    if (variant.storage==='throw') Object.defineProperty(context,name,{get(){throw new Error('Synthetic security error');}});
    else if (variant.storage!==false) context[name]={};
  }
  context.window=context;
  vm.runInNewContext(source,context,{timeout:1000,filename:'pinned-d9core'});
  if (requests.length!==1) throw new Error('Unexpected SDK request count');
  const request=requests[0];
  const device=JSON.parse(decodeURIComponent(new URLSearchParams(request.body).get('tbx')));
  cases.push({id:`sdk-${transport}-${variant.id}`,evidence_kind:'executed_official_producer',
    profile:'d9core_indexed_seed31',variant,transport,request,device,
    indexed_latin1:Buffer.from(device.D9_33,'base64').toString('latin1'),
    expected_findings:variant.id==='ambiguous-index-marker'
      ?[{code:'vendor.flashtalking-ftrack.fingerprint.unvalidated',severity:'info'}]:[]});
}
console.log(JSON.stringify({source_url:'https://d9.flashtalking.com/d9core',source_sha256:sha256,
  execution:'Unmodified official SDK in Node VM with fixed clock and captured constructors; no network requests executed',
  reference_time_unix_seconds:1800000000,cases},null,2));
