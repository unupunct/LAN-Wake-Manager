import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import type { Device, Enrollment, MetricPoint } from '../api/types';
import { countdown, formatBytes, formatDuration, formatPercent, formatTemp, formatTime, relativeTime } from '../lib/format';
import { useApp } from '../store';
import { DeviceButtons, useDeviceActions } from './actions';
import { Icon } from './Icon';
import { OpRow } from './ops';
import { CheckBadge, CodeBlock, ConfirmDialog, Empty, ErrorBox, Meter, Modal, Pill, Sparkline, Spinner, StatusBadge, Tabs, useLoad } from './ui';

export function deviceIcon(t: string | undefined): string {
  switch (t) { case 'laptop': return 'laptop'; case 'server': return 'server'; case 'nas': return 'nas'; default: return 'desktop'; }
}

const NA = <span className="na">not reported</span>;

function KV({ k, children }: { k: string; children: React.ReactNode }) {
  return <><dt>{k}</dt><dd>{children}</dd></>;
}

type Tab = 'overview' | 'system' | 'history' | 'agent' | 'troubleshoot';

export function DeviceDrawer({ id }: { id: string }) {
  const { devices, locations, groups, isAdmin, toast, toastError, upsertDevice, reloadDevices, liveTick } = useApp();
  const nav = useNavigate();
  const actions = useDeviceActions();
  const [tab, setTab] = useState<Tab>('overview');
  const [confirmDel, setConfirmDel] = useState(false);
  const [busy, setBusy] = useState(false);
  const device = devices.find((d) => d.id === id);
  const close = () => nav('/devices');

  // fetch fresh full record (sysinfo etc.) once
  useEffect(() => {
    let live = true;
    api.device(id).then((d) => { if (live) upsertDevice(d); }).catch(() => undefined);
    return () => { live = false; };
  }, [id, upsertDevice]);

  if (!device) {
    return (
      <Modal drawer title="Device" onClose={close}>
        <Empty icon="devices" title="Device not found">It may have been deleted. <Link to="/devices">Back to devices</Link></Empty>
      </Modal>
    );
  }
  const loc = locations.find((l) => l.id === device.location_id);

  const refresh = async () => {
    setBusy(true);
    try { upsertDevice(await api.refreshDevice(device.id)); toast('ok', 'Status refreshed.'); }
    catch (e) { toastError(e, 'Refresh'); }
    finally { setBusy(false); }
  };
  const remove = async () => {
    setBusy(true);
    try { await api.deleteDevice(device.id); toast('ok', `${device.name} was deleted.`); await reloadDevices(); nav('/devices'); }
    catch (e) { toastError(e, 'Delete'); setBusy(false); }
  };

  return (
    <Modal drawer onClose={close} title={<span className="row gap-s"><span>{device.name}</span><StatusBadge status={device.status} /></span>}>
      <div className="stack">
        <div className="row wrap gap-s">
          <DeviceButtons d={device} actions={actions} labels />
          <div className="grow" />
          <button className="btn sm" onClick={refresh} disabled={busy}><Icon name="refresh" size={14} />Refresh</button>
          {isAdmin && <Link className="btn sm" to={`/devices/${device.id}/edit`}><Icon name="edit" size={14} />Edit</Link>}
          {isAdmin && <button className="btn sm danger" onClick={() => setConfirmDel(true)}><Icon name="trash" size={14} />Delete</button>}
        </div>
        {device.status === 'shutting_down' && actions.canOperate && (
          <div className="notice warn"><Icon name="warn" size={16} /><span className="grow">A shutdown is in progress.</span><button className="btn sm" onClick={() => actions.cancelShutdown(device)}>Cancel shutdown</button></div>
        )}
        <Tabs<Tab> value={tab} onChange={setTab} tabs={[
          { value: 'overview', label: 'Overview' }, { value: 'system', label: 'System' }, { value: 'history', label: 'History' },
          { value: 'agent', label: 'Agent & power' }, { value: 'troubleshoot', label: 'Troubleshoot' },
        ]} />
        {tab === 'overview' && (
          <dl className="kv">
            <KV k="Location">{loc?.name ?? device.location_id}</KV>
            <KV k="Hostname">{device.hostname || NA}</KV>
            <KV k="IP address">{device.ip || NA}</KV>
            <KV k="MAC address"><span className="mono">{device.mac || '—'}</span></KV>
            <KV k="OS">{device.os ?? NA}</KV>
            <KV k="Type">{device.device_type}</KV>
            <KV k="Wake-on-LAN">{device.wol_capable === 'yes' ? 'Confirmed working' : device.wol_capable === 'no' ? 'Does not appear to work' : 'Not confirmed yet (confirmed after a successful wake)'}</KV>
            <KV k="Groups">{device.group_ids?.length ? <span className="row wrap gap-xs">{device.group_ids.map((g) => <Pill key={g} tone="cyan">{groups.find((x) => x.id === g)?.name ?? g}</Pill>)}</span> : NA}</KV>
            <KV k="Reachability ports">{device.tcp_check_ports?.length ? device.tcp_check_ports.join(', ') : NA}</KV>
            <KV k="Last seen">{device.last_seen ? `${formatTime(device.last_seen)} (${relativeTime(device.last_seen)})` : 'never'}</KV>
            <KV k="Last checked">{device.status_checked_at ? relativeTime(device.status_checked_at) : 'never'}</KV>
            <KV k="Status changed">{device.status_changed_at ? relativeTime(device.status_changed_at) : NA}</KV>
            <KV k="Notes">{device.notes || NA}</KV>
          </dl>
        )}
        {tab === 'system' && <SystemTab d={device} />}
        {tab === 'history' && <HistoryTab d={device} tick={liveTick} />}
        {tab === 'agent' && <AgentTab d={device} onChanged={() => void reloadDevices()} />}
        {tab === 'troubleshoot' && <TroubleTab d={device} />}
      </div>
      {actions.dialog}
      {confirmDel && (
        <ConfirmDialog title={`Delete ${device.name}?`} confirmLabel="Delete device" danger busy={busy} onConfirm={remove} onCancel={() => setConfirmDel(false)}>
          <p>The device and its history are removed from the controller. The computer itself is not touched. Any installed agent stops being managed.</p>
        </ConfirmDialog>
      )}
    </Modal>
  );
}

