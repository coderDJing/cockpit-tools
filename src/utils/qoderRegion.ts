import type { QoderAccount } from '../types/qoder';
export type QoderRegion = 'global' | 'cn';
export type QoderClient = 'ide' | 'desktop' | 'cli';
export function getQoderRegion(account: QoderAccount): QoderRegion {
  const info = account.auth_user_info_raw as { login_source?: string } | undefined;
  const source = info?.login_source;
  return source === 'qodercn' ? 'cn' : 'global';
}
export function qoderRegionLabel(region: QoderRegion): string {
  return region === 'cn' ? 'Qoder CN · 国内版' : 'Qoder · 国际版';
}
