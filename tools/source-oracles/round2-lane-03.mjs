import fs from 'node:fs';
import vm from 'node:vm';
import crypto from 'node:crypto';
const [rtbPath, wcmPath, ttdPath, outputPath] = process.argv.slice(2);
if (!outputPath) throw new Error('Pass pinned RTB, WCM, TTD source files and an output path. No network is used.');
const readPinned = (file, expected) => {
  const bytes=fs.readFileSync(file);
  const sha256=crypto.createHash('sha256').update(bytes).digest('hex');
  if (sha256!==expected) throw new Error(`Source hash mismatch: ${file}`);
  return {text:bytes.toString('utf8').replace(/^\uFEFF/,''),sha256};
};
const rtb=readPinned(rtbPath,'a31899759ba794a1928e8c83cd016af8b303a174c12d4b7ccc958858910a9a5c');
const wcm=readPinned(wcmPath,'988281412a86dddce13e30b5e2a5a15b05e9e0e7f0caa7be2137ecd7f4fe3a9b');
const ttd=readPinned(ttdPath,'550aff06ea965bbb7be403e1a62d4a70ba197f21f65c3f88866deb4f89c35104');
const rtbCode=rtb.text.split('___SANDBOXED_JS_FOR_WEB_TEMPLATE___')[1].split('___WEB_PERMISSIONS___')[0];
const fixedMillis=1792886400000;
function rtbOutput(input) {
  const urls=[];const storage=new Map(input.storedLid===undefined?[]:[['__rtbhouse.lid',input.storedLid]]);
  const values={injectHiddenIframe:(url,done)=>{urls.push(url);done()},encodeUriComponent:encodeURIComponent,makeString:String,localStorage:{getItem:k=>storage.get(k),setItem:(k,v)=>storage.set(k,v)},Math,getReferrerUrl:()=>input.referrer??'',getUrl:()=>input.page??'',getTimestampMillis:()=>input.timestamp??fixedMillis,generateRandom:()=>0};
  const sandbox={data:{advTaggingHash:'SYNTHETIC00000000000',advRegion:'ams',...input.data,gtmOnSuccess(){}},require:name=>{if (!(name in values)) throw new Error(`Unsupported GTM API: ${name}`);return values[name]}};
  vm.runInNewContext(rtbCode,sandbox,{timeout:1000});
  if(urls.length!==1)throw new Error('Expected one captured RTB iframe URL.');
  return {url:new URL(urls[0]).href,constructor_url:urls[0]};
}
const rtbInputs=[
 ['source-home-ams',{data:{tagHome:true},page:'https://publisher.example/article',referrer:'https://referrer.example/'}],
 ['source-many-us',{data:{advRegion:'us',tagHome:true,tagOffer:true,offerOfferId:'offer:opaque',tagUid:true,uid:'synthetic-user'}}],
 ['source-custom-asia',{data:{advRegion:'asia',tagCustom:true,customTagsTable:[{customTagName:'open_name',customTagValue:'opaque_value,42'},{customTagName:'empty_value',customTagValue:''}]}}],
 ['source-empty-context',{data:{tagBasketStatus:true,basketStatusOfferIds:'sku-a,sku-b'},page:'',referrer:''}],
 ['source-opaque-stored-lid',{data:{tagHome:true},storedLid:'____________________'}],
 ['source-zero-timestamp',{data:{tagStartOrder:true},timestamp:0}],
 ['source-other-ncm',{data:{tagOther:true}}],
 ['source-orderstatus-default',{data:{tagOrderStatus:true,orderStatusValue:'12,50',orderStatusOrderId:'order_with_under',orderStatusOfferIds:'sku-a,sku-b'}}],
 ['source-orderstatus-false',{data:{tagOrderStatus:true,orderStatusCustomDeduplicationCheck:true,orderStatusCustomDeduplication:false}}],
 ['source-orderstatus-true',{data:{tagOrderStatus:true,orderStatusCustomDeduplicationCheck:true,orderStatusCustomDeduplication:'other'}}],
 ['source-empty-offer',{data:{tagOffer:true,offerOfferId:''}}],
 ['source-lid-only',{data:{}}],
 ['source-custom-query-syntax',{data:{tagCustom:true,customTagsTable:[{customTagName:'free',customTagValue:'opaque&id='}]}}],
 ['source-custom-ts-collision',{data:{tagCustom:true,customTagsTable:[{customTagName:'free',customTagValue:'opaque&ts=custom'}]}}],
 ['source-custom-empty-ts-collision',{data:{tagCustom:true,customTagsTable:[{customTagName:'free',customTagValue:'opaque&ts='}]}}],
];
function wcmOutput(input) {
  const urls=[];let attempts=0;
  const location={href:'https://publisher.example/article',protocol:input.protocol??'https:',host:'publisher.example',hostname:'publisher.example',search:'',pathname:'/article',toString(){return this.href}};
  if(input.protocol==='http:')location.href=location.href.replace('https:','http:');
  function element(tag){return {tagName:tag.toUpperCase(),nodeName:tag.toUpperCase(),nodeType:1,checkVisibility(){return true},style:{},children:[],parentNode:{insertBefore(){},appendChild(){}},setAttribute(k,v){this[k]=v},getAttribute(k){return this[k]||null},appendChild(e){this.children.push(e);if(e.src)urls.push(e.src);return e},insertBefore(e){return this.appendChild(e)},getElementsByTagName(){return []},getBoundingClientRect(){return {width:300,height:250,left:0,top:0}},addEventListener(){},removeEventListener(){}}}
  const script=element('script');script.src='https://cstatic.weborama.fr/js/advertiserv2/adperf_launch_1.0.0_scrambled.js';
  const document={location,referrer:input.referrer??'',readyState:'complete',domain:'publisher.example',body:element('body'),documentElement:element('html'),scripts:[script],write(s){urls.push(...[...String(s).matchAll(/src=["']([^"']+)/g)].map(m=>m[1]))},writeln(s){this.write(s)},createElement:element,getElementsByTagName(tag){return tag==='script'?[script]:tag==='head'?[this.head]:[]},getElementById(){return null},addEventListener(){},removeEventListener(){}};document.head=element('head');
  class FixedDate extends Date {constructor(...args){super(...(args.length?args:[fixedMillis]))}static now(){return fixedMillis}}
  const fixedMath=Object.create(Math);fixedMath.random=()=>0.5;
  const sandbox={document,location,navigator:{userAgent:'SyntheticBrowser/1.0',appVersion:'SyntheticBrowser',appName:'Netscape',platform:'Synthetic',cookieEnabled:true},screen:{width:1920,height:1080},console:{log(){},error(){}},setTimeout(){return 1},clearTimeout(){},setInterval(){return 1},clearInterval(){},addEventListener(){},removeEventListener(){},escape,unescape,encodeURIComponent,decodeURIComponent,URL,Math:fixedMath,Date:FixedDate,adperfobj:{account_id:1111,tracking_element_id:2222,width:300,height:250,fullhost:'wcmvideotutorials.solution.weborama.fr',random:String(fixedMillis),burst:'auto',imptrackers:[],clicktrackers:[],publisherclick:'',...input.data},Image:function(){return element('img')},Element:function(){},XMLHttpRequest:function(){attempts++;throw new Error('network disabled')},fetch(){attempts++;throw new Error('network disabled')},frames:{},getComputedStyle(){return {getPropertyValue(){return '300px'}}}};
  sandbox.window=sandbox;sandbox.top=sandbox;sandbox.self=sandbox;sandbox.parent=sandbox;
  vm.runInNewContext(wcm.text,sandbox,{timeout:1000});
  const captured=urls.filter(url=>url.includes('/fcgi-bin/dispatch.fcgi?'));
  if(captured.length!==1||attempts!==0)throw new Error('Expected one captured WCM dispatch and zero network attempts.');
  return {url:new URL(captured[0]).href,constructor_url:captured[0],network_attempts:attempts};
}
const wcmInputs=[
 ['source-guide-default',{data:{}}],
 ['source-opaque-identifiers',{data:{account_id:'synthetic-account',tracking_element_id:'synthetic:element'}}],
 ['source-empty-identifiers',{data:{account_id:'',tracking_element_id:''}}],
 ['source-zero-identifiers',{data:{account_id:0,tracking_element_id:0}}],
 ['source-optional-flags',{data:{is_mobile:true,previewmode:true,weak_encoding:false,restrict:true}}],
 ['source-optional-prices',{data:{price_paid:'12.50',bid_price:'15.0',publisherclick:'https://publisher.example/click?token=synthetic'}}],
 ['source-nonnumeric-prices',{data:{price_paid:'not-a-number',bid_price:'Infinity'}}],
 ['source-http-referrer',{protocol:'http:',referrer:'http://referrer.example/',data:{}}],
 ['source-account-query-collisions',{data:{account_id:'opaque&g.de=2&g.ism=3&a.we=4'}}],
 ['source-tracking-query-collisions',{data:{tracking_element_id:'opaque&g.de=2&g.ism=3&a.we=4'}}],
];
const tagPattern=ttd.text.match(/`\/\/insight\.adsrvr\.org\/track\/\{TAG_TYPE\}\/`/);
const queryPattern=ttd.text.match(/`adv=ADVERTISER_ID_PLACEHOLDER&ct=0:TRACKING_TAG_ID_PLACEHOLDER&fmt=3`/);
if(!tagPattern||!queryPattern)throw new Error('Published TTD static template changed.');
const template='https:'+tagPattern[0].slice(1,-1)+'?'+queryPattern[0].slice(1,-1);
const ttdUrl=kind=>template.replace('{TAG_TYPE}',kind).replace('ADVERTISER_ID_PLACEHOLDER','synthetic-advertiser').replace('TRACKING_TAG_ID_PLACEHOLDER','synthetic:tag-id');
const ttdCases=['pxl','conv','envt','imp'].map(kind=>({id:`source-${kind}-template`,input:{tag_type:kind},url:ttdUrl(kind)}));
const dynamic=ttd.text.match(/&v=\{REVENUE\}&vf=\{CURRENCY\}&orderid=\{ORDER_ID\}&td1=\{VALUE_1\}[^`\n]*/)[0];
const replacements={REVENUE:'12.99',CURRENCY:'GBP',ORDER_ID:'synthetic-order'};
for(let i=1;i<=10;i++)replacements[`VALUE_${i}`]=`synthetic-${i}`;
ttdCases.push({id:'source-dynamic-template',input:replacements,url:ttdUrl('pxl')+dynamic.replace(/\{([^}]+)\}/g,(_,key)=>encodeURIComponent(replacements[key]))});
const output={network_policy:'No network APIs available. Iframe/script emissions captured as strings only.',reference_time:fixedMillis/1000,sources:{rtb:{commit:'a0d161cf4a144ff11bdcaae218f03f27c9ca2197',sha256:rtb.sha256},wcm:{version:'1.0.0.80.6',sha256:wcm.sha256},ttd:{sha256:ttd.sha256}},rtb:rtbInputs.map(([id,input])=>({id,input,...rtbOutput(input)})),wcm:wcmInputs.map(([id,input])=>({id,input,...wcmOutput(input)})),ttd:ttdCases};
fs.writeFileSync(outputPath,JSON.stringify(output,null,2)+'\n');
console.log(JSON.stringify({rtb:output.rtb.length,wcm:output.wcm.length,ttd:output.ttd.length,output:outputPath}));