function SystemTab({ d }: { d: Device }) {
  const s = d.sysinfo;
  if (!s) {
    return <Empty icon="server" title="No system information reported">System details come from the controller agent on the device. {d.power_method === 'agent' ? 'Enroll the agent in the “Agent & power” tab.' : 'This device does not use the agent.'}</Empty>;
  }
  const memPct = s.mem_total_bytes && s.mem_used_bytes !== null && s.mem_used_bytes !== undefined ? (s.mem_used_bytes / s.mem_total_bytes) * 100 : null;
  return (
    <div className="stack">
      <div className="muted small">Reported {s.reported_at ? relativeTime(s.reported_at) : 'at an unknown time'} by the agent. Values the device cannot read are shown as “not reported”.</div>
      <dl className="kv">
        <KV k="Hostname">{s.hostname || NA}</KV>
        <KV k="Operating system">{s.os_name ? `${s.os_name}${s.os_version ? ` (${s.os_version})` : ''}` : NA}</KV>
        <KV k="Kernel">{s.kernel || NA}</KV>
        <KV k="Architecture">{s.arch || NA}</KV>
        <KV k="Uptime">{s.uptime_secs === null || s.uptime_secs === undefined ? NA : formatDuration(s.uptime_secs)}</KV>
        <KV k="CPU">{s.cpu_model ? `${s.cpu_model}${s.cpu_cores ? ` · ${s.cpu_cores} cores` : ''}` : NA}</KV>
        <KV k="CPU usage">
          {s.cpu_usage_percent === null || s.cpu_usage_percent === undefined ? NA : <div className="stack-s"><span>{formatPercent(s.cpu_usage_percent)}</span><Meter value={s.cpu_usage_percent} /></div>}
        </KV>
        <KV k="Memory">
          {s.mem_total_bytes === null || s.mem_total_bytes === undefined ? NA : (
            <div className="stack-s">
              <span>{s.mem_used_bytes === null || s.mem_used_bytes === undefined ? 'usage not reported' : `${formatBytes(s.mem_used_bytes)} used`} of {formatBytes(s.mem_total_bytes)}{memPct !== null ? ` (${formatPercent(memPct)})` : ''}</span>
              <Meter value={memPct} />
            </div>
          )}
        </KV>
      </dl>
      <h3>Disks</h3>
      {s.disks && s.disks.length ? (
        <div className="stack-s">
          {s.disks.map((dk) => {
            const used = dk.total_bytes && dk.free_bytes !== null && dk.free_bytes !== undefined ? ((dk.total_bytes - dk.free_bytes) / dk.total_bytes) * 100 : null;
            return (
              <div key={dk.mount}>
                <div className="row between small"><span className="mono">{dk.mount}</span><span className="muted">{dk.free_bytes === null || dk.free_bytes === undefined ? 'free not reported' : `${formatBytes(dk.free_bytes)} free`} of {dk.total_bytes === null || dk.total_bytes === undefined ? 'unknown size' : formatBytes(dk.total_bytes)}</span></div>
                <Meter value={used} />
              </div>
            );
          })}
        </div>
      ) : <div className="na">not reported</div>}
      <h3>Temperatures</h3>
      {s.temperatures && s.temperatures.length ? (
        <dl className="kv">{s.temperatures.map((t, i) => <KV k={t.label || `Sensor ${i + 1}`} key={i}>{formatTemp(t.celsius)}</KV>)}</dl>
      ) : <div className="na">not reported (the device exposes no temperature sensors to the agent)</div>}
      <h3>Network adapters</h3>
      {s.networks && s.networks.length ? (
        <dl className="kv">{s.networks.map((n, i) => <KV k={n.name || `Adapter ${i + 1}`} key={i}><span className="mono">{(n.ips ?? []).join(', ') || 'no address'}{n.mac ? ` · ${n.mac}` : ''}</span></KV>)}</dl>
      ) : <div className="na">not reported</div>}
    </div>
  );
}

