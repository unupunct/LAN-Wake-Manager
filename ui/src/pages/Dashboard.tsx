import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import type { Action, Location } from '../api/types';
import { BulkDialog } from '../components/actions';
import { Icon } from '../components/Icon';
import { OpRow } from '../components/ops';
import { Empty, ErrorBox, PageHeader, Panel, Pill, Spinner, useLoad } from '../components/ui';
import { relativeTime } from '../lib/format';
import { useApp } from '../store';

function Tile({ label, value, note, tone, icon, to, nz }: { label: string; value: number | string; note?: string; tone: string; icon: string; to?: string; nz?: boolean }) {
  const inner = (
    <>
      <div className="label"><Icon name={icon} size={15} />{label}</div>
      <div className={`value${nz ? ' nz' : ''}`}>{value}</div>
      {note && <div className="note">{note}</div>}
    </>
  );
  return to ? <Link to={to} className={`tile ${tone}`}>{inner}</Link> : <div className={`tile ${tone}`}>{inner}</div>;
}

function vpnLabel(l: Location): string {
  return { tailscale: 'Tailscale', wireguard: 'WireGuard', lan: 'LAN only', none: 'None' }[l.vpn] ?? String(l.vpn);
}

function relayTone(s: string): 'green' | 'cyan' | 'red' | 'muted' {
  return s === 'online' ? 'green' : s === 'builtin' ? 'cyan' : s === 'offline' ? 'red' : 'muted';
}

