/** Reproduce primary template substitutions and a generic helper boundary without network. */
import fs from 'node:fs';
import crypto from 'node:crypto';
import vm from 'node:vm';
const [sourceDir, outputFile] = process.argv.slice(2);
if (!outputFile) throw new Error('Pass source directory and output file. No network APIs are provided.');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const metadata = JSON.parse(fs.readFileSync(`${sourceDir}/sourcepoint-article-review.json`, 'utf8'));
const template = fs.readFileSync(`${sourceDir}/sourcepoint-choice-template.txt`);
if (sha(template) !== metadata.template_sha256) throw new Error('Primary-template excerpt changed.');
const sdk = fs.readFileSync(`${sourceDir}/sourcepoint-sdk.source`);
const sdkSha = 'ed8637bc61dfdc56af42270023c828f72e9f501a7897fc071f91a3f531ecd605';
if (sha(sdk) !== sdkSha) throw new Error('SDK source changed.');
const sdkText = sdk.toString('utf8');
const start = sdkText.indexOf('t.appendQueryParams=function');
const stop = sdkText.indexOf(',t.setCookies=', start);
if (start < 0 || stop < 0) throw new Error('Generic helper boundaries changed.');
const exactHelper = sdkText.slice(start + 't.appendQueryParams='.length, stop);
const append = vm.runInNewContext(`(${exactHelper})`, {}, { timeout: 1000 });
const genericBase = 'https://synthetic.example/choice';
const helperCases = [
  { input: `${genericBase}?hasCsp=false`, output: append(`${genericBase}?hasCsp=false`, { env: 'prod' }) },
  { input: `${genericBase}?unrelated=hasCsp`, output: append(`${genericBase}?unrelated=hasCsp`, { env: 'stage' }) },
];
if (helperCases[0].output.includes('hasCsp=true')) throw new Error('Existing arbitrary values must remain preserved.');
const docUrl = template.toString('utf8').trim().replace(/^POST /, '');
const cases = [['source-prod', '11', 'prod', 'synthetic-request'], ['source-stage', '13', 'stage', 'synthetic-request'], ['source-future-choice', 'future-choice:opaque', 'prod', 'opaque:request']].map(([id, choice, env, request]) => ({
  id, capture: { url: docUrl.replace('{CHOICE_TYPE_ID}', encodeURIComponent(choice)).replace('{REQUEST_UUID}', encodeURIComponent(request)).replace('{ENVIRONMENT}', env), method: 'POST', headers: {} },
}));
const result = {
  source_url: metadata.url,
  template_sha256: sha(template),
  source_access_limit: metadata.access_status,
  method: 'Offline synthetic substitutions into the exact documentation excerpt. Separate unchanged generic SDK helper counterexamples do not establish a producer for this route.',
  engine_derived: false,
  network_calls: 0,
  cases,
  helper_scope: { sdk_sha256: sdkSha, version: '4.40.3', exact_route_not_present: !sdkText.includes('messages/choice'), callback_or_receiver_semantics_not_inferred: true, helper_cases: helperCases },
};
fs.writeFileSync(outputFile, JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ template_outputs: cases.length, generic_helper_controls: helperCases.length, network_calls: 0 }));