function HistoryTab({ d, tick }: { d: Device; tick: number }) {
  const [minutes, setMinutes] = useState(360);
  const m = useLoad<MetricPoint[]>(() => api.metrics(d.id, minutes), [d.id, minutes]);
  const o = useLoad(() => api.operations({ device_id: d.id, limit: 20 }), [d.id, tick]);
  const pts = m.data ?? [];
  const mem = useMemo(() => pts.map((p) => ({ t: p.t, v: p.mem_total ? ((p.mem_used ?? NaN) / p.mem_total) * 100 : null })), [pts]);
  return (
    <div className="stack">
      <div className="row between wrap">
        <h3>Live history</h3>
        <select style={{ width: 'auto' }} value={minutes} onChange={(e) => setMinutes(Number(e.target.value))} aria-label="Time range">
          <option value={60}>Last hour</option><option value={360}>Last 6 hours</option><option value={1440}>Last 24 hours</option><option value={10080}>Last 7 days</option>
        </select>
      </div>
      {m.loading && !m.data ? <Spinner /> : m.error ? <ErrorBox message={m.error} onRetry={() => void m.reload()} /> : (
        <div className="grid cols-2">
          <Sparkline label="CPU" unit="%" max={100} points={pts.map((p) => ({ t: p.t, v: p.cpu }))} />
          <Sparkline label="Memory" unit="%" max={100} color="var(--cyan)" points={mem} />
          <Sparkline label="Min. free disk (GB)" color="var(--amber)" min={0} points={pts.map((p) => ({ t: p.t, v: p.disk_free_min === null || p.disk_free_min === undefined ? null : p.disk_free_min / 1024 ** 3 }))} />
          <Sparkline label="Max temperature" unit=" °C" color="var(--red)" min={20} points={pts.map((p) => ({ t: p.t, v: p.temp_max }))} />
        </div>
      )}
      <h3>Operations for this device</h3>
      {o.loading && !o.data ? <Spinner /> : o.error ? <ErrorBox message={o.error} onRetry={() => void o.reload()} /> : (o.data ?? []).length === 0
        ? <div className="muted">No operations recorded for this device yet.</div>
        : <div className="ops-list">{(o.data ?? []).map((op) => <OpRow key={op.id} op={op} showDevice={false} />)}</div>}
    </div>
  );
}