export function Dashboard() {
  const { scopedDevices, locationId, locations, canOperate, liveTick, loaded } = useApp();
  const nav = useNavigate();
  const { data, error, loading, reload } = useLoad(() => api.dashboard(), []);
  const [bulk, setBulk] = useState<Action | null>(null);

  // refresh dashboard (debounced) on live events
  const first = useRef(true);
  useEffect(() => {
    if (first.current) { first.current = false; return; }
    const t = window.setTimeout(() => void reload(true), 800);
    return () => window.clearTimeout(t);
  }, [liveTick, reload]);

  const totals = useMemo(() => {
    const online = scopedDevices.filter((d) => d.status === 'online').length;
    const unknown = scopedDevices.filter((d) => d.status === 'unknown').length;
    return { total: scopedDevices.length, online, unknown, offline: scopedDevices.filter((d) => d.status === 'offline' || d.status === 'shutting_down').length };
  }, [scopedDevices]);

  if (loading && !data) return <Spinner label="Loading dashboard" />;
  if (error && !data) return <ErrorBox message={error} onRetry={() => void reload()} />;
  if (!data) return null;

  const locs = locationId ? (data.locations ?? []).filter((l) => l.id === locationId) : (data.locations ?? locations);
  const ops = (data.recent_operations ?? []).filter((o) => !locationId || o.location_id === locationId);
  const relays = data.relays ?? { total: 0, online: 0 };
  const vpn = data.vpn;
  const upcoming = data.upcoming_schedules ?? [];
  const noDevices = loaded && scopedDevices.length === 0;

  return (
    <>
      <PageHeader title="Dashboard" subtitle={locationId ? `Showing ${locations.find((l) => l.id === locationId)?.name ?? 'location'}` : 'Overview of all network locations'} />

      <div className="tiles">
        <Tile label="Total devices" value={totals.total} tone="cyan" icon="devices" to="/devices" note={totals.unknown ? `${totals.unknown} unknown` : undefined} />
        <Tile label="Online" value={totals.online} tone="green" icon="power" to="/devices" />
        <Tile label="Offline" value={totals.offline} tone="amber" icon="stop" to="/devices" />
        <Tile label="Active relays" value={`${relays.online}/${relays.total}`} tone="cyan" icon="wifi" to="/locations" note="online of paired" />
        <Tile label="VPN connections" value={vpn?.active_connections ?? 0} tone="cyan" icon="vpn" to="/remote"
          note={vpn?.tailscale ? (vpn.tailscale.running ? 'Tailscale running' : vpn.tailscale.installed ? 'Tailscale stopped' : 'Tailscale not installed') : 'Tailscale status unknown'} />
        <Tile label="Pending schedules" value={data.pending_schedules ?? 0} tone="green" icon="schedule" to="/schedules" />
        <Tile label="Failed (24 h)" value={data.failed_operations_24h ?? 0} tone="red" icon="warn" nz={(data.failed_operations_24h ?? 0) > 0} to="/history" />
      </div>

      <Panel title="Quick actions">
        <div className="quick">
          <button className="btn primary-soft" disabled={!canOperate || noDevices} onClick={() => setBulk('wake')}><Icon name="power" />Wake all</button>
          <button className="btn" disabled={!canOperate || noDevices} onClick={() => setBulk('shutdown')}><Icon name="stop" />Shutdown all</button>
          <button className="btn" disabled={!canOperate || noDevices} onClick={() => setBulk('restart')}><Icon name="restart" />Restart all</button>
          <button className="btn accent" onClick={() => nav('/discovery')}><Icon name="discover" />Discover devices</button>
          <button className="btn" onClick={() => nav('/schedules', { state: { create: true } })}><Icon name="schedule" />Schedule operation</button>
          <button className="btn" onClick={() => nav('/devices/new')}><Icon name="plus" />Add device</button>
        </div>
      </Panel>

      <div className="two-col">
        <Panel title="Recent operations" actions={<Link to="/history" className="btn sm ghost">View all</Link>}>
          {ops.length === 0 ? <Empty icon="history" title="No operations yet">Wake, shutdown and restart actions will show up here with their real outcome.</Empty> : (
            <div className="ops-list">{ops.slice(0, 8).map((o) => <OpRow key={o.id} op={o} />)}</div>
          )}
        </Panel>
        <Panel title="Upcoming schedules" actions={<Link to="/schedules" className="btn sm ghost">Manage</Link>}>
          {upcoming.length === 0 ? <Empty icon="schedule" title="Nothing scheduled">Create a schedule to wake or shut down devices automatically.</Empty> : (
            <div className="ops-list">
              {upcoming.slice(0, 6).map((s) => (
                <div className="op-row" key={s.schedule_id}>
                  <div className={`kind-ico kind-${s.action}`}><Icon name="schedule" size={16} /></div>
                  <div className="what"><strong>{s.name}</strong><span>{s.next_run ? new Date(s.next_run * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) : 'No next run'}</span></div>
                  <Pill tone="cyan">{s.action}</Pill>
                  <span className="muted small nowrap">{relativeTime(s.next_run)}</span>
                </div>
              ))}
            </div>
          )}
        </Panel>
      </div>

      <Panel title="Locations" actions={<Link to="/locations" className="btn sm ghost">Manage</Link>}>
        {locs.length === 0 ? <Empty icon="location" title="No locations">Add a network location to start managing devices.</Empty> : (
          <div className="grid auto">
            {locs.map((l) => {
              const off = Math.max(0, (l.device_count ?? 0) - (l.online_count ?? 0));
              const pct = l.device_count ? (l.online_count / l.device_count) * 100 : 0;
              return (
                <div className="loc-card" key={l.id} style={{ ['--lc' as string]: l.color || 'var(--cyan)' }}>
                  <div className="row between"><h3>{l.name}</h3><Pill tone={l.controller_status === 'online' ? 'green' : 'amber'}>Controller {l.controller_status}</Pill></div>
                  <div className="row wrap gap-s">
                    <Pill tone={relayTone(l.relay_status)}>{l.relay_status === 'builtin' ? 'Built-in relay' : `Relay ${l.relay_status}`}</Pill>
                    <Pill tone={l.vpn === 'none' ? 'muted' : 'cyan'}>VPN: {vpnLabel(l)}</Pill>
                  </div>
                  <div>
                    <div className="row between small"><span className="muted">{l.device_count} devices</span><span><b style={{ color: 'var(--green)' }}>{l.online_count}</b> online · {off} offline</span></div>
                    <div className="bar" style={{ marginTop: 6 }} aria-hidden="true"><span style={{ width: `${pct}%` }} /></div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Panel>

      {bulk && (
        <BulkDialog action={bulk} devices={scopedDevices} scope={locationId ? `all devices in ${locations.find((l) => l.id === locationId)?.name ?? 'location'}` : 'all devices'} onClose={() => setBulk(null)} />
      )}
    </>
  );
}
