import { useState } from 'react';
import { api } from '../api/client';
import type { Connectivity, Diagnostics, ToolResult, WgProfile, WgProfileInput } from '../api/types';
import { Icon } from '../components/Icon';
import { CheckBadge, ConfirmDialog, CopyButton, Empty, ErrorBox, Field, Modal, PageHeader, Panel, Pill, Spinner, useLoad } from '../components/ui';
import { isValidHost, isValidPort } from '../lib/validate';
import { formatTime } from '../lib/format';
import { useApp } from '../store';

function TailscaleCard({ c }: { c: Connectivity }) {
  const t = c.tailscale;
  const addr = t?.magicdns_name || t?.ip || '';
  const url = addr ? `https://${addr}:${c.port}` : '';
  return (
    <Panel title="Tailscale" actions={t ? <Pill tone={t.running ? 'green' : t.installed ? 'amber' : 'muted'}>{t.running ? 'Running' : t.installed ? (t.backend_state || 'Stopped') : 'Not installed'}</Pill> : <Pill>Unknown</Pill>}>
      <div className="stack">
        {t?.error && <ErrorBox message={t.error} />}
        <dl className="kv">
          <dt>Tailscale IP</dt><dd className="mono">{t?.ip || <span className="na">not reported</span>}</dd>
          <dt>Host name</dt><dd>{t?.magicdns_name || t?.hostname || <span className="na">not reported</span>}</dd>
          <dt>Tailnet</dt><dd>{t?.tailnet || <span className="na">not reported</span>}</dd>
        </dl>
        <h3>Use your phone away from home</h3>
        <ol className="steps-ol">
          <li>{t?.installed ? 'Tailscale is installed on this computer.' : <>Install Tailscale on <strong>this computer</strong> (the controller) from tailscale.com/download.</>}</li>
          <li>{t?.running ? 'Tailscale is signed in and running here.' : 'Sign in to Tailscale on this computer and make sure it is connected.'}</li>
          <li>Install Tailscale on your <strong>phone</strong> and sign in with the <strong>same account</strong> (same tailnet).</li>
          <li>Keep Tailscale connected on the phone. Then pair the app with a QR code on the Mobile Access page. The code includes the Tailscale address automatically.</li>
          <li>The controller is then reachable at the address below from anywhere, without opening any port on your router.</li>
        </ol>
        {url && <div className="row gap-s wrap"><span className="mono grow" style={{ overflowWrap: 'anywhere' }}>{url}</span><CopyButton text={url} /></div>}
      </div>
    </Panel>
  );
}

function WgEditor({ p, onClose, onSaved }: { p: WgProfile | null; onClose: () => void; onSaved: () => void }) {
  const { toast, toastError } = useApp();
  const [f, setF] = useState<WgProfileInput>(p ? { name: p.name, controller_address: p.controller_address, port: p.port, endpoint: p.endpoint, allowed_ips: p.allowed_ips, notes: p.notes } : { name: '', controller_address: '', port: 8443, endpoint: '', allowed_ips: '', notes: '' });
  const [busy, setBusy] = useState(false);
  const [touched, setTouched] = useState(false);
  const set = <K extends keyof WgProfileInput>(k: K, v: WgProfileInput[K]) => setF((o) => ({ ...o, [k]: v }));
  const errs = {
    name: !f.name.trim() ? 'Name is required.' : null,
    addr: !isValidHost(f.controller_address) ? 'Enter the controller’s address inside the VPN, e.g. 10.8.0.1.' : null,
    port: !isValidPort(f.port) ? 'Port must be 1–65535.' : null,
  };
  const invalid = Object.values(errs).some(Boolean);
  const show = (k: keyof typeof errs) => (touched ? errs[k] : null);
  const save = async () => {
    setTouched(true);
    if (invalid) return;
    setBusy(true);
    try { if (p) await api.updateWg(p.id, f); else await api.createWg(f); toast('ok', 'WireGuard profile saved.'); onSaved(); }
    catch (e) { toastError(e, 'Save profile'); setBusy(false); }
  };
  return (
    <Modal wide title={p ? 'Edit WireGuard profile' : 'New WireGuard profile'} onClose={onClose} locked={busy}
      footer={<><button className="btn" onClick={onClose} disabled={busy}>Cancel</button><button className="btn primary" onClick={save} disabled={busy}>Save</button></>}>
      <div className="stack">
        <div className="notice"><Icon name="shield" size={16} />Profiles only describe how phones reach the controller. They never store WireGuard private keys. Keep keys in the WireGuard app.</div>
        <div className="form-grid">
          <Field label="Name *" error={show('name')}><input data-autofocus value={f.name} onChange={(e) => set('name', e.target.value)} placeholder="Home WireGuard" /></Field>
          <Field label="Controller address in VPN *" error={show('addr')}><input value={f.controller_address} onChange={(e) => set('controller_address', e.target.value)} placeholder="10.8.0.1" className="mono" /></Field>
          <Field label="Controller port" error={show('port')}><input type="number" value={f.port} onChange={(e) => set('port', Number(e.target.value))} /></Field>
          <Field label="VPN endpoint (public)" hint="Your router’s public address and WireGuard port, e.g. vpn.example.com:51820."><input value={f.endpoint} onChange={(e) => set('endpoint', e.target.value)} className="mono" /></Field>
          <Field label="Allowed IPs" hint="Subnets the phone should route through the VPN." className="full"><input value={f.allowed_ips} onChange={(e) => set('allowed_ips', e.target.value)} placeholder="10.8.0.0/24, 192.168.1.0/24" className="mono" /></Field>
          <Field label="Notes" className="full"><textarea value={f.notes} onChange={(e) => set('notes', e.target.value)} /></Field>
        </div>
      </div>
    </Modal>
  );
}

