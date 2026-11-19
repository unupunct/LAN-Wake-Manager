import { useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import type { Action, Device } from '../api/types';
import { BulkDialog, DeviceButtons, useDeviceActions } from '../components/actions';
import { DeviceDrawer, deviceIcon } from '../components/DeviceDrawer';
import { Icon } from '../components/Icon';
import { Empty, PageHeader, Panel, Pill, Segmented, Spinner, StatusBadge } from '../components/ui';
import { relativeTime } from '../lib/format';
import { loadPref, savePref } from '../lib/storage';
import { useApp } from '../store';

type View = 'grid' | 'table' | 'list';
const STATUS_ORDER: Record<string, number> = { online: 0, waking: 1, shutting_down: 2, unknown: 3, offline: 4 };

export function DevicesPage() {
  const { scopedDevices, locations, groups, isAdmin, loaded } = useApp();
  const { id } = useParams();
  const nav = useNavigate();
  const actions = useDeviceActions();
  const [view, setViewState] = useState<View>(() => loadPref<View>('devview', 'grid'));
  const [q, setQ] = useState('');
  const [status, setStatus] = useState('');
  const [group, setGroup] = useState('');
  const [os, setOs] = useState('');
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [bulk, setBulk] = useState<Action | null>(null);
  const setView = (v: View) => { setViewState(v); savePref('devview', v); };

  const list = useMemo(() => {
    const t = q.trim().toLowerCase();
    return scopedDevices
      .filter((d) => (!status || d.status === status) && (!group || d.group_ids?.includes(group)) && (!os || (d.os ?? 'other') === os))
      .filter((d) => !t || [d.name, d.hostname, d.ip, d.mac].some((x) => (x ?? '').toLowerCase().includes(t)))
      .sort((a, b) => (STATUS_ORDER[a.status] ?? 9) - (STATUS_ORDER[b.status] ?? 9) || a.name.localeCompare(b.name));
  }, [scopedDevices, q, status, group, os]);

  const locName = (d: Device) => locations.find((l) => l.id === d.location_id)?.name ?? '';
  const open = (d: Device) => nav(`/devices/${d.id}`);
  const toggle = (d: Device) => setSel((s) => { const n = new Set(s); if (n.has(d.id)) n.delete(d.id); else n.add(d.id); return n; });
  const allSel = list.length > 0 && list.every((d) => sel.has(d.id));
  const toggleAll = () => setSel(allSel ? new Set() : new Set(list.map((d) => d.id)));
  const selected = list.filter((d) => sel.has(d.id));
  const filtered = !!(q || status || group || os);

  const check = (d: Device) => (
    <input type="checkbox" checked={sel.has(d.id)} onChange={() => toggle(d)} onClick={(e) => e.stopPropagation()} aria-label={`Select ${d.name}`} />
  );

  return (
    <>
      <PageHeader title="Devices" subtitle={`${list.length} of ${scopedDevices.length} devices`}>
        <Segmented<View> label="View mode" value={view} onChange={setView} options={[
          { value: 'grid', label: <><Icon name="grid" size={15} />Cards</>, title: 'Grid cards' },
          { value: 'table', label: <><Icon name="table" size={15} />Table</>, title: 'Table' },
          { value: 'list', label: <><Icon name="list" size={15} />List</>, title: 'Compact list' },
        ]} />
        <Link to="/discovery" className="btn accent"><Icon name="discover" size={16} />Discover</Link>
        {isAdmin && <Link to="/devices/new" className="btn primary"><Icon name="plus" size={16} />Add device</Link>}
      </PageHeader>

      <div className="filters">
        <div className="search"><Icon name="search" size={16} /><input type="search" placeholder="Search name, host, IP or MAC" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search devices" /></div>
        <select value={status} onChange={(e) => setStatus(e.target.value)} aria-label="Filter by status">
          <option value="">Any status</option><option value="online">Online</option><option value="offline">Offline</option><option value="waking">Waking</option><option value="shutting_down">Shutting down</option><option value="unknown">Unknown</option>
        </select>
        <select value={group} onChange={(e) => setGroup(e.target.value)} aria-label="Filter by group">
          <option value="">Any group</option>{groups.map((g) => <option key={g.id} value={g.id}>{g.name}</option>)}
        </select>
        <select value={os} onChange={(e) => setOs(e.target.value)} aria-label="Filter by operating system">
          <option value="">Any OS</option><option value="windows">Windows</option><option value="linux">Linux</option><option value="macos">macOS</option><option value="other">Other / unknown</option>
        </select>
        {filtered && <button className="btn sm ghost" onClick={() => { setQ(''); setStatus(''); setGroup(''); setOs(''); }}>Clear filters</button>}
        {view !== 'table' && list.length > 0 && <button className="btn sm ghost" onClick={toggleAll}>{allSel ? 'Deselect all' : 'Select all'}</button>}
      </div>

      {selected.length > 0 && (
        <div className="bulkbar" role="region" aria-label="Bulk actions">
          <strong>{selected.length} selected</strong>
          <div className="grow" />
          <button className="btn sm primary-soft" disabled={!actions.canOperate} onClick={() => setBulk('wake')}><Icon name="power" size={14} />Wake</button>
          <button className="btn sm" disabled={!actions.canOperate} onClick={() => setBulk('shutdown')}><Icon name="stop" size={14} />Shutdown</button>
          <button className="btn sm" disabled={!actions.canOperate} onClick={() => setBulk('restart')}><Icon name="restart" size={14} />Restart</button>
          <button className="btn sm ghost" onClick={() => setSel(new Set())}>Clear</button>
        </div>
      )}

      {!loaded ? <Spinner label="Loading devices" /> : list.length === 0 ? (
        <Panel>
          {scopedDevices.length === 0 ? (
            <Empty icon="devices" title="No devices yet" action={<div className="row gap-s"><Link to="/discovery" className="btn accent">Discover devices</Link>{isAdmin && <Link to="/devices/new" className="btn primary">Add manually</Link>}</div>}>
              Scan your network to find computers, or add one by its MAC address.
            </Empty>
          ) : <Empty icon="search" title="No devices match">Try changing or clearing the filters.</Empty>}
        </Panel>
      ) : view === 'grid' ? (
        <div className="grid auto">
          {list.map((d) => (
            <div key={d.id} className={`dev-card ${d.status}${sel.has(d.id) ? ' selected' : ''}`} onClick={() => open(d)}>
              <div className="sel">{check(d)}</div>
              <div className="top">
                <div className="dev-ico"><Icon name={deviceIcon(d.device_type)} size={20} /></div>
                <div className="grow">
                  <button className="dev-name linklike" onClick={(e) => { e.stopPropagation(); open(d); }}>{d.name}</button>
                  <div className="dev-meta">{d.ip || 'IP unknown'} · {locName(d)}</div>
                </div>
              </div>
              <div className="row between"><StatusBadge status={d.status} /><span className="muted small">{d.status === 'online' ? `seen ${relativeTime(d.last_seen)}` : d.last_seen ? `last seen ${relativeTime(d.last_seen)}` : 'never seen'}</span></div>
              <div className="facts">
                {d.os && <Pill>{d.os}</Pill>}
                {d.agent && d.agent.state !== 'none' && <Pill tone={d.agent.state === 'online' ? 'green' : 'muted'}>agent {d.agent.state}</Pill>}
                {d.wol_capable === 'yes' && <Pill tone="green">WoL confirmed</Pill>}
                {!d.mac && <Pill tone="red">no MAC</Pill>}
              </div>
              <DeviceButtons d={d} actions={actions} />
            </div>
          ))}
        </div>
      ) : view === 'table' ? (
        <Panel className="flush">
          <div className="table-wrap">
            <table className="tbl">
              <thead><tr>
                <th className="tbl-select"><input type="checkbox" checked={allSel} onChange={toggleAll} aria-label="Select all devices" /></th>
                <th>Name</th><th>Status</th><th>IP</th><th>MAC</th><th>Location</th><th>OS</th><th>Last seen</th><th aria-label="Actions" />
              </tr></thead>
              <tbody>
                {list.map((d) => (
                  <tr key={d.id} className="clickable" onClick={() => open(d)}>
                    <td>{check(d)}</td>
                    <td><button className="linklike strong" onClick={(e) => { e.stopPropagation(); open(d); }}>{d.name}</button>{d.hostname && <div className="muted small">{d.hostname}</div>}</td>
                    <td><StatusBadge status={d.status} /></td>
                    <td className="mono">{d.ip || '—'}</td>
                    <td className="mono">{d.mac || '—'}</td>
                    <td>{locName(d)}</td>
                    <td>{d.os ?? '—'}</td>
                    <td className="muted nowrap">{relativeTime(d.last_seen)}</td>
                    <td className="actions"><DeviceButtons d={d} actions={actions} onDetails={() => open(d)} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      ) : (
        <Panel className="flush">
          {list.map((d) => (
            <div key={d.id} className={`dev-list-row${sel.has(d.id) ? ' selected' : ''}`} onClick={() => open(d)}>
              {check(d)}
              <Icon name={deviceIcon(d.device_type)} size={16} className="muted" />
              <span className="name">{d.name}</span>
              <StatusBadge status={d.status} small />
              <span className="ip mono">{d.ip || '—'}</span>
              <span className="loc">{locName(d)}</span>
              <DeviceButtons d={d} actions={actions} />
            </div>
          ))}
        </Panel>
      )}

      {id && <DeviceDrawer id={id} />}
      {actions.dialog}
      {bulk && <BulkDialog action={bulk} devices={selected} scope="selected devices" onClose={() => setBulk(null)} onDone={() => setSel(new Set())} />}
    </>
  );
}
