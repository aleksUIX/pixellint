import {readFileSync, writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import vm from 'node:vm';

const sourcePath = process.argv[2];
const outputPath = process.argv[3];
const source = readFileSync(sourcePath, 'utf8');
const hash = value => createHash('sha256').update(value).digest('hex');
const sections = {};
function section(name, start, end, transform = value => value) {
  const a = source.indexOf(start);
  const b = source.indexOf(end, a + start.length);
  if (a < 0 || b < 0) throw new Error(`Missing pinned source section: ${name}`);
  const literal = source.slice(a, b);
  sections[name] = {start_byte: a, end_byte: b, sha256: hash(literal), literal};
  return transform(literal);
}
const query = section('getURLParam', 'this.getURLParam=function', ',this.getDomainUrlParam=', x => x.replace('this.getURLParam=', ''));
const banner = section('createBannerQueryParameters', 'createBannerQueryParameters=function', ',createBanner=', x => x.replace('createBannerQueryParameters=', ''));
const fragments = [
  section('blockingmode', 'var px2=this.getURLParam("blockingmode")', ';var polx='),
  section('framework', 'var pfx=this.getURLParam("framework")', ';var pfy='),
  section('culture', 'var puc=this.getURLParam("culture")', ';var pcm='),
  section('consentmode', 'var pcm=this.getURLParam("consentmode")', ';var paramMsConsentMode='),
  section('consentmode-dataredaction', 'var cmdr=this.getURLParam("consentmode-dataredaction")', ',this.dataLayerName='),
  section('framework-normalization', 'var lowercaseFramework=this.framework.toLowerCase()', ',this.frameworkBlocked&&')
];
const definitions = [
  ['omitted', '', {}],
  ['blank-fallback', '?type=&level=&framework=&blockingmode=&consentmode=&consentmode-dataredaction=&culture=', {'data-type':'optout','data-level':'strict','data-culture':'es','data-consentmode-dataredaction':'false'}],
  ...['optin','optout','optinout','leveloptin','inlineoptin','optionaloptin'].map(x => [`type-${x}`, `?type=${x}`, {}]),
  ...['implied','strict'].map(x => [`level-${x}`, `?level=${x}`, {}]),
  ...['IAB','iab1','IabV2','TCFv2.2','tcfv2.3','TcF'].map(x => [`framework-${x.toLowerCase()}`, `?framework=${x}`, {}]),
  ['automatic-uppercase', '?blockingmode=AUTO&consentmode=DISABLED', {}],
  ['manual-uppercase', '?blockingmode=NONE', {}],
  ...['true','false','dynamic'].map(x => [`redaction-${x}`, `?consentmode-dataredaction=${x}`, {}]),
  ...['PT-BR','ZH-HANT','haw','x-custom'].map(x => [`culture-${x.toLowerCase()}`, `?culture=${x}`, {}]),
  ['query-overrides-attribute', '?type=optin&level=implied&culture=EN', {'data-type':'optout','data-level':'strict','data-culture':'es'}],
  ['first-duplicate-wins', '?blockingmode=none&blockingmode=auto&culture=PT-BR&culture=ZH-HANT', {}],
  ['valid-then-invalid-duplicate', '?blockingmode=auto&blockingmode=future', {}],
  ['invalid-then-valid-duplicate', '?blockingmode=future&blockingmode=auto', {}],
  ['bare-keys-fall-back', '?type&level&blockingmode&consentmode&consentmode-dataredaction&framework&culture', {'data-type':'optin','data-level':'strict','data-culture':'es'}],
  ['encoded-key-not-read', '?%62lockingmode=auto', {}],
  ['data-framework-query-not-read', '?data-framework=TCF', {}],
  ['unknown-pass-through', '?type=future&level=future&framework=future&blockingmode=future&consentmode=future&culture=custom', {}],
  ['invalid-redaction-logs', '?consentmode-dataredaction=TRUE', {}]
];
const cases = definitions.map(([id, suffix, attributes]) => {
  const script = {src:`https://consent.cookiebot.com/uc.js?cbid=11111111-2222-3333-4444-555555555555${suffix.replace(/^\?/, '&')}`, getAttribute: key => attributes[key] ?? null};
  const logs = [];
  const context = vm.createContext({document:{getElementById:() => script}, getDomainSearchParam:()=>null, console:{warn:(...args)=>logs.push(args)}});
  const getURLParam = vm.runInContext(`(${query})`, context);
  const createParameters = vm.runInContext(`(${banner})`, context);
  const instance = {scriptId:'Cookiebot',scriptElement:script,getURLParam,framework:'',hasFramework:false,frameworkBlocked:false,autoblock:false,computedConfiguration:{},advertiserConsentModeEnabled:true,amznConsentSignalEnabled:false,consentModeDisabled:false,consentModeDataRedaction:'dynamic'};
  context.d = script;
  const apply = vm.runInContext(`(function(){${fragments.join(';')};})`, context);
  apply.call(instance);
  return {id, url:script.src, attributes, banner_parameters:JSON.parse(JSON.stringify(createParameters(instance,script))), settings:{framework:instance.framework,hasFramework:instance.hasFramework,autoblock:instance.autoblock,consentModeDisabled:instance.consentModeDisabled,consentModeDataRedaction:instance.consentModeDataRedaction,userCulture:instance.userCulture ?? null},logs};
});
writeFileSync(outputPath, JSON.stringify({source_url:'https://consent.cookiebot.com/uc.js',source_version:source.split('\n')[0],source_sha256:hash(source),execution:'Extracted unchanged source functions and statements, synthetic script element, VM without network APIs. No script insertion, image, beacon or fetch.',sections,cases},null,2)+'\n');
console.log(JSON.stringify({cases:cases.length,source_sha256:hash(source),sections:Object.keys(sections)}));