function WireGuard() {
  const { isAdmin, toast, toastError } = useApp();
  const { data, error, loading, reload } = useLoad(() => api.wgProfiles(), []);
  const [edit, setEdit] = useState<WgProfile | 'new' | null>(null);
  const [del, setDel] = useState<WgProfile | null>(null);
  const [busy, setBusy] = useState(false);
  const remove = async () => {
    if (!del) return;
    setBusy(true);
    try { await api.deleteWg(del.id); toast('ok', 'Profile deleted.'); setDel(null); await reload(true); }
    catch (e) { toastError(e, 'Delete profile'); }
    finally { setBusy(false); }
  };
  const list = data ?? [];
  return (
    <Panel title="WireGuard profiles" actions={isAdmin && <button className="btn sm primary-soft" onClick={() => setEdit('new')}><Icon name="plus" size={14} />Add profile</button>}>
      {loading && !data ? <Spinner /> : error && !data ? <ErrorBox message={error} onRetry={() => void reload()} /> : list.length === 0 ? (
        <Empty icon="vpn" title="No WireGuard profiles">If you run your own WireGuard server, describe how phones reach this controller here. Tailscale needs no profile.</Empty>
      ) : (
        <div className="table-wrap"><table className="tbl">
          <thead><tr><th>Name</th><th>Controller</th><th>Endpoint</th><th>Allowed IPs</th><th aria-label="Actions" /></tr></thead>
          <tbody>{list.map((p) => (
            <tr key={p.id}><td><strong>{p.name}</strong>{p.notes && <div className="muted small">{p.notes}</div>}</td><td className="mono">{p.controller_address}:{p.port}</td><td className="mono">{p.endpoint || '—'}</td><td className="mono">{p.allowed_ips || '—'}</td>
              <td className="actions">{isAdmin && <><button className="btn sm ghost" onClick={() => setEdit(p)} aria-label={`Edit ${p.name}`}><Icon name="edit" size={14} /></button><button className="btn sm ghost" onClick={() => setDel(p)} aria-label={`Delete ${p.name}`}><Icon name="trash" size={14} /></button></>}</td></tr>
          ))}</tbody>
        </table></div>
      )}
      {edit && <WgEditor p={edit === 'new' ? null : edit} onClose={() => setEdit(null)} onSaved={() => { setEdit(null); void reload(true); }} />}
      {del && <ConfirmDialog title={`Delete ${del.name}?`} confirmLabel="Delete profile" danger busy={busy} onConfirm={remove} onCancel={() => setDel(null)}><p>Only the saved description is removed. Your WireGuard setup is not changed.</p></ConfirmDialog>}
    </Panel>
  );
}

function DiagnosticsPanel() {
  const { locations, locationId, toastError } = useApp();
  const [loc, setLoc] = useState(locationId);
  const [res, setRes] = useState<Diagnostics | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    try { setRes(await api.diagnostics(loc || undefined)); }
    catch (e) { toastError(e, 'Diagnostics'); }
    finally { setBusy(false); }
  };
  const counts = res ? { ok: res.checks.filter((c) => c.status === 'ok').length, warn: res.checks.filter((c) => c.status === 'warn').length, fail: res.checks.filter((c) => c.status === 'fail').length } : null;
  return (
    <Panel title="Diagnostics" actions={
      <>
        <select value={loc} onChange={(e) => setLoc(e.target.value)} aria-label="Location to check" style={{ width: 'auto' }}><option value="">All / controller</option>{locations.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}</select>
        <button className="btn primary" onClick={run} disabled={busy}><Icon name="play" size={14} />{busy ? 'Running…' : 'Run diagnostics'}</button>
      </>}>
      {busy && !res ? <Spinner label="Running checks" /> : !res ? <Empty icon="shield" title="Run a health check">Checks the controller, VPN, relay reachability and latency, database and scheduler, and explains anything that is wrong.</Empty> : (
        <div className="stack">
          <div className="row wrap gap-s">{counts && <><Pill tone="green">{counts.ok} OK</Pill><Pill tone="amber">{counts.warn} warnings</Pill><Pill tone="red">{counts.fail} failed</Pill></>}<span className="muted small">Generated {formatTime(res.generated_at)}</span></div>
          <div className="table-wrap"><table className="tbl"><tbody>
            {res.checks.map((c, i) => (
              <tr key={i}><td className="nowrap"><CheckBadge status={c.status} /></td><td><strong>{c.name}</strong><div className="muted small">{c.detail}</div></td><td className="num muted nowrap">{c.duration_ms !== null && c.duration_ms !== undefined ? `${c.duration_ms} ms` : ''}</td></tr>
            ))}
          </tbody></table></div>
        </div>
      )}
    </Panel>
  );
}

