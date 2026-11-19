import { useCallback, useEffect, useState } from 'react';
import { api, friendlyMessage } from '../api/client';
import type { AuditEntry } from '../api/types';
import { Empty, ErrorBox, PageHeader, Panel, Pill, Spinner } from '../components/ui';
import { formatTime } from '../lib/format';
import { useApp } from '../store';

const PAGE = 100;

export function AuditPage() {
  const { isAdmin } = useApp();
  const [rows, setRows] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [more, setMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [q, setQ] = useState('');

  const load = useCallback(async (reset: boolean, before?: number) => {
    setLoading(true);
    try {
      const r = (await api.audit({ limit: PAGE, before })) ?? [];
      setRows((old) => (reset ? r : [...old, ...r]));
      setMore(r.length >= PAGE);
      setError(null);
    } catch (e) { setError(friendlyMessage(e)); }
    finally { setLoading(false); }
  }, []);
  useEffect(() => { if (isAdmin) void load(true); }, [isAdmin, load]);

  if (!isAdmin) return <Panel><Empty icon="audit" title="Administrators only">The audit log can only be viewed with an administrator account.</Empty></Panel>;
  const t = q.trim().toLowerCase();
  const shown = rows.filter((r) => !t || [r.actor, r.action, r.target, r.detail, r.ip, r.result].some((x) => (x ?? '').toString().toLowerCase().includes(t)));
  return (
    <>
      <PageHeader title="Audit log" subtitle="Who did what, when and from where. Logins, power operations, configuration changes and revocations." />
      <div className="filters"><input style={{ maxWidth: 360 }} type="search" placeholder="Filter by actor, action, target or address" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Filter audit log" /></div>
      <Panel className="flush">
        {loading && rows.length === 0 ? <Spinner /> : error && rows.length === 0 ? <ErrorBox message={error} onRetry={() => void load(true)} /> : shown.length === 0 ? <Empty icon="audit" title="No entries" /> : (
          <div className="table-wrap"><table className="tbl stackm">
            <thead><tr><th>Time</th><th>Actor</th><th>Action</th><th>Target</th><th>Result</th><th>Address</th><th>Detail</th></tr></thead>
            <tbody>{shown.map((r) => (
              <tr key={r.id}><td className="nowrap">{formatTime(r.ts)}</td><td>{r.actor}</td><td className="mono">{r.action}</td><td>{r.target || '—'}</td>
                <td><Pill tone={/^(ok|success|succeeded)/i.test(r.result) ? 'green' : /fail|denied|error/i.test(r.result) ? 'red' : 'muted'}>{r.result}</Pill></td>
                <td className="mono">{r.ip || '—'}</td><td className="muted" style={{ maxWidth: 360, overflowWrap: 'anywhere' }}>{r.detail}</td></tr>
            ))}</tbody>
          </table></div>
        )}
      </Panel>
      {more && <div style={{ textAlign: 'center' }}><button className="btn" disabled={loading} onClick={() => void load(false, rows[rows.length - 1]?.ts)}>{loading ? 'Loading…' : 'Load older'}</button></div>}
    </>
  );
}
