import assert from 'node:assert/strict';
import { readFileSync,writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import vm from 'node:vm';

const source=readFileSync(process.argv[2],'utf8');
const expected='8acdd9de15825f4e55eecac4172296c459fcbec793c3fbb0ba593552f1c18b54';
assert.equal(createHash('sha256').update(source).digest('hex'),expected,'Public runtime source changed after review');
function slice(start,end) {
  const a=source.indexOf(start);assert.ok(a>=0,start);
  const b=source.indexOf(end,a);assert.ok(b>a,end);
  return source.slice(a,b);
}
const ctor=slice('Input_0$classdecl$var9=function(){','\nInput_0$classdecl$var9.prototype.initConfig=');
const builder=slice('Input_0$classdecl$var9.prototype.getErrorURL=function','Input_0$classdecl$var9.prototype.getRandomNumber=');
const transport=slice('Input_0$classdecl$var8.prototype.execute=function','\nvar EventHandler=new Input_0$classdecl$var8');
const errorFire=slice('Input_0$classdecl$var7.prototype.firePubMaticEvents=function','\nInput_0$classdecl$var7.prototype.fire3rdPartyEvents=');
for(const literal of ['this.SSP_TRACKER_DOMAIN="st.pubmatic.com"','this.V_CAMPAIGN_NETWORK_ID=243','this.VIDEO_CLIENT="vc"','this.EMPTY_VAST_EVENT=95'])assert.ok(source.includes(literal));
const pieces={constructor:ctor,builder,transport,errorFire};
const hashes=Object.fromEntries(Object.entries(pieces).map(([k,v])=>[k,createHash('sha256').update(v).digest('hex')]));
async function run({config={},networkId=0,campaignId=0,event,clock=1793000000000,renderError=null}={}) {
  const requests=[];
  class FixedDate extends Date {constructor(...args){super(...(args.length?args:[clock]));}}
  const context={Date:FixedDate,Math,Promise,Constants:{SSP_TRACKER_DOMAIN:'st.pubmatic.com',V_CAMPAIGN_NETWORK_ID:243,VIDEO_CLIENT:'vc',AMP_OP:'&',EQUAL_OP:'='},
    Logger:{warn(){},debug(){},error(){}},
    StringUtils:{isBlank:v=>v===null||v===undefined||String(v).trim()==='',getSecuredURL:v=>v.replace(/^http:/,'https:')},
    GeneralUtils:{timestampMacroReplacement:v=>v},
    document:{createElement(type){assert.equal(type,'img');return {};},body:{appendChild(image){requests.push({url:image.src,method:'GET',headers:{}});image.onload();}}},
    Input_0$classdecl$var8:function(){},Input_0$classdecl$var7:function(){},
  };
  vm.createContext(context);
  vm.runInContext(ctor+'\n'+builder+'\n'+transport+'\n'+errorFire,context,{timeout:1000});
  context.AdParamConfigReader=new context.Input_0$classdecl$var9();
  Object.assign(context.AdParamConfigReader,{pmTrackerDomain:'st.pubmatic.com'},config);
  context.EventHandler=new context.Input_0$classdecl$var8();
  const url=context.AdParamConfigReader.getErrorURL(networkId,campaignId,event);
  if(renderError===null)await context.EventHandler.execute([url]);
  else await new context.Input_0$classdecl$var7().firePubMaticEvents([url],{pmErrorCode:renderError});
  assert.equal(requests.length,1);return {builder_url:url,request:requests[0],config_after:{isAk:context.AdParamConfigReader.isAk,trackerDomain:context.AdParamConfigReader.DEFAULT_TRACKER_DOMAIN}};
}
const q=r=>Object.fromEntries(new URL(r.request.url).searchParams);
const cases=[];
async function add(id,options,check){const result=await run(options);check?.(result);cases.push({id,...result});}
await add('default-error-template',{},r=>{assert.equal(q(r).p,'0');assert.equal(q(r).vadsId,'-1');assert.equal(q(r).er,'[ERRORCODE]');assert.equal(q(r).isAk,'0');assert.equal(q(r).vc,'2');assert.ok(!('wc' in q(r)));assert.ok(!('pmc' in q(r)));});
await add('empty-vast-event',{event:95},r=>{assert.equal(q(r).e,'95');assert.ok(!('er' in q(r)));});
await add('campaign-network-zero-campaign',{networkId:243,campaignId:0},r=>assert.equal(q(r).wc,'0'));
await add('native-numeric-equivalent-network',{networkId:'243.0',campaignId:'opaque-campaign'},r=>assert.equal(q(r).wc,'opaque-campaign'));
await add('native-zero-identifiers',{config:{pubId:0,siteId:0,adId:0,adServingId:0}},r=>{for(const key of ['p','s','a','vadsId'])assert.equal(q(r)[key],'0');});
await add('native-empty-identifiers',{config:{pubId:'',siteId:'',adId:'',adServingId:''}},r=>{for(const key of ['p','s','a','vadsId'])assert.equal(q(r)[key],'');});
await add('opaque-identifiers',{config:{pubId:'publisher-a',siteId:'site-b',adId:'ad-c',adServingId:'serving-d'},networkId:'opaque-network'},r=>assert.equal(q(r).p,'publisher-a'));
await add('truthy-optional-zero-strings',{config:{crID:'0',ucrid:'0',imprId:'0',advertiserId:'0',ecpm:'0'}},r=>{for(const key of ['crId','ucrid','impid','advertiser_id','ecpm'])assert.equal(q(r)[key],'0');});
await add('falsy-native-optionals',{config:{crID:0,ucrid:null,imprId:'',advertiserId:false,ecpm:0}},r=>{for(const key of ['crId','ucrid','impid','advertiser_id','ecpm'])assert.ok(!(key in q(r)));});
await add('string-sentinel-consent',{config:{pubConsent:'-1'}},r=>assert.equal(q(r).pmc,'-1'));
await add('numeric-gated-isak',{config:{isAk:2}},r=>assert.equal(q(r).isAk,'2'));
await add('string-isak-omitted',{config:{isAk:'1'}},r=>assert.ok(!('isAk' in q(r))));
await add('unlisted-native-isak-omitted',{config:{isAk:3}},r=>assert.ok(!('isAk' in q(r))));
await add('override-domain-forced-back',{config:{DEFAULT_TRACKER_DOMAIN:'other.example'}},r=>{assert.equal(new URL(r.request.url).host,'st.pubmatic.com');assert.equal(q(r).isAk,'1');});
await add('zero-event-uses-error',{event:0},r=>assert.ok('er' in q(r)));
await add('string-zero-event-stays-open',{event:'0'},r=>assert.equal(q(r).e,'0'));
await add('custom-event-stays-open',{event:'future-event'},r=>assert.equal(q(r).e,'future-event'));
await add('zero-time',{clock:0},r=>assert.equal(q(r).ts,'0'));
await add('negative-time',{clock:-1000},r=>assert.equal(q(r).ts,'-1'));
await add('literal-error-rendered',{renderError:199},r=>assert.equal(q(r).er,'199'));
const output={source_url:'https://vpaid.pubmatic.com/ads/video/PMAdMgr.js',source_sha256:expected,extracted_function_sha256:hashes,external_network_calls:0,
  helper_limits:'Replay uses unmodified reviewed constructor, tracker builder, error macro substitution and image-transport methods. Logging and unrelated timestamp/HTTPS helpers are deterministic local stubs for synthetic inputs; no whole-runtime or remote acceptance claim.',cases};
if(process.argv[3])writeFileSync(process.argv[3],JSON.stringify(output,null,2)+'\n');
console.log(JSON.stringify({source_sha256:expected,extracted_functions:hashes,source_cases:cases.length,external_network_calls:0}));
