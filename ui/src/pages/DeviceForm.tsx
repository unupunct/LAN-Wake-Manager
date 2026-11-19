import { useEffect, useState, type FormEvent } from 'react';
import { Link, useLocation, useNavigate, useParams } from 'react-router-dom';
import { api } from '../api/client';
import type { DeviceInput, DeviceType, OsKind, PowerMethod } from '../api/types';
import { Field, PageHeader, Panel, Spinner } from '../components/ui';
import { normalizeMac, parsePorts } from '../lib/format';
import { isValidIPv4, isValidMac, isValidPort } from '../lib/validate';
import { useApp } from '../store';

export interface DevicePrefill { name?: string; hostname?: string | null; ip?: string | null; mac?: string | null; os?: OsKind | null; device_type?: DeviceType | null; location_id?: string; tcp_check_ports?: number[] }

export function DeviceForm() {
  const { id } = useParams();
  const editing = !!id;
  const nav = useNavigate();
  const routeState = (useLocation().state ?? {}) as { prefill?: DevicePrefill };
  const { devices, locations, groups, locationId, isAdmin, toast, toastError, reloadDevices, loaded } = useApp();
  const existing = devices.find((d) => d.id === id);
  const pre = routeState.prefill;

  const [name, setName] = useState('');
  const [loc, setLoc] = useState('');
  const [hostname, setHostname] = useState('');
  const [ip, setIp] = useState('');
  const [mac, setMac] = useState('');
  const [os, setOs] = useState<OsKind | ''>('');
  const [type, setType] = useState<DeviceType>('pc');
  const [gids, setGids] = useState<string[]>([]);
  const [ports, setPorts] = useState('22, 3389, 445');
  const [method, setMethod] = useState<PowerMethod>('none');
  const [sshUser, setSshUser] = useState('');
  const [sshPort, setSshPort] = useState('22');
  const [sshKey, setSshKey] = useState('');
  const [notes, setNotes] = useState('');
  const [busy, setBusy] = useState(false);
  const [touched, setTouched] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (!loaded || ready) return;
    if (editing && !existing) return;
    if (existing) {
      setName(existing.name); setLoc(existing.location_id); setHostname(existing.hostname ?? ''); setIp(existing.ip ?? ''); setMac(existing.mac);
      setOs(existing.os ?? ''); setType(existing.device_type); setGids(existing.group_ids ?? []); setPorts((existing.tcp_check_ports ?? []).join(', '));
      setMethod(existing.power_method); setSshUser(existing.ssh?.user ?? ''); setSshPort(String(existing.ssh?.port ?? 22)); setSshKey(existing.ssh?.key_id ?? ''); setNotes(existing.notes ?? '');
    } else {
      setLoc(pre?.location_id ?? (locationId || locations[0]?.id || ''));
      setName(pre?.name ?? pre?.hostname ?? ''); setHostname(pre?.hostname ?? ''); setIp(pre?.ip ?? ''); setMac(pre?.mac ? normalizeMac(pre.mac) : '');
      setOs(pre?.os ?? ''); setType(pre?.device_type ?? 'pc');
      if (pre?.tcp_check_ports?.length) setPorts(pre.tcp_check_ports.join(', '));
    }
    setReady(true);
  }, [loaded, ready, editing, existing, pre, locationId, locations]);

  if (!isAdmin) return <Panel><p>Only administrators can add or edit devices.</p></Panel>;
  if (!loaded || (editing && !existing && !ready)) {
    return loaded ? <Panel><p>Device not found. <Link to="/devices">Back to devices</Link></p></Panel> : <Spinner />;
  }

  const parsed = parsePorts(ports);
  const errs = {
    name: !name.trim() ? 'Give the device a name.' : null,
    loc: !loc ? 'Choose a location.' : null,
    mac: !mac.trim() ? 'A MAC address is required for Wake-on-LAN.' : !isValidMac(mac) ? 'Use a MAC like AA:BB:CC:DD:EE:FF.' : null,
    ip: ip.trim() && !isValidIPv4(ip) ? 'Enter a valid IPv4 address like 192.168.1.20.' : null,
    ports: parsed.invalid.length ? `Invalid port(s): ${parsed.invalid.join(', ')}. Use numbers 1–65535.` : null,
    sshPort: method === 'ssh' && !isValidPort(Number(sshPort)) ? 'Port must be 1–65535.' : null,
    sshUser: method === 'ssh' && !sshUser.trim() ? 'SSH user name is required.' : null,
  };
  const invalid = Object.values(errs).some(Boolean);
  const show = (k: keyof typeof errs) => (touched ? errs[k] : null);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (invalid) return;
    const body: DeviceInput = {
      location_id: loc, name: name.trim(), hostname: hostname.trim() || null, ip: ip.trim() || null, mac: normalizeMac(mac), os: os || null,
      device_type: type, group_ids: gids, tcp_check_ports: parsed.ports, power_method: method,
      ssh: method === 'ssh' ? { user: sshUser.trim(), port: Number(sshPort), key_id: sshKey.trim() || null } : null, notes: notes.trim() || null,
    };
    setBusy(true);
    try {
      const d = editing ? await api.updateDevice(id!, body) : await api.createDevice(body);
      toast('ok', editing ? 'Device saved.' : `${d.name} was added.`);
      await reloadDevices();
      nav(`/devices/${d.id}`);
    } catch (err) { toastError(err, 'Save device'); }
    finally { setBusy(false); }
  };

  const toggleGroup = (g: string) => setGids((l) => (l.includes(g) ? l.filter((x) => x !== g) : [...l, g]));

  return (
    <>
      <PageHeader title={editing ? `Edit ${existing?.name ?? 'device'}` : 'Add device'} subtitle="A device needs a name, a location and the MAC address of its network adapter." />
      <form className="stack" onSubmit={submit} noValidate>
        <Panel title="Identity">
          <div className="form-grid">
            <Field label="Name *" error={show('name')}><input value={name} onChange={(e) => setName(e.target.value)} placeholder="Gaming PC" autoFocus /></Field>
            <Field label="Location *" error={show('loc')}>
              <select value={loc} onChange={(e) => setLoc(e.target.value)}><option value="">Select…</option>{locations.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}</select>
            </Field>
            <Field label="MAC address *" error={show('mac')} hint="Needed to send the Wake-on-LAN packet."><input value={mac} onChange={(e) => setMac(e.target.value)} placeholder="AA:BB:CC:DD:EE:FF" className="mono" spellCheck={false} /></Field>
            <Field label="IPv4 address" error={show('ip')} hint="Used to check whether the device is online."><input value={ip} onChange={(e) => setIp(e.target.value)} placeholder="192.168.1.20" inputMode="decimal" /></Field>
            <Field label="Hostname"><input value={hostname} onChange={(e) => setHostname(e.target.value)} placeholder="DESKTOP-1" /></Field>
            <Field label="Operating system">
              <select value={os} onChange={(e) => setOs(e.target.value as OsKind | '')}><option value="">Unknown</option><option value="windows">Windows</option><option value="linux">Linux</option><option value="macos">macOS</option><option value="other">Other</option></select>
            </Field>
            <Field label="Device type">
              <select value={type} onChange={(e) => setType(e.target.value as DeviceType)}><option value="pc">PC</option><option value="laptop">Laptop</option><option value="server">Server</option><option value="nas">NAS</option><option value="other">Other</option></select>
            </Field>
            <Field label="Reachability TCP ports" error={show('ports')} hint="Open ports used to detect that the device is up (comma separated)."><input value={ports} onChange={(e) => setPorts(e.target.value)} placeholder="22, 3389, 445" /></Field>
          </div>
        </Panel>
        <Panel title="Groups">
          {groups.length === 0 ? <p className="muted">No groups yet. Create groups on the Groups page.</p> : (
            <div className="checks">{groups.map((g) => (
              <label key={g.id} className="chip-check"><input type="checkbox" checked={gids.includes(g.id)} onChange={() => toggleGroup(g.id)} />{g.name}</label>
            ))}</div>
          )}
        </Panel>
        <Panel title="Power control (shutdown / restart)">
          <div className="form-grid">
            <Field label="Method" hint="Wake-on-LAN always works through the magic packet. This sets how shutdown and restart are performed." className="full">
              <select value={method} onChange={(e) => setMethod(e.target.value as PowerMethod)}>
                <option value="none">None (wake only)</option><option value="agent">Controller agent (recommended)</option><option value="ssh">SSH</option><option value="winrm">WinRM</option>
              </select>
            </Field>
            {method === 'ssh' && (
              <>
                <Field label="SSH user *" error={show('sshUser')}><input value={sshUser} onChange={(e) => setSshUser(e.target.value)} autoComplete="off" /></Field>
                <Field label="SSH port" error={show('sshPort')}><input value={sshPort} onChange={(e) => setSshPort(e.target.value)} inputMode="numeric" /></Field>
                <Field label="Key ID (optional)" hint="Identifier of a key known to the controller. No private keys are entered here." className="full"><input value={sshKey} onChange={(e) => setSshKey(e.target.value)} autoComplete="off" /></Field>
              </>
            )}
            {method === 'agent' && <div className="full muted small">After saving, open the device and use “Agent &amp; power” to enroll the agent with a one-time code.</div>}
          </div>
        </Panel>
        <Panel title="Notes"><textarea value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Optional notes" /></Panel>
        <div className="row gap-s">
          <button className="btn primary" disabled={busy}>{busy ? 'Saving…' : editing ? 'Save changes' : 'Add device'}</button>
          <Link className="btn" to={editing ? `/devices/${id}` : '/devices'}>Cancel</Link>
          {touched && invalid && <span className="err-text">Please fix the highlighted fields.</span>}
        </div>
      </form>
    </>
  );
}
