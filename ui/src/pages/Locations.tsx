import { useState } from 'react';
import { api } from '../api/client';
import type { Location, LocationInput, RelayProbe } from '../api/types';
import { Icon } from '../components/Icon';
import { ConfirmDialog, Empty, Field, Modal, PageHeader, Panel, Pill, Spinner } from '../components/ui';
import { formatTime, parsePorts, relativeTime } from '../lib/format';
import { isValidHexColor, isValidIPv4 } from '../lib/validate';
import { useApp } from '../store';

const blank: LocationInput = { name: '', color: '#2ee59d', notes: '', broadcast_addr: null, wol_ports: [9, 7], wake_attempts: 3, wake_interval_secs: 2, wake_timeout_secs: 120 };

function LocationEditor({ loc, onClose }: { loc: Location | null; onClose: () => void }) {
  const { toast, toastError, reload } = useApp();
  const [f, setF] = useState<LocationInput>(loc ? { name: loc.name, color: loc.color ?? '#2ee59d', notes: loc.notes ?? '', broadcast_addr: loc.broadcast_addr ?? null, wol_ports: loc.wol_ports ?? [9, 7], wake_attempts: loc.wake_attempts, wake_interval_secs: loc.wake_interval_secs, wake_timeout_secs: loc.wake_timeout_secs } : blank);
  const [portsText, setPortsText] = useState((f.wol_ports ?? []).join(', '));
  const [busy, setBusy] = useState(false);
  const [touched, setTouched] = useState(false);
  const set = <K extends keyof LocationInput>(k: K, v: LocationInput[K]) => setF((o) => ({ ...o, [k]: v }));
  const pp = parsePorts(portsText);
  const errs = {
    name: !f.name.trim() ? 'Name is required.' : null,
    bc: f.broadcast_addr && !isValidIPv4(f.broadcast_addr) ? 'Enter an IPv4 broadcast address like 192.168.1.255.' : null,
    ports: pp.invalid.length || pp.ports.length === 0 ? 'Enter at least one valid port (1–65535), e.g. 9, 7.' : null,
    att: !(f.wake_attempts >= 1 && f.wake_attempts <= 20) ? '1–20 attempts.' : null,
    int: !(f.wake_interval_secs >= 1 && f.wake_interval_secs <= 60) ? '1–60 seconds.' : null,
    to: !(f.wake_timeout_secs >= 10 && f.wake_timeout_secs <= 900) ? '10–900 seconds.' : null,
    color: f.color && !isValidHexColor(f.color) ? 'Use a #rrggbb color.' : null,
  };
  const invalid = Object.values(errs).some(Boolean);
  const show = (k: keyof typeof errs) => (touched ? errs[k] : null);
  const save = async () => {
    setTouched(true);
    if (invalid) return;
    setBusy(true);
    try {
      const body: LocationInput = { ...f, name: f.name.trim(), wol_ports: pp.ports, broadcast_addr: f.broadcast_addr?.trim() || null, notes: f.notes ?? '' };
      if (loc) await api.updateLocation(loc.id, body); else await api.createLocation(body);
      toast('ok', loc ? 'Location saved.' : 'Location created.');
      await reload();
      onClose();
    } catch (e) { toastError(e, 'Save location'); setBusy(false); }
  };
  return (
    <Modal wide title={loc ? `Edit ${loc.name}` : 'New location'} onClose={onClose} locked={busy}
      footer={<><button className="btn" onClick={onClose} disabled={busy}>Cancel</button><button className="btn primary" onClick={save} disabled={busy}>{busy ? 'Saving…' : 'Save'}</button></>}>
      <div className="form-grid">
        <Field label="Name *" error={show('name')}><input data-autofocus value={f.name} onChange={(e) => set('name', e.target.value)} placeholder="Office" /></Field>
        <Field label="Color" error={show('color')}><div className="row gap-s"><input type="color" value={isValidHexColor(f.color ?? '') ? f.color! : '#2ee59d'} onChange={(e) => set('color', e.target.value)} aria-label="Pick color" /><input value={f.color ?? ''} onChange={(e) => set('color', e.target.value)} className="mono" /></div></Field>
        <fieldset className="full"><legend>Wake-on-LAN</legend>
          <div className="form-grid">
            <Field label="UDP ports" error={show('ports')} hint="Usually 9 and 7."><input value={portsText} onChange={(e) => setPortsText(e.target.value)} className="mono" /></Field>
            <Field label="Broadcast address override" error={show('bc')} hint="Leave empty to derive it from the interface."><input value={f.broadcast_addr ?? ''} onChange={(e) => set('broadcast_addr', e.target.value || null)} placeholder="192.168.1.255" className="mono" /></Field>
            <Field label="Packet attempts" error={show('att')}><input type="number" min={1} max={20} value={f.wake_attempts} onChange={(e) => set('wake_attempts', Number(e.target.value))} /></Field>
            <Field label="Interval between attempts (s)" error={show('int')}><input type="number" min={1} max={60} value={f.wake_interval_secs} onChange={(e) => set('wake_interval_secs', Number(e.target.value))} /></Field>
            <Field label="Wait for device to come online (s)" error={show('to')} hint="After this, the wake is reported as “packet sent, not online”."><input type="number" min={10} max={900} value={f.wake_timeout_secs} onChange={(e) => set('wake_timeout_secs', Number(e.target.value))} /></Field>
          </div>
        </fieldset>
        <Field label="Notes" className="full"><textarea value={f.notes ?? ''} onChange={(e) => set('notes', e.target.value)} /></Field>
      </div>
    </Modal>
  );
}

