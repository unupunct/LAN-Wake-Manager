import { useState } from 'react';
import { api } from '../api/client';
import type { Action, Group } from '../api/types';
import { BulkDialog } from '../components/actions';
import { Icon } from '../components/Icon';
import { ConfirmDialog, Empty, Field, Modal, PageHeader, Spinner } from '../components/ui';
import { deviceToInput } from '../lib/device';
import { useApp } from '../store';

const ICONS = ['desktop', 'gamepad', 'server', 'nas', 'building', 'laptop', 'star', 'all'];
const COLORS = ['#2ee59d', '#22d3ee', '#60a5fa', '#a78bfa', '#f472b6', '#f5b942', '#fb923c', '#ff6b6b', '#94a3b8'];

function GroupEditor({ group, onClose }: { group: Group | null; onClose: () => void }) {
  const { toast, toastError, reloadDevices } = useApp();
  const [name, setName] = useState(group?.name ?? '');
  const [icon, setIcon] = useState<string>(group?.icon ?? 'desktop');
  const [color, setColor] = useState(group?.color ?? '#2ee59d');
  const [busy, setBusy] = useState(false);
  const [touched, setTouched] = useState(false);
  const err = !name.trim() ? 'Give the group a name.' : null;
  const save = async () => {
    setTouched(true);
    if (err) return;
    setBusy(true);
    try {
      const body = { name: name.trim(), icon, color };
      if (group) await api.updateGroup(group.id, body); else await api.createGroup(body);
      toast('ok', group ? 'Group saved.' : 'Group created.');
      await reloadDevices();
      onClose();
    } catch (e) { toastError(e, 'Save group'); setBusy(false); }
  };
  return (
    <Modal title={group ? 'Edit group' : 'New group'} onClose={onClose} locked={busy}
      footer={<><button className="btn" onClick={onClose} disabled={busy}>Cancel</button><button className="btn primary" onClick={save} disabled={busy}>{busy ? 'Saving…' : 'Save'}</button></>}>
      <div className="stack">
        <Field label="Name" error={touched ? err : null}><input data-autofocus value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') void save(); }} /></Field>
        <div className="field"><span>Icon</span>
          <div className="icon-pick" role="radiogroup" aria-label="Icon">{ICONS.map((i) => (
            <button key={i} type="button" role="radio" aria-checked={icon === i} aria-label={i} className={icon === i ? 'on' : ''} onClick={() => setIcon(i)}><Icon name={i} /></button>
          ))}</div>
        </div>
        <div className="field"><span>Color</span>
          <div className="swatches" role="radiogroup" aria-label="Color">
            {COLORS.map((c) => <button key={c} type="button" role="radio" aria-checked={color === c} aria-label={c} className={`swatch${color === c ? ' on' : ''}`} style={{ background: c }} onClick={() => setColor(c)} />)}
            <input type="color" value={/^#[0-9a-f]{6}$/i.test(color) ? color : '#2ee59d'} onChange={(e) => setColor(e.target.value)} aria-label="Custom color" />
          </div>
        </div>
      </div>
    </Modal>
  );
}

function Members({ group, onClose }: { group: Group; onClose: () => void }) {
  const { devices, locations, toast, toastError, reloadDevices } = useApp();
  const [ids, setIds] = useState<Set<string>>(new Set(devices.filter((d) => d.group_ids?.includes(group.id)).map((d) => d.id)));
  const [busy, setBusy] = useState(false);
  const [q, setQ] = useState('');
  const save = async () => {
    setBusy(true);
    try {
      const changes = devices.filter((d) => ids.has(d.id) !== !!d.group_ids?.includes(group.id));
      for (const d of changes) {
        const gids = ids.has(d.id) ? [...(d.group_ids ?? []), group.id] : (d.group_ids ?? []).filter((x) => x !== group.id);
        await api.updateDevice(d.id, deviceToInput(d, { group_ids: gids }));
      }
      toast('ok', `Membership of ${group.name} updated (${changes.length} change${changes.length === 1 ? '' : 's'}).`);
      await reloadDevices();
      onClose();
    } catch (e) { toastError(e, 'Update members'); await reloadDevices(); setBusy(false); }
  };
  const list = devices.filter((d) => !q || d.name.toLowerCase().includes(q.toLowerCase()));
  return (
    <Modal title={`Members of ${group.name}`} onClose={onClose} locked={busy}
      footer={<><button className="btn" onClick={onClose} disabled={busy}>Cancel</button><button className="btn primary" onClick={save} disabled={busy}>{busy ? 'Saving…' : 'Save members'}</button></>}>
      <div className="stack">
        <input type="search" placeholder="Filter devices" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Filter devices" data-autofocus />
        {list.length === 0 ? <p className="muted">No devices.</p> : (
          <ul className="affected">
            {list.map((d) => (
              <li key={d.id}>
                <label className="row gap-s grow" style={{ cursor: 'pointer' }}>
                  <input type="checkbox" checked={ids.has(d.id)} onChange={() => setIds((s) => { const n = new Set(s); if (n.has(d.id)) n.delete(d.id); else n.add(d.id); return n; })} />
                  <span className="grow">{d.name}</span><span className="muted small">{locations.find((l) => l.id === d.location_id)?.name}</span>
                </label>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Modal>
  );
}

export function GroupsPage() {
  const { groups, devices, isAdmin, canOperate, loaded, toast, toastError, reloadDevices } = useApp();
  const [edit, setEdit] = useState<Group | 'new' | null>(null);
  const [members, setMembers] = useState<Group | null>(null);
  const [act, setAct] = useState<{ g: Group; a: Action } | null>(null);
  const [del, setDel] = useState<Group | null>(null);
  const [busy, setBusy] = useState(false);

  const remove = async () => {
    if (!del) return;
    setBusy(true);
    try { await api.deleteGroup(del.id); toast('ok', `${del.name} was deleted. Its devices were not.`); await reloadDevices(); setDel(null); }
    catch (e) { toastError(e, 'Delete group'); }
    finally { setBusy(false); }
  };

  return (
    <>
      <PageHeader title="Groups" subtitle="Organise devices and run power actions on a whole group at once.">
        {isAdmin && <button className="btn primary" onClick={() => setEdit('new')}><Icon name="plus" size={16} />New group</button>}
      </PageHeader>
      {!loaded ? <Spinner /> : groups.length === 0 ? (
        <Empty icon="groups" title="No groups yet" action={isAdmin ? <button className="btn primary" onClick={() => setEdit('new')}>Create a group</button> : undefined}>Groups let you wake or shut down several devices with one click.</Empty>
      ) : (
        <div className="grid auto">
          {groups.map((g) => (
            <div key={g.id} className="group-card" style={{ ['--gc' as string]: g.color || 'var(--green)' }}>
              <div className="row">
                <div className="group-ico"><Icon name={g.icon} size={22} /></div>
                <div className="grow"><h3 style={{ fontSize: 16 }}>{g.name}</h3><div className="muted small">{g.device_count} devices · <b style={{ color: 'var(--green)' }}>{g.online_count}</b> online</div></div>
              </div>
              <div className="bar" aria-hidden="true"><span style={{ width: `${g.device_count ? (g.online_count / g.device_count) * 100 : 0}%`, background: g.color }} /></div>
              <div className="row wrap gap-xs">
                <button className="btn sm primary-soft" disabled={!canOperate || g.device_count === 0} onClick={() => setAct({ g, a: 'wake' })}><Icon name="power" size={14} />Wake</button>
                <button className="btn sm" disabled={!canOperate || g.device_count === 0} onClick={() => setAct({ g, a: 'shutdown' })}><Icon name="stop" size={14} />Shutdown</button>
                <button className="btn sm" disabled={!canOperate || g.device_count === 0} onClick={() => setAct({ g, a: 'restart' })}><Icon name="restart" size={14} />Restart</button>
              </div>
              {isAdmin && (
                <div className="row wrap gap-xs">
                  <button className="btn sm ghost" onClick={() => setMembers(g)}><Icon name="devices" size={14} />Members</button>
                  <button className="btn sm ghost" onClick={() => setEdit(g)}><Icon name="edit" size={14} />Edit</button>
                  <button className="btn sm ghost" onClick={() => setDel(g)}><Icon name="trash" size={14} />Delete</button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {edit && <GroupEditor group={edit === 'new' ? null : edit} onClose={() => setEdit(null)} />}
      {members && <Members group={members} onClose={() => setMembers(null)} />}
      {act && <BulkDialog action={act.a} groupId={act.g.id} devices={devices.filter((d) => d.group_ids?.includes(act.g.id))} scope={`group “${act.g.name}”`} onClose={() => setAct(null)} />}
      {del && (
        <ConfirmDialog title={`Delete group ${del.name}?`} confirmLabel="Delete group" danger busy={busy} onConfirm={remove} onCancel={() => setDel(null)}>
          <p>Only the group is removed. The {del.device_count} device(s) in it stay in the inventory.</p>
        </ConfirmDialog>
      )}
    </>
  );
}
