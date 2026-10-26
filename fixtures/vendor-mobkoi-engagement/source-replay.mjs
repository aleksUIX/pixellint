import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import vm from 'node:vm';

// Read the reviewed public SDK as data. Every network API is a local sink.
const sourceFile=process.argv[2];
if(!sourceFile)throw new Error('Pass the reviewed tag.min.js file as the first argument.');
const source=readFileSync(sourceFile,'utf8');
const digest=createHash('sha256').update(source).digest('hex');
const expected=process.argv[3]||'4f71000675249c21f048bc79e8915f78bd55cf033edb813cde7129b460180892';
if(expected)assert.equal(digest,expected,'SDK source changed after review');

function run({mbiid='synthetic/click+1',stored=null,beacon=true,consent=null,endpointBase=null}={}) {
  const network=[];const listeners={};const timers=[];const storage=new Map();
  if(stored)storage.set('mbk_session',JSON.stringify(stored));
  let clock=1000;
  const location={href:'https://publisher.example/landing'+(mbiid?'?mbiid='+encodeURIComponent(mbiid):''),search:mbiid?'?mbiid='+encodeURIComponent(mbiid):'',hostname:'publisher.example'};
  const add=(prefix,type,fn)=>(listeners[prefix+type]??=[]).push(fn);
  const document={visibilityState:'visible',addEventListener:(t,f)=>add('document:',t,f)};
  const context={document,location,history:{pushState(a,b,next){const u=new URL(next,location.href);Object.assign(location,{href:u.href,search:u.search,hostname:u.hostname});}},
    Date:{now:()=>clock},Math:{random:()=>0.5},console:{log(){}},
    sessionStorage:{getItem:k=>storage.get(k)??null,setItem:(k,v)=>storage.set(k,v)},
    navigator:{sendBeacon(url){network.push({url,method:'POST',headers:{},transport:'beacon',sent:beacon});return beacon;}},
    fetch(url,options){network.push({url,method:options.method,headers:{},transport:'fetch',keepalive:options.keepalive,sent:true});return Promise.resolve({});},
    setTimeout(fn,delay){timers.push({fn,delay});return timers.length;},
    addEventListener:(t,f)=>add('window:',t,f),
  };
  context.window=context;
  if(endpointBase)context.MobkoiTagConfig={endpointBase};
  if(consent!==null)context.__tcfapi=(name,version,callback)=>{
    if(name==='addEventListener')callback({eventStatus:'tcloaded',listenerId:1,vendor:{consents:{898:consent}},purpose:{consents:{1:consent}}},true);
  };
  vm.runInNewContext(source,context,{timeout:1000,contextCodeGeneration:{strings:false,wasm:false}});
  const emit=(type,event={})=>{for(const fn of listeners[type]||[])fn(event);};
  const tick=ms=>{clock+=ms;};
  const flushZero=()=>{for(let i=0;i<timers.length;i++)if(timers[i].delay===0)timers.splice(i--,1)[0].fn();};
  return {network,context,document,location,emit,tick,flushZero};
}
const getQuery=event=>Object.fromEntries(new URL(event.url).searchParams);
const first=run();assert.equal(first.network.length,2);
assert.deepEqual(getQuery(first.network[0]),{deid:'rsi',iid:'synthetic/click+1'});
assert.equal(getQuery(first.network[1]).isl,'true');
first.tick(250);first.document.visibilityState='hidden';first.emit('document:visibilitychange');first.flushZero();
assert.equal(getQuery(first.network.at(-1)).sr,'focus_lost');assert.equal(getQuery(first.network.at(-1)).top,'250');
first.tick(100);first.document.visibilityState='visible';first.emit('document:visibilitychange');
assert.equal(getQuery(first.network.at(-1)).rt,'tab_return');assert.equal(getQuery(first.network.at(-1)).isl,'false');
first.tick(30);first.context.history.pushState({},'','/next');
assert.equal(getQuery(first.network.at(-1)).rt,'spa_history_change');
assert.equal(getQuery(first.network.at(-2)).sr,'page_close');
const outside=run();outside.tick(40);outside.emit('document:click',{target:{closest(){return {href:'https://outside.example/',target:'_self',hostname:'outside.example'};}}});outside.emit('window:pagehide');
assert.equal(getQuery(outside.network.at(-1)).sr,'click_outside');
const resumed=run({mbiid:null,stored:{mbiid:'synthetic-session',traffic_scope:'mobkoi',page_count:3}});
assert.equal(resumed.network.length,1);assert.equal(getQuery(resumed.network[0]).isl,'false');
const fallback=run({beacon:false});assert.equal(fallback.network.filter(e=>e.transport==='fetch').length,2);
assert.ok(fallback.network.filter(e=>e.transport==='fetch').every(e=>e.method==='POST'&&e.keepalive===true));
assert.equal(run({mbiid:null}).network.length,0);
assert.equal(run({consent:false}).network.length,0);
assert.equal(run({consent:true}).network.length,2);
assert.ok(run({endpointBase:'https://configured.example'}).network.every(e=>new URL(e.url).host==='configured.example'));
const captures=[...first.network,...outside.network.slice(-1),...resumed.network,...fallback.network.filter(e=>e.sent)];
const output={sdk_url:'https://one-tag.mobkoi.com/tag.min.js',sdk_sha256:digest,observed_sdk_version:'1.0.0',external_network_calls:0,
  verified:['new-session','landing-page','tab-return','spa-history','focus-lost','outside-click','page-close','stored-session-resume','sendBeacon-false-fetch-fallback','no-attribution-no-send','TCF-denied-no-send','TCF-approved-send','configured-endpoint'],captures};
if(process.argv[4])writeFileSync(process.argv[4],JSON.stringify(output,null,2)+'\n');
console.log(JSON.stringify({sdk_sha256:digest,synthetic_captures:captures.length,external_network_calls:0,verified:output.verified}));
