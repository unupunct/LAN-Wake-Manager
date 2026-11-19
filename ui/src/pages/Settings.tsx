import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { api } from '../api/client';
import type { Settings } from '../api/types';
import { Icon } from '../components/Icon';
import { ConfirmDialog, Empty, ErrorBox, Field, PageHeader, Panel, Pill, Spinner, Toggle, useLoad } from '../components/ui';
import { relativeTime } from '../lib/format';
import { useApp } from '../store';

function zones(): string[] {
  try { return (Intl as unknown as { supportedValuesOf: (k: string) => string[] }).supportedValuesOf('timeZone'); } catch { return ['UTC']; }
}

function PasswordPanel() {
  const { toast, toastError } = useApp();
  const [oldPw, setOld] = useState('');
  const [pw, setPw] = useState('');
  const [pw2, setPw2] = useState('');
  const [busy, setBusy] = useState(false);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (pw.length < 10 || pw !== pw2 || !oldPw) return;
    setBusy(true);
    try { await api.changePassword(oldPw, pw); toast('ok', 'Password changed.'); setOld(''); setPw(''); setPw2(''); }
    catch (err) { toastError(err, 'Change password'); }
    finally { setBusy(false); }
  };
  return (
    <Panel title="Change admin password">
      <form className="stack" onSubmit={submit} style={{ maxWidth: 420 }}>
        <Field label="Current password"><input type="password" autoComplete="current-password" value={oldPw} onChange={(e) => setOld(e.target.value)} /></Field>
        <Field label="New password" error={pw && pw.length < 10 ? 'At least 10 characters.' : null}><input type="password" autoComplete="new-password" value={pw} onChange={(e) => setPw(e.target.value)} /></Field>
        <Field label="Repeat new password" error={pw2 && pw !== pw2 ? 'The passwords do not match.' : null}><input type="password" autoComplete="new-password" value={pw2} onChange={(e) => setPw2(e.target.value)} /></Field>
        <div><button className="btn" disabled={busy || !oldPw || pw.length < 10 || pw !== pw2}>{busy ? 'Changing…' : 'Change password'}</button></div>
      </form>
    </Panel>
  );
}

function Notifications() {
  const { liveTick } = useApp();
  const { data, error, loading, reload } = useLoad(() => api.notifications({ limit: 20 }), []);
  useEffect(() => { if (liveTick) void reload(true); }, [liveTick, reload]);
  const list = [...(data ?? [])].sort((a, b) => b.ts - a.ts);
  return (
    <Panel title="Recent notifications">
      {loading && !data ? <Spinner /> : error && !data ? <ErrorBox message={error} onRetry={() => void reload()} /> : list.length === 0 ? <Empty icon="bell" title="No notifications">Wake results, offline relays and other events appear here.</Empty> : (
        <div className="ops-list">{list.map((n) => (
          <div className="op-row" key={n.id}>
            <div className="kind-ico" style={{ color: n.severity === 'error' ? 'var(--red)' : n.severity === 'warn' ? 'var(--amber)' : 'var(--cyan)' }}><Icon name={n.severity === 'info' ? 'bell' : 'warn'} size={16} /></div>
            <div className="what"><strong>{n.title}</strong><span>{n.body}</span></div>
            <span className="muted small nowrap">{relativeTime(n.ts)}</span>
          </div>
        ))}</div>
      )}
    </Panel>
  );
}