function Tools() {
  const { locations, locationId, toastError } = useApp();
  const [host, setHost] = useState('');
  const [port, setPort] = useState('443');
  const [loc, setLoc] = useState(locationId);
  const [out, setOut] = useState<{ label: string; r: ToolResult } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const hostOk = isValidHost(host);
  const go = async (kind: 'ping' | 'dns' | 'tcp') => {
    setBusy(kind); setOut(null);
    try {
      const l = loc || undefined;
      const r = kind === 'ping' ? await api.ping(host.trim(), l) : kind === 'dns' ? await api.dns(host.trim(), l) : await api.tcp(host.trim(), Number(port), l);
      setOut({ label: `${kind.toUpperCase()} ${host.trim()}${kind === 'tcp' ? `:${port}` : ''}`, r });
    } catch (e) { toastError(e, 'Tool'); }
    finally { setBusy(null); }
  };
  return (
    <Panel title="Network tools" actions={<span className="muted small">Run from the selected location’s relay</span>}>
      <div className="stack">
        <div className="form-grid">
          <Field label="Host or IP" error={host && !hostOk ? 'Enter a host name or IPv4 address.' : null}><input value={host} onChange={(e) => setHost(e.target.value)} placeholder="192.168.1.20 or nas.local" /></Field>
          <Field label="Run from"><select value={loc} onChange={(e) => setLoc(e.target.value)}><option value="">Controller</option>{locations.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}</select></Field>
          <Field label="TCP port" error={!isValidPort(Number(port)) ? '1–65535' : null}><input value={port} onChange={(e) => setPort(e.target.value)} inputMode="numeric" /></Field>
        </div>
        <div className="row wrap gap-s">
          <button className="btn" disabled={!hostOk || !!busy} onClick={() => go('ping')}>{busy === 'ping' ? 'Pinging…' : 'Ping'}</button>
          <button className="btn" disabled={!hostOk || !!busy} onClick={() => go('dns')}>{busy === 'dns' ? 'Resolving…' : 'DNS lookup'}</button>
          <button className="btn" disabled={!hostOk || !isValidPort(Number(port)) || !!busy} onClick={() => go('tcp')}>{busy === 'tcp' ? 'Connecting…' : 'TCP connect'}</button>
        </div>
        {out && (
          <div className={`notice${out.r.ok ? '' : ' warn'}`} role="status">
            <Icon name={out.r.ok ? 'check' : 'warn'} size={16} />
            <div><strong>{out.label}</strong>: {out.r.ok ? 'success' : 'failed'}{out.r.duration_ms !== null && out.r.duration_ms !== undefined ? ` in ${out.r.duration_ms} ms` : ''}<div className="muted small" style={{ whiteSpace: 'pre-wrap' }}>{out.r.detail}</div></div>
          </div>
        )}
      </div>
    </Panel>
  );
}

export function RemotePage() {
  const { data, error, loading, reload } = useLoad(() => api.connectivity(), []);
  return (
    <>
      <PageHeader title="Remote access & diagnostics" subtitle="Reach the controller away from home with Tailscale or WireGuard, and check that everything works." />
      {loading && !data ? <Spinner /> : error && !data ? <ErrorBox message={error} onRetry={() => void reload()} /> : data && (
        <>
          {data.allow_public && <div className="notice warn"><Icon name="warn" size={16} />The controller accepts connections from public internet addresses (“allow public” is on). Prefer Tailscale or WireGuard and turn this off in Settings.</div>}
          <div className="two-col">
            <TailscaleCard c={data} />
            <Panel title="Controller addresses">
              {(data.addresses ?? []).length === 0 ? <Empty icon="wifi" title="No addresses reported" /> : (
                <div className="ops-list">
                  {data.addresses.map((a) => (
                    <div className="op-row" key={a.address}>
                      <Pill tone={a.kind === 'tailscale' ? 'green' : a.kind === 'lan' ? 'cyan' : 'muted'}>{a.kind}</Pill>
                      <div className="what"><strong className="mono" style={{ fontWeight: 500 }}>https://{a.address}:{data.port}</strong>{a.interface && <span>{a.interface}</span>}</div>
                      <CopyButton text={`https://${a.address}:${data.port}`} />
                    </div>
                  ))}
                </div>
              )}
            </Panel>
          </div>
        </>
      )}
      <WireGuard />
      <DiagnosticsPanel />
      <Tools />
    </>
  );
}
