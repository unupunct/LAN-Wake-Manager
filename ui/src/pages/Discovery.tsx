import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import type { Candidate, DiscoveryJob, NetInterface } from '../api/types';
import { Icon } from '../components/Icon';
import { Empty, ErrorBox, Field, PageHeader, Panel, Pill, Spinner } from '../components/ui';
import { parsePorts } from '../lib/format';
import { isValidCidr } from '../lib/validate';
import { useApp } from '../store';

const METHODS = [
  { id: 'arp', label: 'ARP', hint: 'Finds MAC addresses (same subnet)' },
  { id: 'ping', label: 'Ping', hint: 'ICMP echo' },
  { id: 'tcp', label: 'TCP ports', hint: 'Probes common ports' },
  { id: 'mdns', label: 'mDNS', hint: 'Names of Apple / Linux devices' },
];

export function Discovery() {
  const { locations, locationId, canOperate, toast, toastError, reloadDevices } = useApp();
  const nav = useNavigate();
  const [loc, setLoc] = useState(locationId || locations[0]?.id || '');
  const [ifaces, setIfaces] = useState<NetInterface[] | null>(null);
  const [ifaceErr, setIfaceErr] = useState<string | null>(null);
  const [iface, setIface] = useState('');
  const [subnet, setSubnet] = useState('');
  const [speed, setSpeed] = useState<'slow' | 'normal' | 'fast'>('normal');
  const [timeout, setTimeoutMs] = useState('800');
  const [ports, setPorts] = useState('22, 445, 3389, 80, 443');
  const [methods, setMethods] = useState<string[]>(['arp', 'ping', 'tcp']);
  const [job, setJob] = useState<DiscoveryJob | null>(null);
  const [starting, setStarting] = useState(false);
  const jobId = useRef<string | null>(null);

  useEffect(() => { if (!loc && locations[0]) setLoc(locationId || locations[0].id); }, [locations, loc, locationId]);

  useEffect(() => {
    if (!loc) return;
    let live = true;
    setIfaces(null); setIfaceErr(null); setIface(''); setSubnet('');
    api.interfaces(loc).then((l) => {
      if (!live) return;
      setIfaces(l ?? []);
      if (l && l[0]) { setIface(l[0].name); setSubnet(l[0].subnet_cidr); }
    }).catch((e) => { if (live) { setIfaces([]); setIfaceErr(e instanceof Error ? e.message : 'Could not read interfaces'); } });
    return () => { live = false; };
  }, [loc]);

  // poll job
  useEffect(() => {
    if (!job || job.state !== 'running') return;
    const t = window.setInterval(async () => {
      try {
        const j = await api.discovery(job.job_id);
        setJob(j);
        if (j.state !== 'running') { void reloadDevices(); }
      } catch (e) { toastError(e, 'Discovery'); window.clearInterval(t); }
    }, 1000);
    return () => window.clearInterval(t);
  }, [job, toastError, reloadDevices]);

  const parsedPorts = parsePorts(ports);
  const subnetErr = subnet && !isValidCidr(subnet) ? 'Use CIDR notation like 192.168.1.0/24.' : subnet && Number(subnet.split('/')[1]) < 17 ? 'Subnets larger than /17 are refused. Pick a smaller one.' : null;
  const valid = !!loc && !subnetErr && parsedPorts.invalid.length === 0 && methods.length > 0 && (Number(timeout) >= 100);

  const pickIface = (name: string) => {
    setIface(name);
    const i = ifaces?.find((x) => x.name === name);
    if (i) setSubnet(i.subnet_cidr);
  };

  const start = async () => {
    setStarting(true);
    try {
      const { job_id } = await api.startDiscovery({
        location_id: loc, subnet: subnet || undefined, interface: iface || undefined, speed, timeout_ms: Number(timeout) || 800,
        tcp_ports: parsedPorts.ports, methods,
      });
      jobId.current = job_id;
      setJob({ job_id, state: 'running', progress: { done: 0, total: 0 }, error: null, results: [] });
    } catch (e) { toastError(e, 'Start scan'); }
    finally { setStarting(false); }
  };

  const cancel = async () => {
    if (!job) return;
    try { await api.cancelDiscovery(job.job_id); toast('info', 'Scan cancelled.'); setJob({ ...job, state: 'cancelled' }); } catch (e) { toastError(e, 'Cancel'); }
  };

  const register = (c: Candidate) => nav('/devices/new', { state: { prefill: { name: c.hostname ?? c.ip, hostname: c.hostname, ip: c.ip, mac: c.mac, os: c.os, device_type: c.device_type, location_id: loc, tcp_check_ports: c.open_ports } } });

  const running = job?.state === 'running';
  const pct = job?.progress && job.progress.total > 0 ? Math.min(100, (job.progress.done / job.progress.total) * 100) : 0;
  const results = job?.results ?? [];

  return (
    <>
      <PageHeader title="Discover devices" subtitle="Scan one explicitly chosen subnet. Nothing outside the selected subnet is touched, and found devices are only added when you register them." />
      {!canOperate && <div className="notice"><Icon name="info" size={16} />Your role cannot start scans.</div>}
      <Panel title="Scan settings">
        <div className="form-grid">
          <Field label="Location">
            <select value={loc} onChange={(e) => setLoc(e.target.value)} disabled={running}>{locations.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}</select>
          </Field>
          <Field label="Network interface" hint={ifaceErr ? `Could not load interfaces: ${ifaceErr}. Enter the subnet manually.` : ifaces === null ? 'Loading interfaces…' : ifaces.length === 0 ? 'No interfaces reported. Enter the subnet manually.' : 'Reported by the location’s relay.'}>
            <select value={iface} onChange={(e) => pickIface(e.target.value)} disabled={running || !ifaces || ifaces.length === 0}>
              {(ifaces ?? []).map((i) => <option key={i.name} value={i.name}>{i.name} — {i.ip}</option>)}
              {(!ifaces || ifaces.length === 0) && <option value="">—</option>}
            </select>
          </Field>
          <Field label="Subnet (CIDR)" error={subnetErr} hint="e.g. 192.168.1.0/24"><input value={subnet} onChange={(e) => setSubnet(e.target.value)} disabled={running} placeholder="192.168.1.0/24" className="mono" /></Field>
          <Field label="Speed" hint="Slow is gentler on weak routers.">
            <select value={speed} onChange={(e) => setSpeed(e.target.value as typeof speed)} disabled={running}><option value="slow">Slow</option><option value="normal">Normal</option><option value="fast">Fast</option></select>
          </Field>
          <Field label="Timeout per host (ms)"><input type="number" min={100} max={10000} value={timeout} onChange={(e) => setTimeoutMs(e.target.value)} disabled={running} /></Field>
          <Field label="TCP ports" error={parsedPorts.invalid.length ? `Invalid: ${parsedPorts.invalid.join(', ')}` : null}><input value={ports} onChange={(e) => setPorts(e.target.value)} disabled={running || !methods.includes('tcp')} /></Field>
          <fieldset className="full"><legend>Methods</legend>
            <div className="checks">{METHODS.map((m) => (
              <label key={m.id} className="chip-check" title={m.hint}><input type="checkbox" disabled={running} checked={methods.includes(m.id)} onChange={() => setMethods((l) => (l.includes(m.id) ? l.filter((x) => x !== m.id) : [...l, m.id]))} />{m.label}</label>
            ))}</div>
          </fieldset>
        </div>
        <div className="row gap-s" style={{ marginTop: 16 }}>
          {running
            ? <button className="btn danger" onClick={cancel}><Icon name="x" size={15} />Cancel scan</button>
            : <button className="btn primary" disabled={!valid || !canOperate || starting} onClick={start}><Icon name="discover" size={16} />{starting ? 'Starting…' : job ? 'Scan again' : 'Start scan'}</button>}
          {locations.length === 0 && <span className="muted">Create a location first.</span>}
        </div>
      </Panel>

      {job && (
        <Panel title="Results" actions={<Pill tone={job.state === 'done' ? 'green' : job.state === 'failed' ? 'red' : job.state === 'cancelled' ? 'amber' : 'cyan'}>{job.state}</Pill>}>
          {running && (
            <div className="stack-s" style={{ marginBottom: 14 }}>
              <div className="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}><span style={{ width: `${pct}%` }} /></div>
              <div className="muted small">{job.progress ? `${job.progress.done} of ${job.progress.total || '?'} hosts checked` : 'Starting…'} · {results.length} found so far</div>
            </div>
          )}
          {job.state === 'failed' && <ErrorBox message={job.error || 'The scan failed.'} />}
          {job.state === 'cancelled' && <p className="muted" style={{ marginBottom: 10 }}>The scan was cancelled. Partial results are shown.</p>}
          {results.length === 0 ? (running ? <Spinner label="Scanning" /> : <Empty icon="discover" title="No devices found">Check the subnet and methods. Devices that are switched off cannot be discovered.</Empty>) : (
            <div className="table-wrap">
              <table className="tbl">
                <thead><tr><th>IP</th><th>Name</th><th>MAC</th><th>Vendor</th><th>Open ports</th><th>Via</th><th aria-label="Action" /></tr></thead>
                <tbody>
                  {results.map((c) => (
                    <tr key={c.ip}>
                      <td className="mono">{c.ip}</td>
                      <td>{c.hostname || <span className="na">unknown</span>}</td>
                      <td className="mono">{c.mac || <Pill tone="amber" title="Without a MAC address Wake-on-LAN is impossible">no MAC</Pill>}</td>
                      <td>{c.vendor || <span className="na">unknown</span>}</td>
                      <td className="mono">{(c.open_ports ?? []).join(', ') || '—'}</td>
                      <td className="muted">{(c.methods ?? []).join(', ')}</td>
                      <td className="actions">
                        {c.registered ? <Pill tone="green">registered</Pill> : (
                          <button className="btn sm primary-soft" onClick={() => register(c)} title={c.mac ? 'Register this device' : 'No MAC address was found. You can register it, but you must enter the MAC yourself before Wake works.'}>
                            <Icon name="plus" size={14} />Register
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {results.some((c) => !c.mac) && <div className="notice warn" style={{ marginTop: 12 }}><Icon name="warn" size={16} />Some candidates have no MAC address (the scan may have crossed a router, or the host did not answer ARP). Without a MAC, Wake is impossible, so enter it by hand when registering.</div>}
        </Panel>
      )}
    </>
  );
}
