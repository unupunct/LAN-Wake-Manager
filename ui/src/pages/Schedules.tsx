import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import type { Action, RecKind, Recurrence, Schedule, ScheduleInput } from '../api/types';
import { Icon } from '../components/Icon';
import { ConfirmDialog, Empty, ErrorBox, Field, Modal, PageHeader, Panel, Pill, Spinner, Toggle, useLoad } from '../components/ui';
import { formatTime, relativeTime } from '../lib/format';
import { describeRecurrence, describeSchedule, emptyRecurrence, SHORT_DAYS } from '../lib/recurrence';
import { isValidDate, isValidTime } from '../lib/validate';
import { useApp } from '../store';

const blank = (): ScheduleInput => ({
  name: '', enabled: true, action: 'wake', target: { device_ids: [], group_ids: [], location_ids: [] },
  recurrence: emptyRecurrence('daily'), start_date: null, end_date: null, exceptions: [],
  missed_policy: 'skip', missed_grace_minutes: 60, retry: { max_attempts: 3, interval_secs: 60 }, executor: 'controller', delay_secs: 0,
});

function MultiPick({ legend, items, value, onChange }: { legend: string; items: { id: string; label: string }[]; value: string[]; onChange: (v: string[]) => void }) {
  if (items.length === 0) return null;
  return (
    <fieldset><legend>{legend}</legend>
      <div className="checks">{items.map((i) => (
        <label key={i.id} className="chip-check"><input type="checkbox" checked={value.includes(i.id)} onChange={() => onChange(value.includes(i.id) ? value.filter((x) => x !== i.id) : [...value, i.id])} />{i.label}</label>
      ))}</div>
    </fieldset>
  );
}

