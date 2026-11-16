import type { TFunction } from 'i18next';
import type { QoderAccount } from '../types/qoder';
export type QoderRegion = 'global' | 'cn';
export type QoderClient = 'ide' | 'desktop' | 'cli';
export function isQoderClientAvailable(
  region: QoderRegion,
  client: QoderClient,
  isWindows: boolean,
): boolean {
  if (client === 'ide') return region === 'global';
  if (client === 'desktop') return isWindows;
  return true;
}
export function defaultQoderClient(region: QoderRegion, isWindows: boolean): QoderClient {
  if (region === 'global') return 'ide';
  return isWindows ? 'desktop' : 'cli';
}
export function getQoderRegion(account: QoderAccount): QoderRegion {
  const info = account.auth_user_info_raw as { login_source?: string } | undefined;
  const source = info?.login_source;
  return source === 'qodercn' ? 'cn' : 'global';
}
export function qoderRegionLabel(region: QoderRegion, t: TFunction): string {
  return region === 'cn'
    ? t('qoder.region.cn', 'Qoder CN · 国内版')
    : t('qoder.region.global', 'Qoder · 国际版');
}