function RelayDialog({ loc, onClose }: { loc: Location; onClose: () => void }) {
  const { toast, toastError, reload } = useApp();
  const [url, setUrl] = useState(loc.relay_url ?? 'https://');
  const [probe, setProbe] = useState<RelayProbe | null>(null);
  const [code, setCode] = useState('');
  const [trust, setTrust] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirmUnpair, setConfirmUnpair] = useState(false);
  const remote = loc.relay_mode === 'remote';
  const urlOk = /^https:\/\/[^\s/]+(:\d+)?\/?$/i.test(url.trim());

  const doProbe = async () => {
    setBusy(true); setProbe(null); setTrust(false);
    try { setProbe(await api.relayProbe(loc.id, url.trim().replace(/\/$/, ''))); }
    catch (e) { toastError(e, 'Probe relay'); }
    finally { setBusy(false); }
  };
  const doPair = async () => {
    if (!probe) return;
    setBusy(true);
    try { await api.relayPair(loc.id, url.trim().replace(/\/$/, ''), code.trim(), probe.fingerprint); toast('ok', `Relay paired with ${loc.name}.`); await reload(); onClose(); }
    catch (e) { toastError(e, 'Pair relay'); setBusy(false); }
  };
  const doUnpair = async () => {
    setBusy(true);
    try { await api.relayUnpair(loc.id); toast('ok', 'Relay removed. This location uses the controller’s own network again.'); await reload(); onClose(); }
    catch (e) { toastError(e, 'Unpair relay'); setBusy(false); }
  };

  return (
    <Modal wide title={`Relay for ${loc.name}`} onClose={onClose} locked={busy}>
      <div className="stack">
        <p className="muted">A relay is a small helper running on a computer in another network (e.g. your office). The controller sends Wake-on-LAN packets and probes through it, so remote locations work over VPN.</p>
        {remote ? (
          <>
            <dl className="kv">
              <dt>Status</dt><dd><Pill tone={loc.relay_status === 'online' ? 'green' : 'red'}>{loc.relay_status}</Pill></dd>
              <dt>Name</dt><dd>{loc.relay_name || <span className="na">not reported</span>}</dd>
              <dt>Address</dt><dd className="mono">{loc.relay_url}</dd>
              <dt>Version</dt><dd>{loc.relay_version || <span className="na">not reported</span>}</dd>
              <dt>Last seen</dt><dd>{loc.relay_last_seen ? `${formatTime(loc.relay_last_seen)} (${relativeTime(loc.relay_last_seen)})` : 'never'}</dd>
            </dl>
            <div><button className="btn danger" onClick={() => setConfirmUnpair(true)} disabled={busy}>Unpair relay</button></div>
          </>
        ) : (
          <>
            <div className="notice"><Icon name="info" size={16} /><span>This location currently uses the <strong>built-in relay</strong> (the controller’s own network).</span></div>
            <ol className="steps-ol">
              <li>Start the relay on the remote network and note its address and pairing code (shown on the relay console).</li>
              <li>Enter the relay address and press <strong>Check relay</strong>.</li>
              <li>Compare the fingerprint below with the one on the relay console. Only continue if they are identical.</li>
              <li>Enter the pairing code and pair.</li>
            </ol>
            <Field label="Relay address" error={url.length > 8 && !urlOk ? 'Use https://host:port, e.g. https://100.64.1.9:8444' : null}>
              <div className="row gap-s"><input value={url} onChange={(e) => { setUrl(e.target.value); setProbe(null); }} className="mono" placeholder="https://100.64.1.9:8444" data-autofocus /><button className="btn accent" onClick={doProbe} disabled={!urlOk || busy}>{busy && !probe ? 'Checking…' : 'Check relay'}</button></div>
            </Field>
            {probe && (
              <div className="stack">
                <div className="panel">
                  <dl className="kv"><dt>Relay name</dt><dd>{probe.name}</dd><dt>Version</dt><dd>{probe.version}</dd><dt>Relay ID</dt><dd className="mono">{probe.relay_id}</dd><dt>Fingerprint (SHA-256)</dt><dd className="mono" style={{ wordBreak: 'break-all' }}>{probe.fingerprint}</dd></dl>
                </div>
                <label className="chip-check" style={{ alignSelf: 'flex-start' }}><input type="checkbox" checked={trust} onChange={(e) => setTrust(e.target.checked)} />This fingerprint matches the one shown on the relay</label>
                <Field label="Pairing code"><input value={code} onChange={(e) => setCode(e.target.value)} className="mono" autoComplete="off" placeholder="123456" /></Field>
                <div><button className="btn primary" onClick={doPair} disabled={!trust || !code.trim() || busy}>{busy ? 'Pairing…' : 'Pair relay'}</button></div>
              </div>
            )}
          </>
        )}
      </div>
      {confirmUnpair && (
        <ConfirmDialog title="Unpair relay?" confirmLabel="Unpair" danger busy={busy} onConfirm={doUnpair} onCancel={() => setConfirmUnpair(false)}>
          <p>Wake-on-LAN for {loc.name} will go back to being sent from the controller’s own network, which usually cannot reach a remote LAN. Scheduled wakes that run on the relay will stop.</p>
        </ConfirmDialog>
      )}
    </Modal>
  );
}