function Editor({ sched, onClose, onSaved }: { sched: Schedule | null; onClose: () => void; onSaved: () => void }) {
  const { devices, groups, locations, toast, toastError } = useApp();
  const [f, setF] = useState<ScheduleInput>(() => {
    if (!sched) return blank();
    const { id: _id, next_run: _n, last_run: _l, last_state: _s, ...rest } = sched; // eslint-disable-line @typescript-eslint/no-unused-vars
    return { ...blank(), ...rest, target: { device_ids: rest.target?.device_ids ?? [], group_ids: rest.target?.group_ids ?? [], location_ids: rest.target?.location_ids ?? [] }, exceptions: rest.exceptions ?? [] };
  });
  const [exc, setExc] = useState('');
  const [busy, setBusy] = useState(false);
  const [touched, setTouched] = useState(false);
  const set = <K extends keyof ScheduleInput>(k: K, v: ScheduleInput[K]) => setF((o) => ({ ...o, [k]: v }));
  const rec = f.recurrence;
  const setRec = (p: Partial<Recurrence>) => setF((o) => ({ ...o, recurrence: { ...o.recurrence, ...p } }));
  const t = f.target;
  const nTargets = t.device_ids.length + t.group_ids.length + t.location_ids.length;

  const targetLocations = useMemo(() => {
    const s = new Set<string>(t.location_ids);
    devices.forEach((d) => { if (t.device_ids.includes(d.id) || d.group_ids?.some((g) => t.group_ids.includes(g))) s.add(d.location_id); });
    return s;
  }, [devices, t]);

  const errs = {
    name: !f.name.trim() ? 'Give the schedule a name.' : null,
    target: nTargets === 0 ? 'Choose at least one device, group or location.' : null,
    at: rec.kind !== 'custom' && !(rec.at && isValidTime(rec.at)) ? 'Enter a time like 07:30.' : rec.kind === 'custom' && rec.at && !isValidTime(rec.at) ? 'Enter a time like 07:30 or leave empty.' : null,
    date: rec.kind === 'once' && !(rec.date && isValidDate(rec.date)) ? 'Choose a date.' : null,
    weekdays: rec.kind === 'weekly' && !(rec.weekdays && rec.weekdays.length) ? 'Choose at least one weekday.' : null,
    dom: rec.kind === 'monthly' && !(rec.day_of_month && rec.day_of_month >= 1 && rec.day_of_month <= 31) ? 'Day must be 1–31.' : null,
    interval: rec.kind === 'custom' && !(rec.interval && rec.interval.every >= 1) ? 'Interval must be at least 1.' : null,
    range: f.start_date && f.end_date && f.end_date < f.start_date ? 'The end date is before the start date.' : null,
    relay: f.executor === 'relay' && f.action !== 'wake' ? 'Only wake schedules can run on the relay.' : f.executor === 'relay' && targetLocations.size !== 1 ? 'Relay schedules must target devices of exactly one location.' : null,
  };
  const invalid = Object.values(errs).some(Boolean);
  const show = (k: keyof typeof errs) => (touched ? errs[k] : null);

  const addExc = () => {
    if (!isValidDate(exc)) return;
    if (!f.exceptions.includes(exc)) set('exceptions', [...f.exceptions, exc].sort());
    setExc('');
  };

  const changeKind = (k: RecKind) => setF((o) => ({ ...o, recurrence: { ...emptyRecurrence(k), at: k === 'custom' ? null : o.recurrence.at || '07:30' } }));

  const save = async () => {
    setTouched(true);
    if (invalid) return;
    setBusy(true);
    try {
      const body: ScheduleInput = { ...f, name: f.name.trim(), start_date: f.start_date || null, end_date: f.end_date || null };
      if (sched) await api.updateSchedule(sched.id, body); else await api.createSchedule(body);
      toast('ok', sched ? 'Schedule saved.' : 'Schedule created.');
      onSaved();
    } catch (e) { toastError(e, 'Save schedule'); setBusy(false); }
  };

  return (
    <Modal wide title={sched ? `Edit schedule` : 'New schedule'} onClose={onClose} locked={busy}
      footer={<><button className="btn" onClick={onClose} disabled={busy}>Cancel</button><button className="btn primary" onClick={save} disabled={busy}>{busy ? 'Saving…' : 'Save schedule'}</button></>}>
      <div className="stack">
        <div className="form-grid">
          <Field label="Name *" error={show('name')}><input data-autofocus value={f.name} onChange={(e) => set('name', e.target.value)} placeholder="Wake office PCs" /></Field>
          <Field label="Action">
            <select value={f.action} onChange={(e) => { const a = e.target.value as Action; setF((o) => ({ ...o, action: a, executor: a === 'wake' ? o.executor : 'controller' })); }}>
              <option value="wake">Wake</option><option value="shutdown">Shutdown</option><option value="restart">Restart</option>
            </select>
          </Field>
        </div>
        {f.action !== 'wake' && <div className="notice warn"><Icon name="warn" size={16} /><span>{f.action === 'shutdown' ? 'Shutdown' : 'Restart'} schedules run from the controller and need the agent (or another power method) on each device. They will not ask for confirmation when they fire.</span></div>}

        <fieldset><legend>Targets *</legend>
          <div className="stack">
            <MultiPick legend="Locations" items={locations.map((l) => ({ id: l.id, label: l.name }))} value={t.location_ids} onChange={(v) => set('target', { ...t, location_ids: v })} />
            <MultiPick legend="Groups" items={groups.map((g) => ({ id: g.id, label: g.name }))} value={t.group_ids} onChange={(v) => set('target', { ...t, group_ids: v })} />
            <MultiPick legend="Devices" items={devices.map((d) => ({ id: d.id, label: d.name }))} value={t.device_ids} onChange={(v) => set('target', { ...t, device_ids: v })} />
            {show('target') && <small className="err-text">{errs.target}</small>}
          </div>
        </fieldset>

        <fieldset><legend>Recurrence</legend>
          <div className="stack">
            <div className="form-grid">
              <Field label="Repeats">
                <select value={rec.kind} onChange={(e) => changeKind(e.target.value as RecKind)}>
                  <option value="once">Once</option><option value="daily">Daily</option><option value="weekly">Weekly</option><option value="monthly">Monthly</option><option value="custom">Custom interval</option>
                </select>
              </Field>
              <Field label={rec.kind === 'custom' ? 'Starting at (optional)' : 'Time (controller timezone)'} error={show('at')}>
                <input type="time" value={rec.at ?? ''} onChange={(e) => setRec({ at: e.target.value || null })} />
              </Field>
              {rec.kind === 'once' && <Field label="Date" error={show('date')}><input type="date" value={rec.date ?? ''} onChange={(e) => setRec({ date: e.target.value })} /></Field>}
              {rec.kind === 'monthly' && <Field label="Day of month" error={show('dom')} hint="31 uses the last day in shorter months."><input type="number" min={1} max={31} value={rec.day_of_month ?? 1} onChange={(e) => setRec({ day_of_month: Number(e.target.value) })} /></Field>}
              {rec.kind === 'custom' && (
                <Field label="Every" error={show('interval')}>
                  <div className="row gap-s">
                    <input type="number" min={1} value={rec.interval?.every ?? 1} onChange={(e) => setRec({ interval: { every: Number(e.target.value), unit: rec.interval?.unit ?? 'hours' } })} />
                    <select value={rec.interval?.unit ?? 'hours'} onChange={(e) => setRec({ interval: { every: rec.interval?.every ?? 1, unit: e.target.value as 'minutes' | 'hours' | 'days' } })}>
                      <option value="minutes">minutes</option><option value="hours">hours</option><option value="days">days</option>
                    </select>
                  </div>
                </Field>
              )}
            </div>
            {rec.kind === 'weekly' && (
              <div className="field"><span>Weekdays</span>
                <div className="dow" role="group" aria-label="Weekdays">{SHORT_DAYS.map((n, i) => {
                  const d = i + 1; const on = rec.weekdays?.includes(d) ?? false;
                  return <label key={d} className="chip-check"><input type="checkbox" checked={on} onChange={() => setRec({ weekdays: on ? (rec.weekdays ?? []).filter((x) => x !== d) : [...(rec.weekdays ?? []), d].sort() })} />{n}</label>;
                })}</div>
                {show('weekdays') && <small className="err-text">{errs.weekdays}</small>}
              </div>
            )}
            <div className="form-grid">
              <Field label="Start date (optional)"><input type="date" value={f.start_date ?? ''} onChange={(e) => set('start_date', e.target.value || null)} /></Field>
              <Field label="End date (optional)" error={show('range')}><input type="date" value={f.end_date ?? ''} onChange={(e) => set('end_date', e.target.value || null)} /></Field>
            </div>
            <div className="field"><span>Skip these dates (exceptions)</span>
              <div className="row gap-s"><input type="date" value={exc} onChange={(e) => setExc(e.target.value)} aria-label="Exception date" /><button type="button" className="btn" onClick={addExc} disabled={!isValidDate(exc)}>Add</button></div>
              {f.exceptions.length > 0 && <div className="row wrap gap-xs">{f.exceptions.map((d) => <span key={d} className="pill pill-amber">{d} <button type="button" className="linklike" aria-label={`Remove ${d}`} onClick={() => set('exceptions', f.exceptions.filter((x) => x !== d))}>×</button></span>)}</div>}
            </div>
            <div className="notice"><Icon name="schedule" size={16} /><span><strong>Summary:</strong> {f.action === 'wake' ? 'Wake' : f.action === 'shutdown' ? 'Shut down' : 'Restart'} — {describeSchedule(f)}.</span></div>
          </div>
        </fieldset>

        <fieldset><legend>Reliability</legend>
          <div className="form-grid">
            <Field label="If the controller was down at run time" hint="Skip the missed run, or run it once on return (within the grace period).">
              <select value={f.missed_policy} onChange={(e) => set('missed_policy', e.target.value as 'skip' | 'run_once')}><option value="skip">Skip it</option><option value="run_once">Run once when back</option></select>
            </Field>
            <Field label="Grace period (minutes)"><input type="number" min={0} value={f.missed_grace_minutes} disabled={f.missed_policy === 'skip'} onChange={(e) => set('missed_grace_minutes', Number(e.target.value))} /></Field>
            <Field label="Retry attempts"><input type="number" min={1} max={10} value={f.retry.max_attempts} onChange={(e) => set('retry', { ...f.retry, max_attempts: Number(e.target.value) })} /></Field>
            <Field label="Retry interval (s)"><input type="number" min={5} value={f.retry.interval_secs} onChange={(e) => set('retry', { ...f.retry, interval_secs: Number(e.target.value) })} /></Field>
            {f.action !== 'wake' && <Field label="Delay (seconds)" hint="Time between the schedule firing and the command."><input type="number" min={0} value={f.delay_secs} onChange={(e) => set('delay_secs', Number(e.target.value))} /></Field>}
            <Field label="Executor" error={show('relay')} hint="The relay can run wake schedules even while the controller is down.">
              <select value={f.executor} onChange={(e) => set('executor', e.target.value as 'controller' | 'relay')}><option value="controller">Controller</option><option value="relay" disabled={f.action !== 'wake'}>Relay (wake only)</option></select>
            </Field>
          </div>
        </fieldset>
        <Toggle checked={f.enabled} onChange={(v) => set('enabled', v)} label="Enabled" />
        {touched && invalid && <span className="err-text">Please fix the highlighted fields.</span>}
      </div>
    </Modal>
  );
}

