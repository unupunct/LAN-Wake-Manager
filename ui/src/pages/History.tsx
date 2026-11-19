import { useCallback, useEffect, useState } from 'react';
import { api, friendlyMessage } from '../api/client';
import type { Operation } from '../api/types';
import { OpRow } from '../components/ops';
import { Empty, ErrorBox, PageHeader, Panel, Spinner } from '../components/ui';
import { useApp } from '../store';

const PAGE = 50;

export function HistoryPage() {
  const { devices, ops: liveOps, locationId } = useApp();
  const [state, setState] = useState('');
  const [device, setDevice] = useState('');
  const [kind, setKind] = useState('');
  const [rows, setRows] = useState<Operation[]>([]);
  const [loading, setLoading] = useState(true);
  const [more, setMore] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (reset: boolean, before?: number) => {
    setLoading(true);
    try {
      const r = await api.operations({ limit: PAGE, state: state || undefined, device_id: device || undefined, before });
      const list = r ?? [];
      setRows((old) => (reset ? list : [...old, ...list.filter((x) => !old.some((o) => o.id === x.id))]));
      setMore(list.length >= PAGE);
      setError(null);
    } catch (e) { setError(friendlyMessage(e)); }
    finally { setLoading(false); }
  }, [state, device]);

  useEffect(() => { void load(true); }, [load]);

  // merge live updates (state changes / new operations) into the visible list
  useEffect(() => {
    setRows((old) => {
      let changed = false;
      const map = new Map(liveOps.map((o) => [o.id, o]));
      const next = old.map((o) => { const n = map.get(o.id); if (n && n !== o) { changed = true; return n; } return o; });
      const newest = old[0]?.started_at ?? 0;
      const fresh = liveOps.filter((o) => !old.some((x) => x.id === o.id) && o.started_at >= newest && (!state || o.state === state) && (!device || o.device_id === device));
      if (fresh.length) changed = true;
      return changed ? [...fresh, ...next] : old;
    });
  }, [liveOps, state, device]);

  const shown = rows.filter((o) => (!kind || o.kind === kind) && (!locationId || !o.location_id || o.location_id === locationId));
  const locDevices = locationId ? devices.filter((d) => d.location_id === locationId) : devices;

  return (
    <>
      <PageHeader title="Operations" subtitle="Everything the controller did. A wake counts as successful only once the device was confirmed online; “packet sent, not online” means the magic packet went out but the device did not come up in time." />
      <div className="filters">
        <select value={state} onChange={(e) => setState(e.target.value)} aria-label="Filter by state">
          <option value="">Any state</option><option value="running">Running</option><option value="succeeded">Succeeded</option><option value="timeout">Timed out</option><option value="failed">Failed</option><option value="pending">Pending</option>
        </select>
        <select value={kind} onChange={(e) => setKind(e.target.value)} aria-label="Filter by action">
          <option value="">Any action</option><option value="wake">Wake</option><option value="shutdown">Shutdown</option><option value="restart">Restart</option><option value="cancel_shutdown">Cancel shutdown</option>
        </select>
        <select value={device} onChange={(e) => setDevice(e.target.value)} aria-label="Filter by device">
          <option value="">Any device</option>{locDevices.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
        </select>
        {(state || kind || device) && <button className="btn sm ghost" onClick={() => { setState(''); setKind(''); setDevice(''); }}>Clear filters</button>}
      </div>
      <Panel>
        {loading && rows.length === 0 ? <Spinner /> : error && rows.length === 0 ? <ErrorBox message={error} onRetry={() => void load(true)} /> : shown.length === 0 ? (
          <Empty icon="history" title="No operations match">{state || kind || device ? 'Try clearing the filters.' : 'Power actions and scheduled runs will be listed here.'}</Empty>
        ) : (
          <>
            <div className="ops-list">{shown.map((o) => <OpRow key={o.id} op={o} />)}</div>
            {error && <ErrorBox message={error} />}
            {more && <div style={{ textAlign: 'center', marginTop: 12 }}><button className="btn" disabled={loading} onClick={() => void load(false, rows[rows.length - 1]?.started_at)}>{loading ? 'Loading…' : 'Load older'}</button></div>}
          </>
        )}
      </Panel>
    </>
  );
}