export function LocationsPage() {
  const { locations, isAdmin, loaded, toast, toastError, reload } = useApp();
  const [edit, setEdit] = useState<Location | 'new' | null>(null);
  const [relay, setRelay] = useState<Location | null>(null);
  const [del, setDel] = useState<Location | null>(null);
  const [busy, setBusy] = useState(false);

  const remove = async () => {
    if (!del) return;
    setBusy(true);
    try { await api.deleteLocation(del.id); toast('ok', `${del.name} was deleted.`); await reload(); setDel(null); }
    catch (e) { toastError(e, 'Delete location'); }
    finally { setBusy(false); }
  };

  return (
    <>
      <PageHeader title="Locations" subtitle="Each network location has its own devices, Wake-on-LAN settings and relay.">
        {isAdmin && <button className="btn primary" onClick={() => setEdit('new')}><Icon name="plus" size={16} />Add location</button>}
      </PageHeader>
      {!loaded ? <Spinner /> : locations.length === 0 ? (
        <Empty icon="location" title="No locations" action={isAdmin ? <button className="btn primary" onClick={() => setEdit('new')}>Add location</button> : undefined}>A location is a network such as “Home” or “Office”.</Empty>
      ) : (
        <div className="grid auto">
          {locations.map((l) => (
            <Panel key={l.id} className="loc-panel">
              <div className="stack">
                <div className="row"><span className="swatch on" style={{ background: l.color ?? 'var(--cyan)', cursor: 'default', width: 14, height: 14 }} aria-hidden="true" /><h3 className="grow" style={{ fontSize: 16 }}>{l.name}</h3><Pill tone={l.controller_status === 'online' ? 'green' : 'amber'}>{l.controller_status}</Pill></div>
                {l.notes && <p className="muted">{l.notes}</p>}
                <dl className="kv">
                  <dt>Devices</dt><dd>{l.device_count} ({l.online_count} online)</dd>
                  <dt>Relay</dt><dd>{l.relay_mode === 'remote' ? <>{l.relay_name ?? 'remote'} <Pill tone={l.relay_status === 'online' ? 'green' : 'red'}>{l.relay_status}</Pill></> : <>Built-in <Pill tone="cyan">controller network</Pill></>}</dd>
                  <dt>WoL ports</dt><dd className="mono">{(l.wol_ports ?? []).join(', ')}</dd>
                  <dt>Broadcast</dt><dd className="mono">{l.broadcast_addr || <span className="na">automatic</span>}</dd>
                  <dt>Attempts</dt><dd>{l.wake_attempts} × every {l.wake_interval_secs} s, wait up to {l.wake_timeout_secs} s</dd>
                </dl>
                {isAdmin && (
                  <div className="row wrap gap-xs">
                    <button className="btn sm" onClick={() => setEdit(l)}><Icon name="edit" size={14} />Edit</button>
                    <button className="btn sm accent" onClick={() => setRelay(l)}><Icon name="wifi" size={14} />Relay</button>
                    <button className="btn sm ghost" onClick={() => setDel(l)}><Icon name="trash" size={14} />Delete</button>
                  </div>
                )}
              </div>
            </Panel>
          ))}
        </div>
      )}
      {edit && <LocationEditor loc={edit === 'new' ? null : edit} onClose={() => setEdit(null)} />}
      {relay && <RelayDialog loc={locations.find((l) => l.id === relay.id) ?? relay} onClose={() => setRelay(null)} />}
      {del && (
        <ConfirmDialog title={`Delete ${del.name}?`} confirmLabel="Delete location" danger busy={busy} onConfirm={remove} onCancel={() => setDel(null)} typeToConfirm={del.device_count > 0 ? 'delete' : undefined}>
          <p>{del.device_count > 0 ? `This location still contains ${del.device_count} device(s). The controller may refuse to delete it, or remove them with it.` : 'This location is empty and will be removed.'}</p>
        </ConfirmDialog>
      )}
    </>
  );
}
