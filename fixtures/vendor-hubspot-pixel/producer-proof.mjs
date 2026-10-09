import vm from 'node:vm';
import fs from 'node:fs';
import crypto from 'node:crypto';
const source = fs.readFileSync(process.argv[2], 'utf8');
const sha256 = crypto.createHash('sha256').update(source).digest('hex');
if (sha256 !== '93fa9b91813275827f7cc6fbda22c1b81e25893d48f8e88abf2dd5efbb3d44e4') throw new Error('SDK differs from reviewed source');
function extract(prefix,name,next) {
  const start = source.indexOf(prefix+name+'=function');
  const end = source.indexOf(next,start+prefix.length+name.length+10);
  if (start < 0 || end < 0) throw new Error(`Missing source function ${name}`);
  return source.slice(start,end+1);
}
const utilities = ['isEmpty','isDefined','isArray','isFunction','safeString','makeLowerCase','param','encodeParam','extend','utcnow','resolveInitialUrl','resolveReferrer'];
const methods = ['_getClientInfo','_getPageInfo','_getUserInfo','_getPrivacyInfo','_generateURL','_determineTrackingDomain','_loadImage','trackPageView','trackCustomBehavioralEvent','trackClickEvent','_trackFormActivity','trackFormView','trackFormInstall','trackFormVisible','trackFormInteraction','trackFormCompletion','trackFeedbackView','trackCtaView','trackAudioPlay','trackApproveCookieConsent','trackDeclineCookieConsent','trackRevokeCookieConsent'];
const requests = [];
class FixedDate extends Date { constructor(...args) { super(...(args.length ? args : [1800000000000])); } }
const context = {Date:FixedDate,URL,requests};
context.window = {location:{origin:'https://publisher.example'}};
context.hstc = {JS_VERSION:1.1, ANALYTICS_HOST:'track.hubspot.com', log:()=>{}, utils:{tostr:Object.prototype.toString,
  each:(array,fn)=>array.forEach(value=>fn.call(value)), parseURL:(value,base)=>{try{return new URL(value,base)}catch{return null}},
  loadImage:(url)=>requests.push({url,method:'GET',headers:{}})},
  tracking:{Tracker:function(){},Utk:{COOKIE:'hubspotutk'},Session:{COOKIE:'__hssc'}}};
vm.createContext(context);
for (const name of utilities) vm.runInContext(extract('hstc.utils.',name,';hstc.utils.'),context,{timeout:1000});
for (const name of methods) vm.runInContext(extract('hstc.tracking.Tracker.prototype.',name,';hstc.tracking.Tracker.prototype.'),context,{timeout:1000});
const state = {
  portalId:12345678, pageId:'synthetic-page',contentType:'standard-page',canonicalUrl:'https://publisher.example/canonical',
  path:'/current',referrerPath:'/referrer',session:{viewCount:2},
  contentMetadata:{contentPageId:'synthetic-page',contentGroupId:'synthetic-group',contentFolderId:'synthetic-folder',legacyPageId:'synthetic-legacy',
    abTestId:'synthetic-ab',languageVariantId:'synthetic-language-variant',languageCode:'en-gb',
    mabData:{correlationId:'synthetic-correlation',experimentId:'synthetic-experiment'},scpContentType:'synthetic-content',inChatView:true},
  targetedContentMetadata:Array.from({length:7},(_,i)=>['synthetic-target'+i,'synthetic-variant','synthetic-row']),
  hasResetVisitor:true, identity:{get:()=>({email:'visitor+tag@example.org',firstname:'café + 50%',custom:'東京'})},
  utk:{visitor:'0123456789abcdef0123456789abcdef',isNew:()=>true},
  cookie:{get:(name)=>name==='hubspotutk'?'0123456789abcdef0123456789abcdef':name==='__hssc'?'1.2.1800000000000':null},
  privacySettings:{mode:'COOKIES_BY_CATEGORY'},privacyConsent:{allowed:false,categories:{necessary:true,analytics:true,advertisement:false,functionality:false}},
  trackingEnabled:true,limitTrackingToCookieDomains:false,_manageCookies:()=>{},_hasDoNotTrack:()=>false,_getConsentedFingerprint:()=>null,
  context:{getScreen:()=>({width:1920,height:1080,colorDepth:24}),getCharacterSet:()=> 'UTF-8',getNavigator:()=>({language:'en-GB'}),
    getReferrer:()=> 'https://referrer.example/path',getLocation:()=>({href:'https://publisher.example/current'}),getDocument:()=>({title:'Synthetic café + title'})}
};
const tracker = new context.hstc.tracking.Tracker();Object.assign(tracker,state);context.tracker=tracker;
const cases = [];
function capture(id,method,...args) {
  requests.length=0;tracker[method](...args);
  if (requests.length !== 1) throw new Error(`Unexpected request count for ${id}`);
  cases.push({id,method,args,request:requests[0]});
}
capture('sdk-pageview-rich','trackPageView');
capture('sdk-custom-event','trackCustomBehavioralEvent',{name:'pe12345678_my_event',properties:{property_name:'property_value',brackets:['one','two'],opaque:{x:true}}});
tracker.privacyConsent.allowed=true;capture('sdk-click','trackClickEvent',{properties:{hs_element_text:'café + click',hs_tag:'A'}});tracker.privacyConsent.allowed=false;
for (const method of ['trackFormView','trackFormInstall','trackFormVisible','trackFormInteraction','trackFormCompletion']) capture('sdk-'+method,method,'synthetic-form','synthetic-conversion',{formVariantId:'synthetic-variant',leadFlowId:'synthetic-lead',formType:0});
capture('sdk-feedback','trackFeedbackView',{surveyType:'synthetic-type',surveyId:'synthetic-survey'});
capture('sdk-feedback-empty','trackFeedbackView');
capture('sdk-cta','trackCtaView','synthetic-cta','synthetic-variant');
capture('sdk-cta-empty','trackCtaView');
capture('sdk-audio','trackAudioPlay',{fileId:'synthetic-file',episodeId:'synthetic-episode',showId:'synthetic-show',moduleType:'synthetic-module',duration:2.5,playSessionId:'synthetic-session',eventPhase:'synthetic-phase'});
capture('sdk-audio-negative-duration','trackAudioPlay',{duration:-2.5});
capture('sdk-audio-nan-duration','trackAudioPlay',{duration:NaN});
for (const method of ['trackApproveCookieConsent','trackDeclineCookieConsent','trackRevokeCookieConsent']) capture('sdk-'+method,method);
tracker.identity={get:()=>({email:['one@example.org','two+tag@example.org'],visitor:'synthetic-visitor'})};capture('sdk-repeated-identity','trackPageView');
console.log(JSON.stringify({source_url:'https://js.hs-analytics.net/analytics/1791543000000/53.js',source_sha256:sha256,
  source_build:'1.4506',fetched_at:'2026-10-09',execution:'Exact published pure SDK functions, synthetic tracker state, image loading captured, no network',
  extracted_utilities:utilities,extracted_methods:methods,cases},(_key,value)=>typeof value === 'number' && !Number.isFinite(value) ? {javascript_number:String(value)} : value,2));
