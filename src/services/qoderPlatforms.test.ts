import assert from 'node:assert/strict';
import test from 'node:test';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { ALL_PLATFORM_IDS, PLATFORM_PAGE_MAP } from '../types/platform';
import * as globalService from './qoderService';
import * as cnService from './qoderCnService';

test('Qoder and Qoder CN keep independent navigation and credential IPC destinations', async () => {
  assert.ok(ALL_PLATFORM_IDS.includes('qoder'));
  assert.ok(ALL_PLATFORM_IDS.includes('qoder_cn'));
  assert.equal(PLATFORM_PAGE_MAP.qoder, 'qoder');
  assert.equal(PLATFORM_PAGE_MAP.qoder_cn, 'qoder-cn');
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  Object.defineProperty(globalThis, 'window', { configurable: true, value: {} });
  const calls: Array<{ command: string; args: unknown }> = [];
  mockIPC((command, args) => {
    calls.push({ command, args });
    if (command.endsWith('login_start')) return { login_id: 'fixture', verification_uri: 'https://example.invalid', expires_in: 600, interval_seconds: 1 };
    return [];
  });
  try {
    await globalService.listQoderAccounts();
    await cnService.listQoderAccounts();
    await globalService.refreshQoderToken('same-fixture-id');
    await cnService.refreshQoderToken('same-fixture-id');
    await globalService.importQoderFromJson('[]');
    await cnService.importQoderFromJson('[]');
    await globalService.qoderOauthLoginStart();
    await cnService.qoderOauthLoginStart();
    assert.deepEqual(calls.map(call => call.command), [
      'list_qoder_accounts', 'list_qoder_cn_accounts',
      'refresh_qoder_token', 'refresh_qoder_cn_token',
      'import_qoder_from_json', 'import_qoder_cn_from_json',
      'qoder_oauth_login_start', 'qoder_cn_oauth_login_start',
    ]);
    assert.deepEqual(calls.slice(-2).map(call => call.args), [{ region: 'global' }, { region: 'cn' }]);
  } finally {
    clearMocks();
    if (previous) Object.defineProperty(globalThis, 'window', previous);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});
