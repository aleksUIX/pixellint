import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';

const fixture = JSON.parse(readFileSync(new URL('./compression-oracle.json', import.meta.url)));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
for (const entry of fixture.cases) {
  const wire = Buffer.from(entry.body_base64, 'base64');
  assert.equal(wire.toString('base64'), entry.body_base64, entry.id);
  assert.equal(wire.length, entry.wire_bytes, entry.id);
  assert.equal(sha(wire), entry.wire_sha256, entry.id);
  const entity = entry.gzip ? gunzipSync(wire) : wire;
  assert.equal(entity.length, entry.entity_bytes, entry.id);
  assert.equal(sha(entity), entry.entity_sha256, entry.id);
}
console.log(JSON.stringify({status: 'passed', cases: fixture.cases.length, oracle: 'Node zlib and crypto independently verified Python-produced bytes'}));
