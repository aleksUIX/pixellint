import fs from 'node:fs';
import vm from 'node:vm';
import crypto from 'node:crypto';
import {createRequire} from 'node:module';
const [sources, tsPath, output] = process.argv.slice(2);
const ts=createRequire(import.meta.url)(tsPath);
const pins=Object.freeze({
  "src/middleware/senderWatchInfo/senderWatchInfo.ts": "ebcce587d6a3952294fcf15f3bf122b4f13bc1745131362e06db92ba496cc2a6",
  "src/middleware/prepareUrl/prepareUrl.ts": "89c3730e9b4518283685e8e61719b6912d4927a7a864d879640d1321d88c1906",
  "src/middleware/params/params.ts": "63bc9037195a44702ba4ba38d913dbc99e42b93558ba2a00408b0cb266132957",
  "src/utils/browserInfo/browserInfo.ts": "ea960e0115065c41cc90ff82b41fb5cbe568623253061c4f4f34a7dd5dff6ea0",
  "src/utils/flagsStorage/flagsStorage.ts": "e5c0f0186ff519fb243ec0c802a8f12d22f96567cf52a43c3a43018e5a350908",
  "src/utils/json/json.ts": "e32dfec355fb1bff2abfd2b3d9d821f570aad05ef11cb089cc6f058fe8a92ea1",
  "src/utils/querystring/querystring.ts": "9c281074a5874893e67f0ea50b25a1b58d70fced69a44c9b279541ed9d5ad956",
  "src/utils/querystring/safeEncodeURI.ts": "59f67d30f89d9534364579968f858ffdd095d934a6eff9a4111c7749b1ad5206",
  "src/sender/default/default.ts": "c75ed73422883734678196a8f6352f462038b4bcd80550a976feb7955c1f9e32",
  "src/sender/default/const.ts": "55a60020071bc731ca832cc16475ec3260b0f5c32847bdaf2f27b1ee00f44fca",
  "src/sender/default/query/watchAPI.ts": "d8fdda5084a13606263661f0a6580762b7be5002f4a37095075dc985f64eca2c",
  "src/transport/watchModes.ts": "7b3e74b70d9261e9c8d9cde8a77125f7cb44d247cba517431a08292842d387fb",
  "src/transport/beacon/beacon.ts": "02a4b9ee877546bdb9673f9f1f711af4474020e3db1b32afa2e07e12428b6bb3"
});
const loaded=new Map();
const flags={SENDER_COLLECT_FEATURE:false,DEBUG_EVENTS_FEATURE:false,DEBUG_EVENTS:false,TELEMETRY_FEATURE:false,TURBO_PARAMS_FEATURE:false};
const common={WATCH_URL_PARAM:'page-url',WATCH_REFERER_PARAM:'page-ref',WATCH_ENCODING_PARAM:'charset',WATCH_CLASS_PARAM:'cnt-class',DEFER_KEY:'nohit',PAGE_VIEW_BR_KEY:'pv',TRACK_HASH_BR_KEY:'wh',REQUEST_BODY_KEY:'site-info',TITLE_BR_KEY:'t',BROWSERINFO_QUERY_KEY:'browser-info',TELEMETRY_QUERY_KEY:'t',SENDER_TIME_BR_KEY:'st',REQUEST_MODE_KEY:'wmode',FORCE_URLENCODED_KEY:'force-urlencoded'};
const makePipe=(...f)=>(x)=>f.reduce((v,fn)=>fn(v),x);
const stubs={
 '@inject':{flags},'src/api/watch':common,'src/api/common':common,
 'src/utils/object':{mix:Object.assign,entries:Object.entries,isUndefined:v=>v===undefined,isNil:v=>v==null,getPath:(obj,path)=>path.split('.').reduce((a,k)=>a?.[k],obj)},
 'src/utils/browser/browser':{isIE:ctx=>Boolean(ctx.ie)},'src/utils/location/location':{getLocation:ctx=>ctx.location},
 'src/utils/string':{isString:v=>typeof v==='string',stringIndexOf:(a,b)=>a.indexOf(b)},'src/utils/string/remove':{trimRegexp:/^\s+|\s+$/g},
 'src/utils/array/join':{arrayJoin:(sep,a)=>a.join(sep),ctxJoin:sep=>a=>a.join(sep)},
 'src/utils/array/map':{ctxMap:f=>a=>a.map(f),cForEach:(f,a)=>a.forEach(f)},'src/utils/array/filter':{cFilter:(f,a)=>a.filter(f)},
 'src/utils/array/reduce':{ctxReduce:(f,init)=>a=>a.reduce(f,init),dirtyReduce:(f,init,a)=>a.reduce(f,init)},
 'src/utils/array/indexOf':{cIndexOfWin:(x,a)=>a.indexOf(x)},'src/utils/array/utils':{head:a=>a[0],indexOfWin:()=>((x,a)=>a.indexOf(x))},
 'src/utils/function/identity':{firstArg:x=>x},
 'src/utils/function/pipe':{pipe:makePipe},'src/utils/function/curry':{curry2:f=>a=>b=>f(a,b),equal:a=>b=>a===b},
 'src/utils/function/bind':{bindArg:(a,f)=>(...r)=>f(a,...r),bindArgs:(a,f)=>(...r)=>f(...a,...r),bind:(f,c)=>f.bind(c)},
 'src/utils/function/construct':{constructArray:()=>[]},'src/utils/function/memo':{memo:f=>{const m=new Map();return x=>{if(!m.has(x))m.set(x,f(x));return m.get(x)}}},
 'src/providers/counterOptions':{DEFAULT_COUNTER_TYPE:'0'},'src/providers/params/const':{METHOD_NAME_PARAMS:'params'},
 'src/utils/counter/getInstance':{getCounterInstance:()=>undefined},'src/utils/counterOptions/getCounterKey':{getCounterKey:()=> 'synthetic'},
 'src/utils/debugEvents':{dispatchDebuggerEvent(){}},'src/utils/turboParams':{setTurboInfo(){}},
 'src/utils/time/time':{TimeOne:()=>()=>1792886400,getSec:x=>x},
 'src/utils/errorLogger/throwFunction':{throwFunction:e=>{throw e}},'src/utils/debugEvents/debugRequest':{debugLogRequest(){},logRequestFailure(){},logRequestSuccess(){}},
 'src/utils/promise':{PolyPromise:Promise},'src/utils/browser/browser':{isIE:ctx=>Boolean(ctx.ie),isAndroidWebView:()=>false},
 'src/utils/errorLogger/knownError':{createKnownError:x=>new Error(String(x))},'src/utils/function/isNativeFunction/isNativeFunction':{isNativeFunction:()=>true},
 'src/utils/url':{addQuery:(u,q)=>u+(u.includes('?')?'&':'?')+q},
};
function load(file){
 if(loaded.has(file))return loaded.get(file);
 const raw=fs.readFileSync(`${sources}/${file}`);const hash=crypto.createHash('sha256').update(raw).digest('hex');if(pins[file]!==hash)throw Error(`Source changed: ${file}`);
 const cjs=ts.transpileModule(raw.toString(),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 const exports={};loaded.set(file,exports);
 const require=name=>{
  if(stubs[name])return stubs[name];
  const map={'src/utils/browserInfo/browserInfo':'src/utils/browserInfo/browserInfo.ts','../flagsStorage/flagsStorage':'src/utils/flagsStorage/flagsStorage.ts','src/utils/json':'src/utils/json/json.ts','src/utils/querystring':'src/utils/querystring/querystring.ts','./safeEncodeURI':'src/utils/querystring/safeEncodeURI.ts','../array/filter':'src/utils/array/filter.ts','../array/join':'src/utils/array/join.ts','./const':'src/sender/default/const.ts','./query':'src/sender/default/query/watchAPI.ts','src/middleware/senderWatchInfo':'src/middleware/senderWatchInfo/senderWatchInfo.ts'};
  if(name==='./const'&&file.includes('senderWatchInfo'))return {WATCH_RESOURCE:'watch'};
  if(name==='./query')return {createQuery:load(map[name]).createWatchQuery};
  if(name==='../array/filter')return stubs['src/utils/array/filter'];if(name==='../array/join')return stubs['src/utils/array/join'];
  if(map[name])return load(map[name]);throw Error(`Unknown dependency ${file}: ${name}`);
 };
 vm.runInNewContext(cjs,{exports,require,console:{log(){}},encodeURIComponent,decodeURIComponent,JSON,Promise},{timeout:1000});return exports;
}
const init=load('src/middleware/senderWatchInfo/senderWatchInfo.ts').senderWatchInfo;
const prepare=load('src/middleware/prepareUrl/prepareUrl.ts').prepareUrlMiddleware;
const params=load('src/middleware/params/params.ts').paramsMiddleware;
const browser=load('src/utils/browserInfo/browserInfo.ts').browserInfo;
const send=load('src/sender/default/default.ts').useDefaultSender;
const strings=load('src/utils/querystring/querystring.ts');
const getSrc=load('src/transport/watchModes.ts').getSrcUrl;
const beacon=load('src/transport/beacon/beacon.ts').request;
const inputs=[
 ['source-image-custom-object',{kind:'image',pv:1,data:{items:[null,3,'x'],custom:{value:'A&B',nullable:null}}}],
 ['source-form-params',{kind:'form',pv:0,data:{custom:{value:'A&B'},items:[1,null]}}],
 ['source-beacon-query',{kind:'beacon',pv:0,data:{custom:'beacon-value'}}],
 ['source-native-array',{kind:'image',pv:1,data:[null,3,'x']}],
 ['source-native-null-root',{kind:'image',pv:1,data:{toJSON(){return null}}}],
 ['source-native-string-root',{kind:'image',pv:1,data:{toJSON(){return 'custom string'}}}],
 ['source-native-number-root',{kind:'image',pv:1,data:{toJSON(){return 7}}}],
 ['source-native-boolean-root',{kind:'image',pv:1,data:{toJSON(){return false}}}],
 ['source-lone-surrogate',{kind:'form',pv:0,data:{custom:'\ud800'}}],
 ['source-native-nonfinite',{kind:'image',pv:1,data:{value:NaN,omitted:undefined,array:[undefined,Infinity]}}],
 ['source-null-params-omitted',{kind:'image',pv:1,data:null}],
 ['source-empty-params-omitted',{kind:'image',pv:1,data:''}],
 ['source-false-params-omitted',{kind:'image',pv:1,data:false}],
 ['source-long-page-url',{kind:'image',pv:1,url:'https://publisher.example/'+ 'a'.repeat(3000)}],
 ['source-astral-page-url',{kind:'image',pv:1,url:'https://publisher.example/'+ '😀'.repeat(1400)}],
 ['source-ie-page-url',{kind:'image',pv:1,ie:true,url:'https://publisher.example/'+ 'a'.repeat(900)}],
 ['source-goal-scheme',{kind:'image',pv:1,url:'goal://publisher.example/custom-goal'}],
 ['source-long-metadata-deferred',{kind:'image',pv:1,data:{long:'x'.repeat(3000)}}],
];
const cases=[];
for(const [id,input] of inputs){
 const captures=[];const ctx={ie:input.ie,location:{href:'https://publisher.example/article',host:'publisher.example'},JSON,navigator:{onLine:true}};
 const brInfo=browser().setVal('pv',input.pv);const info={brInfo,urlParams:{'page-url':input.url||ctx.location.href},middlewareInfo:{params:input.data},transportInfo:{}};
 const opts={id:123456,counterType:'0'};
 init(ctx,opts).beforeRequest(info,()=>{});prepare(ctx,opts).beforeRequest(info,()=>{});params(ctx,opts).beforeRequest(info,()=>{});
 const adapter=async(url,opt)=>{
  if(input.kind==='beacon')await beacon(ctx,u=>{captures.push({url:u,method:'POST',headers:{}});return true},url,opt);
  else if(input.kind==='image'){const q={...opt.rQuery};delete q.wmode;captures.push({url:getSrc(url,opt,q),method:'GET',headers:{}})}
  else {const capture={url:stubs['src/utils/url'].addQuery(url,strings.stringify(opt.rQuery)),method:opt.verb,headers:opt.rHeaders||{}};if(opt.rBody!==undefined)capture.body=opt.rBody;captures.push(capture)}
  return null;
 };
 await send(ctx,[[1,adapter]])(info,['https://mc.yandex.ru/watch/123456']);if(captures.length!==1)throw Error('Expected one local capture.');
 cases.push({id,transport:input.kind,ie:Boolean(input.ie),page_url_utf16_units:info.urlParams['page-url'].length,capture:captures[0]});
}
fs.writeFileSync(output,JSON.stringify({git_commit:'47a80d0f0f8278fbb32309b5444b801eda99556e',typescript_version:ts.version,source_pins:pins,method:'Unchanged pinned producer middleware, JSON/query serializers and default sender transpiled with TypeScript. Generic browser primitives stubbed; image relocation and beacon helper executed directly. No SDK page initialization or network APIs.',engine_derived:false,network_calls:0,cases},null,2)+'\n');
console.log(JSON.stringify({producer_captures:cases.length,network_calls:0,output}));