export function SchedulesPage() {
  const { canOperate, devices, groups, locations, toast, toastError, liveTick } = useApp();
  const { data, error, loading, reload } = useLoad(() => api.schedules(), []);
  const [edit, setEdit] = useState<Schedule | 'new' | null>(null);
  const [del, setDel] = useState<Schedule | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const routeState = useLocation().state as { create?: boolean } | null;
  const nav = useNavigate();

  useEffect(() => {
    if (routeState?.create) { setEdit('new'); nav('.', { replace: true, state: null }); }
  }, [routeState, nav]);
  useEffect(() => { if (liveTick) void reload(true); }, [liveTick, reload]);

  const targetText = (s: Schedule) => {
    const parts: string[] = [];
    const t = s.target ?? { device_ids: [], group_ids: [], location_ids: [] };
    t.location_ids?.forEach((id) => parts.push(locations.find((l) => l.id === id)?.name ?? id));
    t.group_ids?.forEach((id) => parts.push(`${groups.find((g) => g.id === id)?.name ?? id} (group)`));
    t.device_ids?.forEach((id) => parts.push(devices.find((d) => d.id === id)?.name ?? id));
    return parts.length ? parts.join(', ') : 'No targets';
  };

  const toggle = async (s: Schedule, enabled: boolean) => {
    setBusy(s.id);
    try { const { id, next_run: _n, last_run: _l, last_state: _s, ...rest } = s; await api.updateSchedule(id, { ...rest, enabled }); await reload(true); } // eslint-disable-line @typescript-eslint/no-unused-vars
    catch (e) { toastError(e, 'Update schedule'); }
    finally { setBusy(null); }
  };
  const runNow = async (s: Schedule) => {
    setBusy(s.id);
    try { await api.runSchedule(s.id); toast('ok', `“${s.name}” started. Follow it on the Operations page.`); await reload(true); }
    catch (e) { toastError(e, 'Run now'); }
    finally { setBusy(null); }
  };
  const remove = async () => {
    if (!del) return;
    setBusy(del.id);
    try { await api.deleteSchedule(del.id); toast('ok', 'Schedule deleted.'); setDel(null); await reload(true); }
    catch (e) { toastError(e, 'Delete schedule'); }
    finally { setBusy(null); }
  };

  const list = data ?? [];
  return (
    <>
      <PageHeader title="Schedules" subtitle="Wake, shut down or restart devices automatically. A sleeping computer cannot run its own schedule, so wake schedules run on the controller or relay.">
        {canOperate && <button className="btn primary" onClick={() => setEdit('new')}><Icon name="plus" size={16} />New schedule</button>}
      </PageHeader>
      {loading && !data ? <Spinner /> : error && !data ? <ErrorBox message={error} onRetry={() => void reload()} /> : list.length === 0 ? (
        <Panel><Empty icon="schedule" title="No schedules yet" action={canOperate ? <button className="btn primary" onClick={() => setEdit('new')}>Create a schedule</button> : undefined}>For example: wake the office PCs every weekday at 07:30.</Empty></Panel>
      ) : (
        <Panel className="flush">
          <div className="table-wrap">
            <table className="tbl stackm">
              <thead><tr><th>Enabled</th><th>Schedule</th><th>When</th><th>Next run</th><th>Last run</th><th aria-label="Actions" /></tr></thead>
              <tbody>
                {list.map((s) => (
                  <tr key={s.id}>
                    <td><Toggle checked={s.enabled} disabled={!canOperate || busy === s.id} onChange={(v) => void toggle(s, v)} label={s.enabled ? 'On' : 'Off'} /></td>
                    <td style={{ maxWidth: 280 }}><strong>{s.name}</strong> <Pill tone="cyan">{s.action}</Pill> {s.executor === 'relay' && <Pill>relay</Pill>}<div className="muted small" style={{ overflowWrap: 'anywhere' }}>{targetText(s)}</div></td>
                    <td style={{ maxWidth: 300 }}>{describeSchedule(s)}</td>
                    <td className="nowrap">{s.enabled && s.next_run ? <>{formatTime(s.next_run)}<div className="muted small">{relativeTime(s.next_run)}</div></> : <span className="na">{s.enabled ? 'none planned' : 'paused'}</span>}</td>
                    <td className="nowrap">{s.last_run ? <>{relativeTime(s.last_run)}<div>{s.last_state && <Pill tone={s.last_state === 'succeeded' ? 'green' : s.last_state === 'partial' || s.last_state === 'timeout' ? 'amber' : 'red'}>{s.last_state}</Pill>}</div></> : <span className="na">never</span>}</td>
                    <td className="actions">
                      {canOperate && <>
                        <button className="btn sm" disabled={busy === s.id} onClick={() => void runNow(s)} title="Run now"><Icon name="play" size={13} />Run now</button>{' '}
                        <button className="btn sm ghost" onClick={() => setEdit(s)} aria-label={`Edit ${s.name}`}><Icon name="edit" size={14} /></button>
                        <button className="btn sm ghost" onClick={() => setDel(s)} aria-label={`Delete ${s.name}`}><Icon name="trash" size={14} /></button>
                      </>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      )}
      {edit && <Editor sched={edit === 'new' ? null : edit} onClose={() => setEdit(null)} onSaved={() => { setEdit(null); void reload(true); }} />}
      {del && <ConfirmDialog title={`Delete “${del.name}”?`} confirmLabel="Delete schedule" danger busy={busy === del.id} onConfirm={remove} onCancel={() => setDel(null)}><p>{describeRecurrence(del.recurrence)}. It will no longer run.</p></ConfirmDialog>}
    </>
  );
}