function AgentTab({ d, onChanged }: { d: Device; onChanged: () => void }) {
  const { isAdmin, toast, toastError } = useApp();
  const [enr, setEnr] = useState<Enrollment | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmRevoke, setConfirmRevoke] = useState(false);
  const [now, setNow] = useState(Date.now() / 1000);
  useEffect(() => {
    if (!enr) return;
    const t = window.setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => window.clearInterval(t);
  }, [enr]);
  const a = d.agent;
  const enroll = async () => {
    setBusy(true);
    try { setEnr(await api.enroll(d.id)); setNow(Date.now() / 1000); }
    catch (e) { toastError(e, 'Agent enrollment'); }
    finally { setBusy(false); }
  };
  const revoke = async () => {
    setBusy(true);
    try { await api.revokeAgent(d.id); toast('ok', 'Agent access revoked.'); setConfirmRevoke(false); setEnr(null); onChanged(); }
    catch (e) { toastError(e, 'Revoke agent'); }
    finally { setBusy(false); }
  };
  return (
    <div className="stack">
      <h3>Power method</h3>
      <dl className="kv">
        <KV k="Method">{d.power_method === 'none' ? 'None configured (Wake-on-LAN only)' : d.power_method}</KV>
        {d.power_method === 'ssh' && <KV k="SSH">{d.ssh ? `${d.ssh.user}@${d.ip ?? '?'}:${d.ssh.port}${d.ssh.key_id ? ` (key ${d.ssh.key_id})` : ''}` : NA}</KV>}
      </dl>
      <h3>Agent</h3>
      <dl className="kv">
        <KV k="Status">{a && a.state !== 'none' ? <span className={`badge st-${a.state === 'online' ? 'online' : 'offline'}`}><span className="dot" />{a.state}</span> : 'Not installed'}</KV>
        <KV k="Enrolled">{a?.enrolled ? 'Yes' : 'No'}</KV>
        <KV k="Version">{a?.version || NA}</KV>
        <KV k="Last heartbeat">{a?.last_heartbeat ? `${relativeTime(a.last_heartbeat)}` : 'never'}</KV>
      </dl>
      {isAdmin ? (
        <div className="row wrap gap-s">
          <button className="btn accent" onClick={enroll} disabled={busy}><Icon name="key" size={15} />{a?.enrolled ? 'Re-enroll agent' : 'Enroll agent'}</button>
          {a?.enrolled && <button className="btn danger" onClick={() => setConfirmRevoke(true)}>Revoke agent</button>}
        </div>
      ) : <div className="muted small">Only administrators can enroll or revoke agents.</div>}
      {enr && (
        <div className="stack">
          <div className="notice"><Icon name="info" size={16} /><span>One-time enrollment code, valid for <strong>{countdown(enr.expires_at, now)}</strong>. Run one of the commands on the target computer as administrator / root.</span></div>
          <div className="big-code" aria-label="Enrollment code">{enr.code}</div>
          <CodeBlock label="Windows (PowerShell, as Administrator)" text={enr.command_windows} />
          <CodeBlock label="Linux (shell, with sudo)" text={enr.command_linux} />
          <dl className="kv"><KV k="Controller">{enr.controller_url}</KV><KV k="Fingerprint"><span className="mono">{enr.fingerprint}</span></KV></dl>
        </div>
      )}
      {confirmRevoke && (
        <ConfirmDialog title="Revoke agent?" confirmLabel="Revoke" danger busy={busy} onConfirm={revoke} onCancel={() => setConfirmRevoke(false)}>
          <p>The agent on {d.name} loses access immediately. Shutdown, restart and system information stop working until you enroll it again.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}

function TroubleTab({ d }: { d: Device }) {
  const [run, setRun] = useState(0);
  const t = useLoad(() => (run ? api.troubleshoot(d.id) : Promise.resolve(null)), [d.id, run]);
  return (
    <div className="stack">
      <p className="muted">Checks why a Wake-on-LAN packet might not wake this device: network path, MAC, broadcast address, and typical BIOS / adapter settings.</p>
      <div><button className="btn accent" onClick={() => setRun((n) => n + 1)} disabled={t.loading && run > 0}><Icon name="wifi" size={15} />{run ? 'Run again' : 'Run troubleshooting'}</button></div>
      {run > 0 && t.loading && <Spinner label="Running checks" />}
      {t.error && <ErrorBox message={t.error} onRetry={() => void t.reload()} />}
      {t.data && (
        <>
          <div className="table-wrap"><table className="tbl"><tbody>
            {(t.data.checks ?? []).map((c, i) => <tr key={i}><td className="nowrap"><CheckBadge status={c.status} /></td><td><strong>{c.name}</strong><div className="muted small">{c.detail}</div></td></tr>)}
          </tbody></table></div>
          {(t.data.advice ?? []).length > 0 && (
            <div className="panel"><h3 style={{ marginBottom: 8 }}>Advice</h3><ul className="steps-ol">{t.data.advice.map((x, i) => <li key={i}>{x}</li>)}</ul></div>
          )}
        </>
      )}
    </div>
  );
}
