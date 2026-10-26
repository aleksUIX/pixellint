import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const [indexPath, actionPath] = process.argv.slice(2);
if (!indexPath || !actionPath) throw new Error('Pass the two pinned source file paths from source-receipts.json. No network requests are made.');
const receipts = JSON.parse(await readFile(new URL('./source-receipts.json', import.meta.url), 'utf8'));
const load = async (path, suffix) => {
  const source = await readFile(path, 'utf8');
  const expected = receipts.find(r => r.path.endsWith(suffix));
  assert.equal(createHash('sha256').update(source).digest('hex'), expected.sha256);
  return source.replace(/^import .*$/gm, '').replace(/^export default .*$/gm, '').replace(/^export /gm, '').replace(/const (action|destination): [^=]+=/g, 'const $1 =');
};
const context = vm.createContext({ defaultValues: () => ({}) }, { codeGeneration: { strings: false, wasm: false } });
vm.runInContext(await load(actionPath, '/postEvent/index.ts') + '\nglobalThis.sourceAction = action;', context, { timeout: 1000 });
context.postEvent = context.sourceAction;
vm.runInContext(await load(indexPath, '/iqm/index.ts') + '\nglobalThis.sourceDestination = destination;', context, { timeout: 1000 });
const calls = [];
const request = async (url, options) => {
  const req = new Request(url, options);
  calls.push({ url: req.url, method: req.method, headers: Object.fromEntries(req.headers), body: await req.text() });
};
const settings = { pixel_id: 'my-pixel-id' };
await context.sourceAction.perform(request, { settings, payload: {} });
await context.sourceAction.perform(request, { settings, payload: { data: { testType: '[7TV!' } } });
await context.sourceDestination.onDelete(request, { settings, payload: { userId: 'synthetic-user', traits: { optOut: true } } });
const fixtureNames = ['source-required-fields', 'source-all-fields', 'source-delete-object'];
for (let i = 0; i < calls.length; i++) assert.deepEqual(calls[i], JSON.parse(await readFile(new URL(`./${fixtureNames[i]}.json`, import.meta.url), 'utf8')));
console.log(`${calls.length} pinned source-generated requests match the independent golden inputs. Network requests:0.`);
