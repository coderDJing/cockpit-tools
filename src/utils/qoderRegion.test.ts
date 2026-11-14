import assert from 'node:assert/strict';
import test from 'node:test';
import { getQoderRegion } from './qoderRegion';
import type { QoderAccount } from '../types/qoder';
test('Qoder account region is determined by credential scope, not its name or tags', () => {
  const account = { id:'test',email:'test@example.invalid',created_at:1,last_used:1,tags:['国际版'] } as QoderAccount;
  assert.equal(getQoderRegion({...account,auth_user_info_raw:{login_source:'qodercn'}}),'cn');
  assert.equal(getQoderRegion({...account,auth_user_info_raw:{login_source:'dedicated_qodercn'}}),'global');
  assert.equal(getQoderRegion({...account,tags:['国内 CN']}),'global');
  assert.equal(getQoderRegion({...account,auth_user_info_raw:{login_source:'qoder'}}),'global');
});
