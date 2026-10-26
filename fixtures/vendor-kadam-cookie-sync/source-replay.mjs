import fs from 'node:fs';
import crypto from 'node:crypto';
import path from 'node:path';
const [sourceDirectory, output] = process.argv.slice(2);
if (!sourceDirectory || !output) throw new Error('usage: node source-replay.mjs <source-directory> <output.json>');
const extraction=fs.readFileSync(path.join(sourceDirectory,'kadam-opened-primary.json'),'utf8');
const sha256=crypto.createHash('sha256').update(extraction).digest('hex');
if (sha256 !== '77d656bd86ff24f11b2244de94eeb814339c12beb9ea072efca1daff34cc0c2c') throw new Error('unexpected pinned primary extraction');
const text=JSON.parse(extraction).extraction;
// These are literal published templates, not a reconstructed receiver schema.
const templates=[
 'https://s.uuidksinc.net/match/{sspid}/?remote_uid={ssp-userid}',
 'https://d.uuidksinc.net/match/{dspid}/?remote_uid={dsp-userid}',
];
for (const template of templates) if (!text.includes(template)) throw new Error('published template missing');
const values=[['ordinary','partner-synthetic','user-synthetic'],['opaque','opaque%2Fpartner','opaque%2Buser'],['empty-user','partner-synthetic',''],['query-user','partner-synthetic','user&extra=opaque'],['fragment-user','partner-synthetic','user#fragment'],['slash-partner','partner/extra','user-synthetic'],['query-partner','partner?extra=opaque','user-synthetic']];
const results=[];
for (const template of templates) for (const [label,partner,user] of values) {
 const assigned=template.replace(/\{(?:ssp|dsp)id\}/,partner).replace(/\{(?:ssp|dsp)-userid\}/,user);
 const url=new URL(assigned);url.hash='';
 results.push({id:(template.includes('//s.')?'ssp-':'dsp-')+label,template,partner,user,assigned,url:url.href,source_kind:'literal primary-template substitution; not an SDK execution'});
}
fs.writeFileSync(output,JSON.stringify({source_url:'https://help.kadam.net/en/articles/16532646-cookie-matching',source_representation:'web opened-page extraction',source_sha256:sha256,network_calls:0,results},null,2)+'\n');
console.log(JSON.stringify({templates:templates.length,substitutions:results.length,network_calls:0}));
