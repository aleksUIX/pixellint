const fs = require('fs'); const vm = require('vm'); const crypto = require('crypto');
const source = fs.readFileSync(process.argv[2],'utf8');
if (crypto.createHash('sha256').update(source).digest('hex') !== '3e5bc7ca508f5d5aa7255341243840c8d2b70c8aea323800b51610df3f5a8a2c') throw Error('SDK hash mismatch');
const sent=[]; const listeners={}; const timers=[];
const location={href:'https://example.com/shop?campaign=test#details',origin:'https://example.com',protocol:'https:',host:'example.com',pathname:'/shop',search:'?campaign=test'};
const nav={entryType:'navigation',startTime:0,type:'navigate',deliveryType:'cache',nextHopProtocol:'h2',domainLookupStart:1,domainLookupEnd:2,connectStart:3,connectEnd:4,secureConnectionStart:3,requestStart:5,responseStart:6,responseEnd:7,domInteractive:8,domComplete:9,loadEventStart:10,loadEventEnd:11,transferSize:123,decodedBodySize:234};
const performance={now:()=>100,timeOrigin:1770000000000,timing:{navigationStart:1770000000000},getEntriesByType:t=>t==='navigation'?[nav]:t==='paint'?[{name:'first-paint',startTime:12},{name:'first-contentful-paint',startTime:13}]:[],memory:{usedJSHeapSize:42,totalJSHeapSize:64,jsHeapSizeLimit:128}};
const document={readyState:'complete',referrer:'https://referrer.example/path?secret=test',location,currentScript:{getAttribute:n=>n==='src'?'https://static.cloudflareinsights.com/beacon.min.js':n==='data-cf-beacon'?JSON.stringify({token:'test-site-token'}):null},addEventListener:(n,f)=>listeners[n]=f,visibilityState:'visible'};
class XHR {open(m,u){this.method=m;this.url=u;} setRequestHeader(n,v){this.headers={[n]:v};} send(body){sent.push({url:this.url,method:this.method,headers:this.headers,body:JSON.parse(body)});}}
const window={performance,document,location,history:{pushState(){}},addEventListener:(n,f)=>listeners[n]=f,setTimeout:f=>timers.push(f),crypto:{randomUUID:()=> '12345678-1234-4234-9234-123456789abc'}};
const context={window,document,navigator:{userAgent:'Mozilla/5.0 Chrome/120.0.0.0'},XMLHttpRequest:XHR,URL,URLSearchParams,Blob,Map,Set,WeakMap,Promise,Date,Math,console,setTimeout:f=>timers.push(f),crypto:window.crypto};
vm.runInNewContext(source,context); for(const f of timers.splice(0)) f();
fs.writeFileSync(process.argv[3],JSON.stringify(sent,null,2)+'\n'); console.log(JSON.stringify({emitted:sent.length,fields:sent.map(s=>Object.keys(s.body))}));
