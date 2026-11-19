import assert from 'node:assert/strict';
import test from 'node:test';
import { loadHookModule } from '../../tests/helpers/reactHookHarness';

function service(options: { writeError?: Error; invokeError?: Error; cleanupError?: Error } = {}) {
  const calls: { name: string; args: any[] }[] = [];
  const accounts = [{ id: 'fixture', quota_raw: { resources: ['quota'] }, usage_raw: { resources: ['usage'] } }];
  const h = loadHookModule(new URL('./workbuddyService.ts', import.meta.url), {
    '@tauri-apps/api/core': {
      async invoke(...args: any[]) {
        calls.push({ name: 'invoke', args: structuredClone(args) });
        if (options.invokeError) throw options.invokeError;
        return accounts;
      },
    },
    '@tauri-apps/plugin-fs': {
      BaseDirectory: { AppData: 14 },
      async mkdir(...args: any[]) { calls.push({ name: 'mkdir', args: structuredClone(args) }); },
      async writeTextFile(...args: any[]) {
        calls.push({ name: 'write', args: structuredClone(args) });
        if (options.writeError) throw options.writeError;
      },
      async remove(...args: any[]) {
        calls.push({ name: 'remove', args: structuredClone(args) });
        if (options.cleanupError) throw options.cleanupError;
      },
    },
  });
  return { calls, accounts, importJson: h.exports.importWorkbuddyFromJson, threshold: h.exports.WORKBUDDY_IMPORT_VIA_FILE_THRESHOLD };
}

test('small JSON retains the original command and complete payload', async () => {
  const s = service();
  const content = '{"quota_raw":{"quota":1},"usage_raw":{"usage":2}}';
  assert.equal(await s.importJson(content), s.accounts);
  assert.deepEqual(s.calls, [{ name: 'invoke', args: ['import_workbuddy_from_json', { jsonContent: content }] }]);
});

test('large Unicode JSON writes the exact payload and sends only the temporary name', async () => {
  const s = service({ cleanupError: new Error('already removed by backend') });
  const content = JSON.stringify({ quota_raw: '额度'.repeat(s.threshold), usage_raw: { keep: true } });
  assert.equal(await s.importJson(content), s.accounts);
  assert.deepEqual(s.calls.map(c => c.name), ['mkdir', 'write', 'invoke', 'remove']);
  // The AppData dir is ensured with an empty relative path: "." would make Rust's
  // create_dir_all skip a level and fail with os error 2 on a fresh install.
  assert.deepEqual(s.calls[0].args, ['', { baseDir: 14, recursive: true }]);
  const filename = s.calls[1].args[0];
  assert.match(filename, /^workbuddy-import-[a-z0-9-]+\.json$/);
  assert.equal(s.calls[1].args[1], content);
  assert.equal(s.calls[1].args[2].baseDir, 14);
  assert.deepEqual(s.calls[2].args, ['import_workbuddy_from_json_file', { relativePath: filename }]);
  assert.deepEqual(s.calls[3].args, [filename, { baseDir: 14 }]);
});

for (const stage of ['write', 'invoke'] as const) {
  test(`a failed ${stage} cleans up the temporary file and preserves the original error`, async () => {
    const failure = new Error(`${stage} failed`);
    const s = service({ [`${stage}Error`]: failure, cleanupError: new Error('cleanup failed') });
    await assert.rejects(s.importJson('x'.repeat(s.threshold)), error => error === failure);
    assert.equal(s.calls[s.calls.length - 1]?.name, 'remove');
    if (stage === 'write') assert.ok(!s.calls.some(c => c.name === 'invoke'));
  });
}
