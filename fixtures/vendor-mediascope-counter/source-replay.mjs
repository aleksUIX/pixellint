import fs from 'node:fs';
import crypto from 'node:crypto';
import vm from 'node:vm';
import path from 'node:path';

// Executes only the pinned public producer. Image assignments are intercepted.
const [sourceDirectory, output] = process.argv.slice(2);
if (!sourceDirectory || !output) throw new Error('usage: node source-replay.mjs <source-directory> <output.json>');
const sdk = fs.readFileSync(path.join(sourceDirectory, 'tcounter.js'), 'utf8');
const guide = fs.readFileSync(path.join(sourceDirectory, 'mediascope-help.html'), 'utf8');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
if (hash(sdk) !== '92a82d0233445685062df7115e244b34f3e71657d0c80f54cce716b5952eb8dc') throw new Error('unexpected producer');
if (hash(guide) !== 'aae8693233b38c5d22576c74dc08beed60604d3d32d2e310eacec843cd6cc754') throw new Error('unexpected guide');
const scenarios = [
  {id:'sdk-default', referrer:'https://ref.example/path'},
  {id:'sdk-http-context',base:'http://page.example/path'},
  {id:'sdk-empty-referrer',referrer:''},
  {id:'sdk-default-star-escaped',referrer:'https://ref.example/a*b'},
  {id:'sdk-document-query-boundary',referrer:'https://ref.example/?q=synthetic'},
  {id:'sdk-document-fragment-boundary',referrer:'https://ref.example/#synthetic'},
  {id:'sdk-referrer-encoded-delimiters',referrer:'https://ref.example/a%3Fb%23c'},
  {id:'sdk-random-zero',random:0},
  {id:'sdk-random-upper',random:0.9999999999},
  {id:'sdk-autohit-false',options:{autohit:false}},
  {id:'sdk-autohit-null',options:{autohit:null}},
  {id:'sdk-autohit-zero',options:{autohit:0}},
  {id:'sdk-autohit-empty',options:{autohit:''}},
  {id:'sdk-hit-after-disabled',options:{autohit:false},hits:[[]]},
  {id:'sdk-hit-null-fallbacks',options:{autohit:false},hits:[[null,null,null,null]]},
  {id:'sdk-hit-zero-fallbacks',options:{autohit:false,param1:'prefix'},hits:[[0,'ignored',0,0]]},
  {id:'sdk-hit-section',options:{autohit:false},hits:[['site_news-sport']]},
  {id:'sdk-hit-url-ignored-a',options:{autohit:false},hits:[[null,'https://one.example/']]},
  {id:'sdk-hit-url-ignored-b',options:{autohit:false},hits:[[null,'https://two.example/']]},
  {id:'sdk-hit-explicit-star',options:{autohit:false},hits:[[null,null,'https://ref.example/a*b']]},
  {id:'sdk-hit-explicit-query',options:{autohit:false},hits:[[null,null,'https://ref.example/?q=synthetic']]},
  {id:'sdk-hit-explicit-fragment',options:{autohit:false},hits:[[null,null,'https://ref.example/#synthetic']]},
  {id:'sdk-prefix-text',options:{param1:'opaque-prefix'}},
  {id:'sdk-prefix-number',options:{param1:42}},
  {id:'sdk-prefix-boolean',options:{param1:true}},
  {id:'sdk-prefix-array',options:{param1:['opaque','prefix']}},
  {id:'sdk-prefix-object',options:{param1:{opaque:true}}},
  {id:'sdk-prefix-star-boundary',options:{param1:'opaque*prefix'}},
  {id:'sdk-prefix-query-boundary',options:{param1:'opaque?prefix'}},
  {id:'sdk-prefix-fragment-boundary',options:{param1:'opaque#prefix'}},
  {id:'sdk-account-uppercase',options:{account:'Site_ru'}},
  {id:'sdk-account-hyphen',options:{account:'site-ru'}},
  {id:'sdk-account-slash-boundary',options:{account:'site/ru'}},
  {id:'sdk-account-query-boundary',options:{account:'site?ru'}},
  {id:'sdk-account-fragment-boundary',options:{account:'site#ru'}},
  {id:'sdk-section-extra-underscore',options:{tmsec:'site_news_sport'}},
  {id:'sdk-section-uppercase',options:{tmsec:'site_News'}},
  {id:'sdk-section-slash-boundary',options:{tmsec:'site_news/sport'}},
  {id:'sdk-section-query-boundary',options:{tmsec:'site_news?sport'}},
  {id:'sdk-section-fragment-boundary',options:{tmsec:'site_news#sport'}},
  {id:'sdk-section-empty',options:{tmsec:''}},
  {id:'sdk-account-empty',options:{account:''}},
  {id:'sdk-missing-config',missingConfig:true},
];
const results = [];
for (const scenario of scenarios) {
  const captures = [];
  const denied = () => { throw new Error('network attempt'); };
  const math = Object.create(Math);
  math.random = () => scenario.random ?? 0.123456789;
  function Image() {}
  Object.defineProperty(Image.prototype, 'src', {set(raw) {
    const url = new URL(String(raw), scenario.base ?? 'https://page.example/path');
    // HTTP fragments are not sent to the receiver.
    url.hash = '';
    captures.push({assigned:String(raw),url:url.href,method:'GET',headers:{},body_absent:true});
  }});
  const sandbox = {document:{referrer:scenario.referrer ?? 'https://ref.example/path'},Image,Math:math,fetch:denied,XMLHttpRequest:denied,navigator:{sendBeacon:denied}};
  vm.runInNewContext(sdk, sandbox, {timeout:1000,filename:'tcounter.js'});
  const options = scenario.missingConfig ? {} : {account:'site_ru',tmsec:'site_main',...scenario.options};
  const counter = new sandbox.TNS.TnsCounter(options);
  for (const args of scenario.hits ?? []) counter.hit(...args);
  results.push({id:scenario.id,scenario,captures});
}
// The guide HTML contains syntax-highlighted examples. Strip markup only.
const plain = guide.replace(/<[^>]*>/g,'').replaceAll('&lt;','<').replaceAll('&gt;','>').replaceAll('&amp;','&').replaceAll('&quot;','"').replaceAll('&#39;',"'");
const standard = plain.match(/\(new Image\(\)\)\.src\s*=\s*'\/\/www\.tns-counter\.ru\/V13a\*\*\*R>'[^;]+;/)?.[0];
if (!standard) throw new Error('standard producer not found in guide');
for (const [id,referrer] of [['guide-standard','https://ref.example/path'],['guide-standard-query-boundary','https://ref.example/?q=synthetic'],['guide-standard-stars','https://ref.example/a*b']]) {
  const captures=[]; function Image() {}
  Object.defineProperty(Image.prototype,'src',{set(raw) {const u=new URL(raw,'https://page.example/');u.hash='';captures.push({assigned:raw,url:u.href,method:'GET',headers:{},body_absent:true});}});
  const math=Object.create(Math);math.random=()=>0.123456789;
  vm.runInNewContext(standard,{document:{referrer},Image,Math:math},{timeout:1000});
  results.push({id,source:'unchanged guide standard Image.src expression',captures});
}
const proof={source_url:'https://www.tns-counter.ru/tcounter.js',source_sha256:hash(sdk),guide_sha256:hash(guide),standard_expression:standard,standard_expression_sha256:hash(standard),network_calls:0,scenario_count:results.length,capture_count:results.reduce((n,x)=>n+x.captures.length,0),results};
fs.writeFileSync(output,JSON.stringify(proof,null,2)+'\n');
console.log(JSON.stringify({scenarios:proof.scenario_count,captures:proof.capture_count,network_calls:0}));
