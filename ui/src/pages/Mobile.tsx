import { useEffect, useRef, useState } from 'react';
import QRCode from 'qrcode';
import { api } from '../api/client';
import type { MobileDevice, Pairing, Role } from '../api/types';
import { Icon } from '../components/Icon';
import { ConfirmDialog, CopyButton, Empty, ErrorBox, Field, Modal, PageHeader, Panel, Pill, Spinner, useLoad } from '../components/ui';
import { countdown, relativeTime } from '../lib/format';
import { useApp } from '../store';

const ROLES: { id: Role; title: string; text: string }[] = [
  { id: 'viewer', title: 'Viewer', text: 'Can see devices and history. Cannot change anything.' },
  { id: 'operator', title: 'Operator', text: 'Can wake, shut down and restart devices, run schedules and discovery.' },
  { id: 'admin', title: 'Admin', text: 'Full control, including settings, backups and pairing more phones.' },
];

function QR({ payload }: { payload: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!ref.current) return;
    QRCode.toCanvas(ref.current, payload, { width: 300, margin: 1, errorCorrectionLevel: 'M', color: { dark: '#000000', light: '#ffffff' } }).catch((e: unknown) => setErr(String(e)));
  }, [payload]);
  if (err) return <ErrorBox message={`Could not draw the QR code: ${err}`} />;
  return <div className="qr-box"><canvas ref={ref} role="img" aria-label="Pairing QR code" /></div>;
}

function PairDialog({ onClose, onPaired }: { onClose: () => void; onPaired: () => void }) {
  const { toastError } = useApp();
  const [role, setRole] = useState<Role>('operator');
  const [pairing, setPairing] = useState<Pairing | null>(null);
  const [busy, setBusy] = useState(false);
  const [now, setNow] = useState(Date.now() / 1000);

  useEffect(() => {
    if (!pairing) return;
    const t = window.setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => window.clearInterval(t);
  }, [pairing]);

  // While a code is shown, poll the list so the dialog can notice the new phone.
  useEffect(() => {
    if (!pairing || now >= pairing.expires_at) return;
    const t = window.setInterval(onPaired, 4000);
    return () => window.clearInterval(t);
  }, [pairing, now, onPaired]);

  const create = async () => {
    setBusy(true);
    try { const p = await api.createPairing(role, 300); setPairing(p); setNow(Date.now() / 1000); }
    catch (e) { toastError(e, 'Create pairing code'); }
    finally { setBusy(false); }
  };
  const expired = pairing ? now >= pairing.expires_at : false;

  return (
    <Modal wide title="Add mobile device" onClose={onClose}
      footer={<>{pairing && <button className="btn" onClick={() => setPairing(null)}>Change role</button>}<button className="btn primary" onClick={onClose}>Done</button></>}>
      {!pairing ? (
        <div className="stack">
          <p className="muted">Choose what the phone is allowed to do. You can change or revoke this later.</p>
          <div className="stack-s" role="radiogroup" aria-label="Role">
            {ROLES.map((r) => (
              <label key={r.id} className="panel" style={{ cursor: 'pointer', borderColor: role === r.id ? 'var(--green)' : undefined }}>
                <div className="row gap-s"><input type="radio" name="role" checked={role === r.id} onChange={() => setRole(r.id)} /><div><strong>{r.title}</strong><div className="muted small">{r.text}</div></div></div>
              </label>
            ))}
          </div>
          <div><button className="btn primary" onClick={create} disabled={busy}><Icon name="mobile" size={16} />{busy ? 'Creating…' : 'Show pairing QR code'}</button></div>
        </div>
      ) : (
        <div className="stack">
          {expired ? (
            <div className="stack" style={{ alignItems: 'center', padding: 24 }}>
              <Icon name="warn" size={30} /><strong>This code has expired.</strong><p className="muted">Pairing codes work once and only for 5 minutes.</p>
              <button className="btn primary" onClick={create} disabled={busy}>{busy ? 'Creating…' : 'Create a new code'}</button>
            </div>
          ) : (
            <>
              <QR payload={pairing.qr_payload} />
              <div className="row between wrap">
                <span>Role: <Pill tone="cyan">{role}</Pill></span>
                <span>Expires in <strong className="mono" style={{ color: pairing.expires_at - now < 60 ? 'var(--amber)' : 'var(--green)' }}>{countdown(pairing.expires_at, now)}</strong></span>
              </div>
              <ol className="steps-ol">
                <li>Install the LAN Wake Manager app on the phone and open it.</li>
                <li>Choose “Add controller” and scan this QR code.</li>
                <li>The phone must be able to reach one of the addresses below (same Wi-Fi, or connected via Tailscale / VPN).</li>
              </ol>
              <div>
                <h3 style={{ marginBottom: 6 }}>Addresses encoded in the code</h3>
                {(pairing.addresses ?? []).length === 0 ? <p className="muted">No addresses were reported.</p> : (
                  <ul className="affected">{pairing.addresses.map((a) => <li key={a}><span className="mono grow">{a}</span><CopyButton text={a} /></li>)}</ul>
                )}
              </div>
              <div className="notice warn"><Icon name="warn" size={16} /><span>Anyone who scans this code in the next few minutes can pair a phone with the <strong>{role}</strong> role. Do not share a screenshot of it.</span></div>
            </>
          )}
        </div>
      )}
    </Modal>
  );
}

