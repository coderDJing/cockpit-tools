import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Settings } from 'lucide-react';
import { useTranslation } from 'react-i18next';

/** CN native clients have their own refresh preference and OS-managed profile paths. */
export function QoderCnQuickSettings() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [minutes, setMinutes] = useState('10');
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const toggle = async () => {
    if (open) { setOpen(false); return; }
    try {
      const config = await invoke<{ qoder_cn_auto_refresh_minutes: number }>('get_general_config');
      setMinutes(String(config.qoder_cn_auto_refresh_minutes));
      setError('');
      setOpen(true);
    } catch (err) { setError(String(err)); }
  };
  const save = async () => {
    const value = Number(minutes);
    if (!Number.isInteger(value) || (value !== -1 && (value < 1 || value > 999))) return;
    setSaving(true);
    try {
      await invoke('patch_general_config', { updates: { qoder_cn_auto_refresh_minutes: value } });
      window.dispatchEvent(new Event('config-updated'));
      setOpen(false);
      setError('');
    } catch (err) { setError(String(err)); }
    finally { setSaving(false); }
  };
  return <div style={{ position: 'relative' }}>
    <button className="btn btn-secondary" title="Qoder CN" onClick={() => void toggle()}><Settings size={16} /></button>
    {open && <div className="quick-settings-popover" style={{ position: 'absolute', right: 0, zIndex: 50, minWidth: 270, padding: 16, background: 'var(--bg-primary)', border: '1px solid var(--border-color)', borderRadius: 12 }}>
      <strong>Qoder CN</strong>
      <label style={{ display: 'block', margin: '12px 0' }}>
        {t('quickSettings.refreshInterval', '配额自动刷新')}
        <input aria-label="Qoder CN refresh interval" type="number" value={minutes} disabled={saving} onChange={event => setMinutes(event.target.value)} style={{ width: 70, marginLeft: 8 }} />
        {t('wakeup.refreshInterval.minutes', '分钟')}
      </label>
      <p>{t('quickSettings.disableRefreshHint', '-1 表示禁用')}</p>
      <button className="btn btn-primary" disabled={saving} onClick={() => void save()}>{t('common.save', '保存')}</button>
    </div>}
    {error && <div role="alert">{error}</div>}
  </div>;
}