export function SettingsPage() {
  const { isAdmin, theme, toggleTheme, toast, toastError, info } = useApp();
  const { data, error, loading, reload } = useLoad(() => api.settings(), []);
  const [f, setF] = useState<Settings | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmPublic, setConfirmPublic] = useState(false);
  const tz = useMemo(zones, []);

  useEffect(() => { if (data) setF(data); }, [data]);

  if (loading && !f) return <Spinner />;
  if (error && !f) return <ErrorBox message={error} onRetry={() => void reload()} />;
  if (!f) return null;
  const orig = data;
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => setF((o) => (o ? { ...o, [k]: v } : o));
  const dirty = JSON.stringify(f) !== JSON.stringify(orig);
  const errs = {
    name: !f.name.trim() ? 'Name is required.' : null,
    mon: !(f.monitor_interval_secs >= 5 && f.monitor_interval_secs <= 3600) ? 'Between 5 and 3600 seconds.' : null,
    bi: f.backup.auto && !(f.backup.interval_hours >= 1 && f.backup.interval_hours <= 720) ? 'Between 1 and 720 hours.' : null,
    keep: f.backup.auto && !(f.backup.keep >= 1 && f.backup.keep <= 365) ? 'Keep between 1 and 365 backups.' : null,
    notify: !(f.notify_schedule_minutes >= 0 && f.notify_schedule_minutes <= 1440) ? 'Between 0 and 1440 minutes.' : null,
  };
  const invalid = Object.values(errs).some(Boolean);

  const save = async () => {
    setBusy(true);
    try { const s = await api.saveSettings({ ...f, name: f.name.trim() }); setF(s ?? f); toast('ok', 'Settings saved.'); await reload(true); }
    catch (e) { toastError(e, 'Save settings'); }
    finally { setBusy(false); }
  };
  const onPublic = (v: boolean) => { if (v) setConfirmPublic(true); else set('allow_public', false); };

  return (
    <>
      <PageHeader title="Settings" subtitle={`Controller ${info?.version ? `v${info.version}` : ''}`}>
        {isAdmin && <button className="btn primary" disabled={!dirty || invalid || busy} onClick={save}>{busy ? 'Saving…' : 'Save changes'}</button>}
      </PageHeader>
      {!isAdmin && <div className="notice"><Icon name="info" size={16} />Settings are read-only for your role.</div>}
      <fieldset disabled={!isAdmin || busy} style={{ border: 0, padding: 0 }}>
        <div className="stack">
          <Panel title="General">
            <div className="form-grid">
              <Field label="Controller name" error={errs.name} hint="Shown in the dashboard and in phones that pair with this controller."><input value={f.name} onChange={(e) => set('name', e.target.value)} /></Field>
              <Field label="Timezone" hint="Schedules run in this timezone. “Computer default” uses the controller’s system timezone.">
                <select value={f.timezone ?? ''} onChange={(e) => set('timezone', e.target.value || null)}>
                  <option value="">Computer default</option>
                  {f.timezone && !tz.includes(f.timezone) && <option value={f.timezone}>{f.timezone}</option>}
                  {tz.map((z) => <option key={z} value={z}>{z}</option>)}
                </select>
              </Field>
              <Field label="Status check interval (seconds)" error={errs.mon} hint="How often devices are probed for online/offline."><input type="number" min={5} value={f.monitor_interval_secs} onChange={(e) => set('monitor_interval_secs', Number(e.target.value))} /></Field>
            </div>
          </Panel>
          <Panel title="Security">
            <div className="stack">
              <Toggle checked={f.allow_public} onChange={onPublic} label="Allow connections from public internet addresses" />
              {f.allow_public
                ? <div className="notice warn"><Icon name="warn" size={16} /><span>The controller currently answers requests from <strong>any</strong> internet address. Anyone who can reach port 8443 can try to log in. Prefer Tailscale or WireGuard and switch this off.</span></div>
                : <p className="muted small">Off (recommended): only this computer, private networks, Tailscale (100.64.0.0/10) and link-local addresses are served.</p>}
            </div>
          </Panel>
          <Panel title="Automatic backups">
            <div className="stack">
              <Toggle checked={f.backup.auto} onChange={(v) => set('backup', { ...f.backup, auto: v })} label="Create backups automatically" />
              <div className="form-grid">
                <Field label="Every (hours)" error={errs.bi}><input type="number" min={1} disabled={!f.backup.auto} value={f.backup.interval_hours} onChange={(e) => set('backup', { ...f.backup, interval_hours: Number(e.target.value) })} /></Field>
                <Field label="Keep the newest" error={errs.keep} hint="Older automatic backups are deleted."><input type="number" min={1} disabled={!f.backup.auto} value={f.backup.keep} onChange={(e) => set('backup', { ...f.backup, keep: Number(e.target.value) })} /></Field>
              </div>
            </div>
          </Panel>
          <Panel title="Notifications">
            <Field label="Warn before a schedule runs (minutes)" error={errs.notify} hint="Phones get a heads-up this long before a shutdown or restart schedule fires. 0 disables it."><input type="number" min={0} style={{ maxWidth: 200 }} value={f.notify_schedule_minutes} onChange={(e) => set('notify_schedule_minutes', Number(e.target.value))} /></Field>
          </Panel>
        </div>
      </fieldset>
      <Notifications />
      <Panel title="Appearance">
        <div className="row gap-s"><span>Theme</span><Pill tone="cyan">{theme}</Pill><button className="btn sm" onClick={toggleTheme}><Icon name={theme === 'dark' ? 'sun' : 'moon'} size={14} />Switch to {theme === 'dark' ? 'light' : 'dark'}</button></div>
      </Panel>
      {isAdmin && <PasswordPanel />}
      {confirmPublic && (
        <ConfirmDialog title="Allow public internet access?" confirmLabel="Allow public access" danger onConfirm={() => { set('allow_public', true); setConfirmPublic(false); }} onCancel={() => setConfirmPublic(false)}>
          <p>This lets devices on the public internet connect to the controller if your router forwards the port. The admin password then becomes the only barrier. This is <strong>not recommended</strong>. Tailscale or WireGuard are safer.</p>
          <p className="muted small">The change only takes effect after you press “Save changes”.</p>
        </ConfirmDialog>
      )}
    </>
  );
}