function EditDevice({ d, onClose, onSaved }: { d: MobileDevice; onClose: () => void; onSaved: () => void }) {
  const { toast, toastError } = useApp();
  const [name, setName] = useState(d.name);
  const [role, setRole] = useState<Role>(d.role);
  const [busy, setBusy] = useState(false);
  const save = async () => {
    if (!name.trim()) return;
    setBusy(true);
    try { await api.updateMobile(d.id, { name: name.trim(), role }); toast('ok', 'Device updated.'); onSaved(); }
    catch (e) { toastError(e, 'Update device'); setBusy(false); }
  };
  return (
    <Modal title={`Edit ${d.name}`} onClose={onClose} locked={busy}
      footer={<><button className="btn" onClick={onClose} disabled={busy}>Cancel</button><button className="btn primary" onClick={save} disabled={busy || !name.trim()}>Save</button></>}>
      <div className="stack">
        <Field label="Name"><input data-autofocus value={name} onChange={(e) => setName(e.target.value)} /></Field>
        <Field label="Role"><select value={role} onChange={(e) => setRole(e.target.value as Role)}>{ROLES.map((r) => <option key={r.id} value={r.id}>{r.title}</option>)}</select></Field>
      </div>
    </Modal>
  );
}

export function MobilePage() {
  const { isAdmin, toast, toastError } = useApp();
  const { data, error, loading, reload } = useLoad(() => api.mobileDevices(), []);
  const [pair, setPair] = useState(false);
  const [edit, setEdit] = useState<MobileDevice | null>(null);
  const [revoke, setRevoke] = useState<MobileDevice | null>(null);
  const [busy, setBusy] = useState(false);

  if (!isAdmin) return <Panel><Empty icon="mobile" title="Administrators only">Mobile access can only be managed with an administrator account.</Empty></Panel>;

  const doRevoke = async () => {
    if (!revoke) return;
    setBusy(true);
    try { await api.revokeMobile(revoke.id); toast('ok', `${revoke.name} was revoked. Its access stops immediately.`); setRevoke(null); await reload(true); }
    catch (e) { toastError(e, 'Revoke'); }
    finally { setBusy(false); }
  };
  const list = data ?? [];
  return (
    <>
      <PageHeader title="Mobile access" subtitle="Pair phones with a one-time QR code. The code contains no permanent password; each phone gets its own revocable token.">
        <button className="btn primary" onClick={() => setPair(true)}><Icon name="plus" size={16} />Add mobile device</button>
      </PageHeader>
      <Panel className="flush">
        {loading && !data ? <Spinner /> : error && !data ? <ErrorBox message={error} onRetry={() => void reload()} /> : list.length === 0 ? (
          <Empty icon="mobile" title="No phones paired yet" action={<button className="btn primary" onClick={() => setPair(true)}>Add mobile device</button>}>Pair an Android phone to wake and manage your PCs from anywhere.</Empty>
        ) : (
          <div className="table-wrap"><table className="tbl stackm">
            <thead><tr><th>Device</th><th>Role</th><th>Last seen</th><th>Last address</th><th>Paired</th><th aria-label="Actions" /></tr></thead>
            <tbody>{list.map((m) => (
              <tr key={m.id} style={m.revoked ? { opacity: 0.55 } : undefined}>
                <td><strong>{m.name}</strong><div className="muted small">{m.platform}</div></td>
                <td>{m.revoked ? <Pill tone="red">revoked</Pill> : <Pill tone={m.role === 'admin' ? 'amber' : m.role === 'operator' ? 'cyan' : 'muted'}>{m.role}</Pill>}</td>
                <td className="nowrap">{m.last_seen ? relativeTime(m.last_seen) : 'never'}</td>
                <td className="mono">{m.last_ip || '—'}</td>
                <td className="nowrap muted">{relativeTime(m.created_at)}</td>
                <td className="actions">{!m.revoked && <><button className="btn sm" onClick={() => setEdit(m)}><Icon name="edit" size={13} />Edit</button>{' '}<button className="btn sm danger" onClick={() => setRevoke(m)}>Revoke</button></>}</td>
              </tr>
            ))}</tbody>
          </table></div>
        )}
      </Panel>
      {pair && <PairDialog onClose={() => setPair(false)} onPaired={() => void reload(true)} />}
      {edit && <EditDevice d={edit} onClose={() => setEdit(null)} onSaved={() => { setEdit(null); void reload(true); }} />}
      {revoke && <ConfirmDialog title={`Revoke ${revoke.name}?`} confirmLabel="Revoke access" danger busy={busy} onConfirm={doRevoke} onCancel={() => setRevoke(null)}><p>The phone stops working immediately and has to be paired again with a new QR code.</p></ConfirmDialog>}
    </>
  );
}
