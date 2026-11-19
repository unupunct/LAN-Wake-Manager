// Mock LAN Wake Manager controller for UI development. Plain Node, no dependencies.
// Usage: node dev/mock-server.mjs   (PORT=8480, MOCK_SETUP=1 to start in first-run mode)
// Serves ui/dist and implements enough of docs/API.md to exercise every page.
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const PORT = Number(process.env.PORT || 8480);
const PASSWORD = 'demo-password-123';
const DIST = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist');
const now = () => Math.floor(Date.now() / 1000);
const uid = (p = '') => p + crypto.randomBytes(5).toString('hex');
let setupRequired = process.env.MOCK_SETUP === '1';
let adminPassword = PASSWORD;

/* ---------------- state ---------------- */
const loc = (id, name, color, extra = {}) => ({
  id, name, color, notes: '', broadcast_addr: null, wol_ports: [9, 7], wake_attempts: 3, wake_interval_secs: 2, wake_timeout_secs: 120,
  relay_mode: 'builtin', relay_url: null, relay_status: 'builtin', relay_last_seen: null, relay_version: null, relay_name: null, ...extra,
});
const locations = [
  loc('l1', 'Home Network', '#2ee59d', { notes: 'Main apartment LAN' }),
  loc('l2', 'Office', '#22d3ee', { relay_mode: 'remote', relay_url: 'https://100.64.1.9:8444', relay_status: 'online', relay_last_seen: now() - 4, relay_version: '1.0.0', relay_name: 'office-nuc', broadcast_addr: '10.20.0.255' }),
  loc('l3', 'Parents House', '#f5b942', { relay_mode: 'remote', relay_url: 'https://100.64.1.30:8444', relay_status: 'offline', relay_last_seen: now() - 7200, relay_version: '1.0.0', relay_name: 'pi-parents' }),
];
const groups = [
  { id: 'g1', name: 'Workstations', icon: 'desktop', color: '#2ee59d' },
  { id: 'g2', name: 'Gaming PCs', icon: 'gamepad', color: '#a78bfa' },
  { id: 'g3', name: 'Servers', icon: 'server', color: '#22d3ee' },
  { id: 'g4', name: 'NAS', icon: 'nas', color: '#f5b942' },
  { id: 'g5', name: 'Office Computers', icon: 'building', color: '#60a5fa' },
];
const GB = 1024 ** 3;
const devices = [];
const dev = (id, location_id, name, ip, mac, os, device_type, status, group_ids, extra = {}) => devices.push({
  id, location_id, name, hostname: name.toUpperCase().replace(/\s+/g, '-'), ip, mac, os, device_type, wol_capable: extra.wol ?? 'unknown', group_ids,
  tcp_check_ports: [22, 3389, 445], power_method: extra.pm ?? 'none', ssh: extra.ssh ?? null, notes: extra.notes ?? '', status,
  status_checked_at: now() - 8, last_seen: status === 'online' ? now() - 8 : now() - 3600 * (1 + Math.random() * 40) | 0, status_changed_at: now() - 5000,
  agent: extra.agent ?? { state: 'none', version: null, last_heartbeat: null, enrolled: false }, sysinfo: extra.sysinfo ?? null, stubborn: !!extra.stubborn,
});
const sys = (o) => ({
  hostname: o.host, os_name: o.os, os_version: o.ver ?? null, kernel: o.kernel ?? null, arch: 'x86_64', uptime_secs: o.up, cpu_model: o.cpu, cpu_cores: o.cores, cpu_usage_percent: o.cpuUse,
  mem_total_bytes: o.mem * GB, mem_used_bytes: o.memUsed === null ? null : o.memUsed * GB,
  disks: o.disks, networks: [{ name: 'Ethernet', ips: [o.ip], mac: o.mac }], temperatures: o.temps, reported_at: now() - 20,
});
const ag = (state = 'online') => ({ state, version: '1.0.0', last_heartbeat: now() - 12, enrolled: true });
dev('d1', 'l1', 'Gaming PC', '192.168.1.20', 'AA:BB:CC:00:11:20', 'windows', 'pc', 'online', ['g2', 'g1'], { wol: 'yes', pm: 'agent', agent: ag(), sysinfo: sys({ host: 'DESKTOP-1', os: 'Windows 11', ver: '10.0.26200', up: 93000, cpu: 'AMD Ryzen 9 7950X', cores: 32, cpuUse: 12.5, mem: 64, memUsed: 21.4, ip: '192.168.1.20', mac: 'AA:BB:CC:00:11:20', disks: [{ mount: 'C:\\', total_bytes: 1000 * GB, free_bytes: 410 * GB }, { mount: 'D:\\', total_bytes: 2000 * GB, free_bytes: 1500 * GB }], temps: [{ label: 'CPU', celsius: 55 }, { label: 'GPU', celsius: 48 }] }) });
dev('d2', 'l1', 'Work Laptop', '192.168.1.31', 'AA:BB:CC:00:11:31', 'windows', 'laptop', 'offline', ['g1'], { pm: 'agent', agent: ag('offline') });
dev('d3', 'l1', 'Home Server', '192.168.1.5', 'AA:BB:CC:00:11:05', 'linux', 'server', 'online', ['g3'], { wol: 'yes', pm: 'ssh', ssh: { user: 'admin', port: 22, key_id: null }, sysinfo: sys({ host: 'homeserver', os: 'Ubuntu 24.04', kernel: '6.8.0', up: 1209600, cpu: 'Intel Xeon E-2278G', cores: 8, cpuUse: 4.1, mem: 32, memUsed: 11, ip: '192.168.1.5', mac: 'AA:BB:CC:00:11:05', disks: [{ mount: '/', total_bytes: 500 * GB, free_bytes: 300 * GB }], temps: null }) });
dev('d4', 'l1', 'Synology NAS', '192.168.1.8', 'AA:BB:CC:00:11:08', 'linux', 'nas', 'online', ['g4', 'g3'], { wol: 'yes' });
dev('d5', 'l1', 'Media PC', '192.168.1.44', 'AA:BB:CC:00:11:44', 'linux', 'pc', 'waking', ['g1'], { notes: 'Living room' });
dev('d6', 'l2', 'Reception PC', '10.20.0.11', 'AA:BB:CC:00:22:11', 'windows', 'pc', 'offline', ['g5', 'g1'], { pm: 'agent', agent: ag('offline') });
dev('d7', 'l2', 'Design Workstation', '10.20.0.12', 'AA:BB:CC:00:22:12', 'macos', 'pc', 'online', ['g5', 'g1'], { wol: 'yes', pm: 'ssh', ssh: { user: 'design', port: 22, key_id: null } });
dev('d8', 'l2', 'Accounting PC', '10.20.0.13', 'AA:BB:CC:00:22:13', 'windows', 'pc', 'offline', ['g5'], { stubborn: true, notes: 'WoL never works on this one' });
dev('d9', 'l2', 'Office NAS', '10.20.0.5', 'AA:BB:CC:00:22:05', 'linux', 'nas', 'unknown', ['g4'], {});
dev('d10', 'l3', 'Parents PC', '192.168.0.12', 'AA:BB:CC:00:33:12', 'windows', 'pc', 'unknown', [], {});
dev('d11', 'l1', 'Test Rig', '192.168.1.60', 'AA:BB:CC:00:11:60', 'linux', 'server', 'offline', ['g3'], { pm: 'agent', agent: ag('offline') });

let operations = [];
const addOp = (o) => { operations.unshift(o); broadcast({ type: 'operation', operation: o }); return o; };
const seedOp = (kind, d, state, message, ago, source = 'manual', by = 'admin') => operations.push({
  id: uid('op'), kind, device_id: d.id, device_name: d.name, location_id: d.location_id, state, message,
  steps: [{ t: now() - ago, text: `magic packet sent via 192.168.1.2 -> 192.168.1.255:9 (attempt 1/3)` }, { t: now() - ago + 3, text: 'magic packet sent (attempt 2/3)' }].slice(0, kind === 'wake' ? 2 : 0),
  requested_by: by, source, schedule_id: source === 'schedule' ? 's1' : null, delay_secs: 0, started_at: now() - ago, finished_at: state === 'running' ? null : now() - ago + 34,
});
seedOp('wake', devices[0], 'succeeded', 'Online after 34 s (confirmed by ping and agent heartbeat)', 3600 * 2);
seedOp('wake', devices[7], 'timeout', 'Magic packets sent, but the device did not come online within 120 s. Wake-on-LAN may be disabled in BIOS.', 3600 * 5, 'schedule', 'schedule:Wake office PCs');
seedOp('shutdown', devices[1], 'succeeded', 'Shutdown command accepted by agent; device went offline after 12 s', 3600 * 8);
seedOp('wake', devices[5], 'failed', 'Relay office-nuc was unreachable (connection timed out)', 3600 * 20, 'bulk', 'Pixel 8');
seedOp('restart', devices[2], 'succeeded', 'Restart issued via SSH; device came back after 48 s', 86400 * 2);
operations.sort((a, b) => b.started_at - a.started_at);

const schedules = [
  { id: 's1', name: 'Wake office PCs', enabled: true, action: 'wake', target: { device_ids: [], group_ids: ['g5'], location_ids: [] }, recurrence: { kind: 'weekly', at: '07:30', weekdays: [1, 2, 3, 4, 5] }, start_date: null, end_date: null, exceptions: ['2026-12-25'], missed_policy: 'run_once', missed_grace_minutes: 60, retry: { max_attempts: 3, interval_secs: 60 }, executor: 'relay', delay_secs: 0, last_run: now() - 3600 * 5, last_state: 'partial' },
  { id: 's2', name: 'Night shutdown gaming PC', enabled: true, action: 'shutdown', target: { device_ids: ['d1'], group_ids: [], location_ids: [] }, recurrence: { kind: 'daily', at: '01:00' }, start_date: null, end_date: null, exceptions: [], missed_policy: 'skip', missed_grace_minutes: 0, retry: { max_attempts: 2, interval_secs: 120 }, executor: 'controller', delay_secs: 60, last_run: now() - 3600 * 14, last_state: 'succeeded' },
  { id: 's3', name: 'Monthly NAS wake for backup', enabled: false, action: 'wake', target: { device_ids: ['d4'], group_ids: [], location_ids: [] }, recurrence: { kind: 'monthly', at: '02:00', day_of_month: 1 }, start_date: '2026-01-01', end_date: null, exceptions: [], missed_policy: 'run_once', missed_grace_minutes: 240, retry: { max_attempts: 3, interval_secs: 60 }, executor: 'controller', delay_secs: 0, last_run: null, last_state: null },
  { id: 's4', name: 'Keep server awake check', enabled: true, action: 'wake', target: { device_ids: ['d3'], group_ids: [], location_ids: [] }, recurrence: { kind: 'custom', interval: { every: 6, unit: 'hours' } }, start_date: null, end_date: '2026-12-31', exceptions: [], missed_policy: 'skip', missed_grace_minutes: 0, retry: { max_attempts: 1, interval_secs: 60 }, executor: 'controller', delay_secs: 0, last_run: now() - 3600 * 3, last_state: 'succeeded' },
];
const nextRun = (s) => (!s.enabled ? null : now() + 3600 * (2 + (s.id.charCodeAt(1) % 9)));
let mobiles = [
  { id: 'm1', name: 'Pixel 8', platform: 'android', role: 'admin', created_at: now() - 86400 * 20, last_seen: now() - 240, last_ip: '100.64.1.50', revoked: false },
  { id: 'm2', name: 'Family tablet', platform: 'android', role: 'viewer', created_at: now() - 86400 * 4, last_seen: now() - 3600 * 30, last_ip: '192.168.1.77', revoked: false },
  { id: 'm3', name: 'Old phone', platform: 'android', role: 'operator', created_at: now() - 86400 * 90, last_seen: now() - 86400 * 40, last_ip: '192.168.1.80', revoked: true },
];
const wg = [{ id: 'w1', name: 'Home WireGuard', controller_address: '10.8.0.1', port: 8443, endpoint: 'vpn.example.net:51820', allowed_ips: '10.8.0.0/24, 192.168.1.0/24', notes: 'Router peer' }];
let backups = [
  { name: 'lwm-2026-11-19-0300-auto.db', created_at: now() - 3600 * 6, size_bytes: 482304, kind: 'auto' },
  { name: 'lwm-2026-11-18-0300-auto.db', created_at: now() - 3600 * 30, size_bytes: 479232, kind: 'auto' },
  { name: 'lwm-2026-11-10-1415-manual.db', created_at: now() - 86400 * 9, size_bytes: 455680, kind: 'manual' },
];
const audit = [];
const addAudit = (actor, action, target, result = 'ok', detail = '') => audit.unshift({ id: audit.length + 1, ts: now(), actor, action, target, result, ip: '127.0.0.1', detail });
[['admin', 'auth.login', null, 'ok', ''], ['Pixel 8', 'device.wake', 'Gaming PC', 'ok', ''], ['admin', 'device.update', 'Media PC', 'ok', 'power_method changed'], ['unknown', 'auth.login', null, 'denied', 'bad password'], ['admin', 'mobile.revoke', 'Old phone', 'ok', ''], ['schedule:Wake office PCs', 'bulk.wake', '3 devices', 'ok', '1 timeout']].reverse().forEach((a) => addAudit(...a));
let settings = { name: 'Home controller', monitor_interval_secs: 15, allow_public: false, backup: { auto: true, interval_hours: 24, keep: 14 }, timezone: 'Europe/Berlin', notify_schedule_minutes: 10 };
const notifications = [
  { id: 1, ts: now() - 600, type: 'wake_completed', severity: 'info', title: 'Gaming PC is online', body: 'Wake confirmed after 34 s.', device_id: 'd1' },
  { id: 2, ts: now() - 7200, type: 'relay_offline', severity: 'warn', title: 'Relay pi-parents is offline', body: 'Parents House has not been reachable for 2 hours.', device_id: null },
  { id: 3, ts: now() - 3600 * 5, type: 'operation_failed', severity: 'error', title: 'Wake failed for Reception PC', body: 'Relay office-nuc was unreachable.', device_id: 'd6' },
];

/* ---------------- helpers ---------------- */
const sessions = new Map(); // token -> {kind, role, name}
const pairings = new Map();
const publicDevice = ({ stubborn, ...d }) => d;
const locView = (l) => {
  const ds = devices.filter((d) => d.location_id === l.id);
  return { ...l, device_count: ds.length, online_count: ds.filter((d) => d.status === 'online').length, controller_status: 'online', vpn: l.relay_mode === 'remote' ? 'tailscale' : 'lan' };
};
const groupView = (g) => { const ds = devices.filter((d) => d.group_ids.includes(g.id)); return { ...g, device_count: ds.length, online_count: ds.filter((d) => d.status === 'online').length }; };
const tailscale = { installed: true, running: true, backend_state: 'Running', ip: '100.64.1.2', hostname: 'pc', magicdns_name: 'pc.tail1234.ts.net', tailnet: 'tail1234.ts.net', error: null };

function sendJson(res, status, body, headers = {}) {
  const data = body === undefined ? '' : JSON.stringify(body);
  res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store', ...headers });
  res.end(data);
}
const err = (res, status, code, message, headers) => sendJson(res, status, { error: { code, message } }, headers);

/* ---------------- websocket ---------------- */
const clients = new Set();
function wsFrame(str) {
  const payload = Buffer.from(str);
  const len = payload.length;
  const head = len < 126 ? Buffer.from([0x81, len]) : Buffer.from([0x81, 126, len >> 8, len & 255]);
  return Buffer.concat([head, payload]);
}
function broadcast(ev) { const f = wsFrame(JSON.stringify(ev)); for (const c of clients) if (c.authed) c.socket.write(f); }
function handleUpgrade(req, socket) {
  if (!req.url.startsWith('/api/v1/events')) { socket.destroy(); return; }
  const key = req.headers['sec-websocket-key'];
  const accept = crypto.createHash('sha1').update(`${key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest('base64');
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  const c = { socket, authed: false };
  clients.add(c);
  const timer = setTimeout(() => { if (!c.authed) socket.destroy(); }, 5000);
  socket.on('data', (buf) => {
    // minimal client frame parser (single small masked text frames)
    const op = buf[0] & 15;
    if (op === 8) { socket.end(); return; }
    let len = buf[1] & 127; let off = 2;
    if (len === 126) { len = buf.readUInt16BE(2); off = 4; }
    const mask = buf.slice(off, off + 4); off += 4;
    const data = Buffer.alloc(len);
    for (let i = 0; i < len; i++) data[i] = buf[off + i] ^ mask[i % 4];
    if (op !== 1) return;
    try {
      const m = JSON.parse(data.toString());
      if (m.type === 'auth' && sessions.has(m.token)) { c.authed = true; clearTimeout(timer); socket.write(wsFrame(JSON.stringify({ type: 'hello' }))); }
      else socket.destroy();
    } catch { socket.destroy(); }
  });
  socket.on('close', () => clients.delete(c));
  socket.on('error', () => clients.delete(c));
}

/* ---------------- operation simulation ---------------- */
function setStatus(d, status) { d.status = status; d.status_changed_at = now(); if (status === 'online') d.last_seen = now(); broadcast({ type: 'device_status', device_id: d.id, status, last_seen: d.last_seen }); }
const later = (ms, fn) => setTimeout(fn, ms);
function step(op, text) { op.steps.push({ t: now(), text }); broadcast({ type: 'operation', operation: { ...op } }); }
function runWake(d, source, by) {
  const l = locations.find((x) => x.id === d.location_id);
  const op = addOp({ id: uid('op'), kind: 'wake', device_id: d.id, device_name: d.name, location_id: d.location_id, state: 'pending', message: 'Queued', steps: [], requested_by: by, source, schedule_id: null, delay_secs: 0, started_at: now(), finished_at: null });
  const relayDown = l.relay_status === 'offline';
  later(600, () => {
    op.state = 'running';
    if (relayDown) { op.state = 'failed'; op.message = `Relay ${l.relay_name} is unreachable`; op.finished_at = now(); step(op, 'relay connection timed out'); return; }
    op.message = 'Waiting for the device to come online'; setStatus(d, 'waking');
    step(op, `magic packet sent via ${l.relay_mode === 'remote' ? l.relay_name : '192.168.1.2'} -> ${l.broadcast_addr ?? '255.255.255.255'}:9 (attempt 1/3)`);
  });
  later(2600, () => { if (op.state === 'running') step(op, 'magic packet sent (attempt 2/3)'); });
  later(5200, () => {
    if (op.state !== 'running') return;
    if (d.stubborn) { op.state = 'timeout'; op.message = 'Packets were sent, but the device did not come online (mock: timing out early)'; op.finished_at = now(); setStatus(d, 'offline'); step(op, 'gave up waiting'); return; }
    op.state = 'succeeded'; op.message = `Online after ${now() - op.started_at} s (confirmed by ping)`; op.finished_at = now(); d.wol_capable = 'yes'; setStatus(d, 'online'); step(op, 'ping reply received');
  });
  return op;
}
function runPower(kind, d, delay, source, by) {
  const op = addOp({ id: uid('op'), kind, device_id: d.id, device_name: d.name, location_id: d.location_id, state: 'pending', message: 'Queued', steps: [], requested_by: by, source, schedule_id: null, delay_secs: delay, started_at: now(), finished_at: null });
  later(500, () => { op.state = 'running'; op.message = kind === 'shutdown' ? 'Shutdown command sent' : 'Restart command sent'; setStatus(d, 'shutting_down'); step(op, `command sent via ${d.power_method}`); });
  later(3500, () => {
    if (op.state !== 'running') return;
    op.state = 'succeeded'; op.finished_at = now();
    if (kind === 'shutdown') { op.message = 'Device went offline'; setStatus(d, 'offline'); } else { op.message = 'Device restarted and is back online'; setStatus(d, 'online'); }
    step(op, kind === 'shutdown' ? 'device no longer answers ping' : 'device answered ping again');
  });
  return op;
}

/* ---------------- discovery ---------------- */
const jobs = new Map();
function startJob(body) {
  const id = uid('job');
  const known = devices.map((d) => d.mac);
  const pool = [
    { ip: '192.168.1.20', mac: 'AA:BB:CC:00:11:20', hostname: 'DESKTOP-1', os: 'windows', device_type: 'pc', vendor: 'ASUSTek', open_ports: [3389, 445], methods: ['arp', 'ping', 'tcp'] },
    { ip: '192.168.1.52', mac: '3C:7C:3F:AA:12:34', hostname: 'printer-hp', os: 'other', device_type: 'other', vendor: 'HP Inc.', open_ports: [80, 443], methods: ['arp', 'ping', 'tcp'] },
    { ip: '192.168.1.71', mac: 'B8:27:EB:11:22:33', hostname: 'raspberrypi', os: 'linux', device_type: 'server', vendor: 'Raspberry Pi Foundation', open_ports: [22], methods: ['arp', 'ping', 'tcp', 'mdns'] },
    { ip: '192.168.1.88', mac: null, hostname: null, os: null, device_type: null, vendor: null, open_ports: [80], methods: ['ping', 'tcp'] },
    { ip: '192.168.1.90', mac: 'F0:18:98:55:66:77', hostname: 'MacBook-Air', os: 'macos', device_type: 'laptop', vendor: 'Apple', open_ports: [22], methods: ['arp', 'mdns'] },
    { ip: '192.168.1.5', mac: 'AA:BB:CC:00:11:05', hostname: 'homeserver', os: 'linux', device_type: 'server', vendor: 'Supermicro', open_ports: [22, 445], methods: ['arp', 'ping', 'tcp'] },
  ].map((c) => ({ ...c, registered: !!c.mac && known.includes(c.mac), device_id: devices.find((d) => d.mac === c.mac)?.id ?? null }));
  const job = { job_id: id, state: 'running', progress: { done: 0, total: 254 }, error: null, results: [], _t: setInterval(() => {
    job.progress.done = Math.min(254, job.progress.done + 32);
    const n = Math.floor((job.progress.done / 254) * pool.length);
    job.results = pool.slice(0, n);
    if (job.progress.done >= 254) { job.state = 'done'; job.results = pool; clearInterval(job._t); }
  }, 700) };
  jobs.set(id, job);
  return id;
}

/* ---------------- routing ---------------- */
const readBody = (req) => new Promise((resolve) => {
  const chunks = [];
  req.on('data', (c) => chunks.push(c));
  req.on('end', () => { try { resolve(chunks.length ? JSON.parse(Buffer.concat(chunks).toString()) : {}); } catch { resolve(null); } });
});

const failedLogins = { n: 0, until: 0 };

async function api(req, res, url) {
  const p = url.pathname.replace(/^\/api\/v1/, '') || '/';
  const m = req.method;
  const q = url.searchParams;
  const body = ['POST', 'PUT', 'PATCH'].includes(m) ? await readBody(req) : {};
  if (body === null) return err(res, 400, 'bad_request', 'invalid JSON body');

  // public
  if (p === '/info' && m === 'GET') return sendJson(res, 200, { controller_id: 'ctrl-demo-0001', name: settings.name, version: '1.0.0-mock', api_version: 1, setup_required: setupRequired, tls_fingerprint: 'a'.repeat(64), time: now() });
  if (p === '/auth/setup' && m === 'POST') {
    if (!setupRequired) return err(res, 409, 'conflict', 'setup already completed');
    if (!body.password || body.password.length < 10) return err(res, 400, 'bad_request', 'password must be at least 10 characters');
    adminPassword = body.password; setupRequired = false;
    const token = uid('sess_'); sessions.set(token, { kind: 'session', role: 'admin', name: 'admin' });
    return sendJson(res, 200, { token, expires_at: now() + 86400, role: 'admin' });
  }
  if (p === '/auth/login' && m === 'POST') {
    if (Date.now() < failedLogins.until) return err(res, 429, 'rate_limited', 'too many attempts', { 'Retry-After': String(Math.ceil((failedLogins.until - Date.now()) / 1000)) });
    if (body.password !== adminPassword) { if (++failedLogins.n >= 5) { failedLogins.until = Date.now() + 15000; failedLogins.n = 0; } return err(res, 401, 'unauthorized', 'wrong password'); }
    failedLogins.n = 0;
    const token = uid('sess_'); sessions.set(token, { kind: 'session', role: 'admin', name: 'admin' });
    return sendJson(res, 200, { token, expires_at: now() + 86400, role: 'admin' });
  }
  if (p === '/pair' && m === 'POST') return err(res, 400, 'bad_request', 'pairing is not simulated by the mock');

  // auth
  const tok = (req.headers.authorization || '').replace(/^Bearer /, '');
  const who = sessions.get(tok);
  if (!who) return err(res, 401, 'unauthorized', 'missing or invalid token');
  const admin = who.role === 'admin';
  const operator = admin || who.role === 'operator';
  if (m !== 'GET' && who.role === 'viewer') return err(res, 403, 'forbidden', 'viewers are read-only');
  const needAdmin = () => { if (!admin) { err(res, 403, 'forbidden', 'administrator role required'); return true; } return false; };
  let r;

  if (p === '/auth/logout') { sessions.delete(tok); return sendJson(res, 200, { ok: true }); }
  if (p === '/auth/password' && m === 'POST') {
    if (body.old_password !== adminPassword) return err(res, 400, 'bad_request', 'current password is wrong');
    if (!body.new_password || body.new_password.length < 10) return err(res, 400, 'bad_request', 'new password must be at least 10 characters');
    adminPassword = body.new_password; addAudit('admin', 'auth.password', null); return sendJson(res, 200, { ok: true });
  }
  if (p === '/me') return sendJson(res, 200, { ...who, ...(who.kind === 'session' ? {} : {}) });

  if (p === '/dashboard') {
    const totals = { devices: devices.length, online: devices.filter((d) => d.status === 'online').length, offline: devices.filter((d) => d.status === 'offline' || d.status === 'shutting_down').length, unknown: devices.filter((d) => ['unknown', 'waking'].includes(d.status)).length };
    return sendJson(res, 200, {
      totals, relays: { total: 2, online: locations.filter((l) => l.relay_mode === 'remote' && l.relay_status === 'online').length },
      vpn: { tailscale, wireguard_profiles: wg.length, active_connections: 1 },
      pending_schedules: schedules.filter((s) => s.enabled).length,
      failed_operations_24h: operations.filter((o) => ['failed', 'timeout'].includes(o.state) && o.started_at > now() - 86400).length,
      recent_operations: operations.slice(0, 10),
      upcoming_schedules: schedules.filter((s) => s.enabled).map((s) => ({ schedule_id: s.id, name: s.name, action: s.action, next_run: nextRun(s) })).sort((a, b) => a.next_run - b.next_run),
      locations: locations.map(locView),
    });
  }

  // locations
  if (p === '/locations') {
    if (m === 'GET') return sendJson(res, 200, locations.map(locView));
    if (needAdmin()) return;
    if (!body.name) return err(res, 400, 'bad_request', 'name is required');
    const l = loc(uid('l'), body.name, body.color ?? '#22d3ee', body); locations.push(l); addAudit('admin', 'location.create', l.name); return sendJson(res, 200, locView(l));
  }
  if ((r = p.match(/^\/locations\/([^/]+)$/))) {
    const l = locations.find((x) => x.id === r[1]); if (!l) return err(res, 404, 'not_found', 'location not found');
    if (m === 'GET') return sendJson(res, 200, locView(l));
    if (needAdmin()) return;
    if (m === 'PUT') { Object.assign(l, body); return sendJson(res, 200, locView(l)); }
    if (m === 'DELETE') { if (devices.some((d) => d.location_id === l.id)) return err(res, 409, 'conflict', 'location still has devices'); locations.splice(locations.indexOf(l), 1); return sendJson(res, 200, { ok: true }); }
  }
  if ((r = p.match(/^\/locations\/([^/]+)\/relay\/probe$/)) && m === 'POST') {
    if (needAdmin()) return;
    if (/unreachable|\.bad/.test(body.url || '')) return err(res, 502, 'relay_unreachable', 'connection refused');
    return sendJson(res, 200, { fingerprint: crypto.createHash('sha256').update(body.url || '').digest('hex'), relay_id: 'relay-' + crypto.createHash('md5').update(body.url || '').digest('hex').slice(0, 8), name: 'new-relay', version: '1.0.0' });
  }
  if ((r = p.match(/^\/locations\/([^/]+)\/relay\/pair$/)) && m === 'POST') {
    if (needAdmin()) return;
    const l = locations.find((x) => x.id === r[1]); if (!l) return err(res, 404, 'not_found', 'location not found');
    if (body.code !== '123456') return err(res, 400, 'bad_request', 'pairing code is wrong (mock accepts 123456)');
    Object.assign(l, { relay_mode: 'remote', relay_url: body.url, relay_status: 'online', relay_last_seen: now(), relay_version: '1.0.0', relay_name: 'new-relay' }); return sendJson(res, 200, locView(l));
  }
  if ((r = p.match(/^\/locations\/([^/]+)\/relay$/)) && m === 'DELETE') {
    if (needAdmin()) return;
    const l = locations.find((x) => x.id === r[1]); if (!l) return err(res, 404, 'not_found', 'location not found');
    Object.assign(l, { relay_mode: 'builtin', relay_url: null, relay_status: 'builtin', relay_last_seen: null, relay_version: null, relay_name: null }); return sendJson(res, 200, locView(l));
  }
  if ((r = p.match(/^\/locations\/([^/]+)\/interfaces$/))) {
    const l = locations.find((x) => x.id === r[1]); if (!l) return err(res, 404, 'not_found', 'location not found');
    if (l.relay_status === 'offline') return err(res, 502, 'relay_unreachable', `relay ${l.relay_name} is offline`);
    return sendJson(res, 200, l.id === 'l2'
      ? [{ name: 'eth0', ip: '10.20.0.2', netmask: '255.255.255.0', broadcast: '10.20.0.255', subnet_cidr: '10.20.0.0/24', mac: '00:11:22:33:44:55' }]
      : [{ name: 'Ethernet', ip: '192.168.1.2', netmask: '255.255.255.0', broadcast: '192.168.1.255', subnet_cidr: '192.168.1.0/24', mac: 'AA:BB:CC:00:00:02' }, { name: 'Tailscale', ip: '100.64.1.2', netmask: '255.192.0.0', broadcast: '100.127.255.255', subnet_cidr: '100.64.0.0/10', mac: null }]);
  }

  // devices
  if (p === '/devices') {
    if (m === 'GET') return sendJson(res, 200, devices.filter((d) => (!q.get('location_id') || d.location_id === q.get('location_id')) && (!q.get('group_id') || d.group_ids.includes(q.get('group_id')))).map(publicDevice));
    if (needAdmin()) return;
    if (!body.name || !body.mac || !body.location_id) return err(res, 400, 'bad_request', 'name, mac and location_id are required');
    if (devices.some((d) => d.mac.toLowerCase() === body.mac.toLowerCase())) return err(res, 409, 'conflict', 'a device with this MAC already exists');
    const base = { hostname: null, ip: null, os: null, device_type: 'pc', group_ids: [], tcp_check_ports: [], power_method: 'none', ssh: null, notes: '' };
    dev(uid('d'), body.location_id, body.name, body.ip, body.mac, body.os ?? null, 'pc', 'unknown', [], {});
    const d = devices[devices.length - 1]; Object.assign(d, base, body, { id: d.id, status: 'unknown', agent: { state: 'none', version: null, last_heartbeat: null, enrolled: false } });
    addAudit('admin', 'device.create', d.name); return sendJson(res, 200, publicDevice(d));
  }
  if ((r = p.match(/^\/devices\/([^/]+)(?:\/([a-z-]+))?$/))) {
    const d = devices.find((x) => x.id === r[1]); const sub = r[2];
    if (!d) return err(res, 404, 'not_found', 'device not found');
    if (!sub) {
      if (m === 'GET') return sendJson(res, 200, publicDevice(d));
      if (needAdmin()) return;
      if (m === 'PUT') { Object.assign(d, body, { id: d.id }); addAudit('admin', 'device.update', d.name); return sendJson(res, 200, publicDevice(d)); }
      if (m === 'DELETE') { devices.splice(devices.indexOf(d), 1); addAudit('admin', 'device.delete', d.name); return sendJson(res, 200, { ok: true }); }
    }
    if (sub === 'metrics') {
      if (d.status !== 'online' || !d.sysinfo) return sendJson(res, 200, []);
      const mins = Number(q.get('minutes') || 1440); const n = 60; const pts = [];
      for (let i = 0; i < n; i++) { const t = now() - Math.floor(((n - i) / n) * mins * 60); const w = Math.sin(i / 5) * 0.5 + 0.5; pts.push({ t, cpu: 8 + w * 40 + (i % 7), mem_used: (18 + w * 8) * GB, mem_total: 64 * GB, disk_free_min: (410 - i * 0.1) * GB, temp_max: d.sysinfo.temperatures ? 48 + w * 14 : null }); }
      return sendJson(res, 200, pts);
    }
    if (sub === 'troubleshoot') return sendJson(res, 200, { checks: [
      { name: 'MAC address format', status: 'ok', detail: `${d.mac} is valid` },
      { name: 'Broadcast address', status: 'ok', detail: 'Derived 192.168.1.255 from the interface' },
      { name: 'Last wake result', status: d.wol_capable === 'yes' ? 'ok' : 'warn', detail: d.wol_capable === 'yes' ? 'A previous wake was confirmed' : 'No wake has been confirmed yet' },
      { name: 'Relay reachability', status: locations.find((l) => l.id === d.location_id).relay_status === 'offline' ? 'fail' : 'ok', detail: 'Relay answered in 14 ms' },
      { name: 'Wired connection', status: 'info', detail: 'Wi-Fi adapters rarely support Wake-on-LAN' },
    ], advice: ['Enable “Wake on LAN” (or “Power on by PCI-E”) in the BIOS/UEFI.', 'In Windows Device Manager, enable “Wake on Magic Packet” on the network adapter and disable “Fast startup”.', 'Make sure the computer is connected via Ethernet to the same network as the relay.'] });
    if (sub === 'refresh' && m === 'POST') { d.status_checked_at = now(); return sendJson(res, 200, publicDevice(d)); }
    if (sub === 'agent-enrollment' && m === 'POST') {
      if (needAdmin()) return;
      return sendJson(res, 200, { code: 'K7Q2-9XMD', expires_at: now() + 600, controller_url: 'https://192.168.1.2:8443', fingerprint: 'a'.repeat(64), command_windows: 'powershell -ExecutionPolicy Bypass -Command "iwr https://192.168.1.2:8443/agent/install.ps1 | iex" -Code K7Q2-9XMD', command_linux: 'curl -fsSk https://192.168.1.2:8443/agent/install.sh | sudo sh -s -- --code K7Q2-9XMD' });
    }
    if (sub === 'agent' && m === 'DELETE') { if (needAdmin()) return; d.agent = { state: 'none', version: null, last_heartbeat: null, enrolled: false }; return sendJson(res, 200, { ok: true }); }
    if (['wake', 'shutdown', 'restart', 'cancel-shutdown'].includes(sub) && m === 'POST') {
      if (!operator) return err(res, 403, 'forbidden', 'operator role required');
      if (sub === 'wake') return sendJson(res, 202, runWake(d, 'manual', who.name));
      if (sub === 'cancel-shutdown') { if (d.status === 'shutting_down') setStatus(d, 'online'); return sendJson(res, 202, addOp({ id: uid('op'), kind: 'cancel_shutdown', device_id: d.id, device_name: d.name, location_id: d.location_id, state: 'succeeded', message: 'Shutdown cancelled', steps: [], requested_by: who.name, source: 'manual', schedule_id: null, delay_secs: 0, started_at: now(), finished_at: now() })); }
      if (d.power_method === 'none') return err(res, 400, 'bad_request', 'no power method configured for this device');
      return sendJson(res, 202, runPower(sub, d, body.delay_secs || 0, 'manual', who.name));
    }
  }
  if ((p === '/bulk' || (r = p.match(/^\/groups\/([^/]+)\/action$/))) && m === 'POST') {
    if (!operator) return err(res, 403, 'forbidden', 'operator role required');
    if (!['wake', 'shutdown', 'restart'].includes(body.action)) return err(res, 400, 'bad_request', 'invalid action');
    if (body.action !== 'wake' && body.confirm !== true) return err(res, 400, 'confirmation_required', 'confirm:true is required for shutdown and restart');
    let targets = devices;
    if (r) targets = devices.filter((d) => d.group_ids.includes(r[1]));
    else if (body.device_ids) targets = devices.filter((d) => body.device_ids.includes(d.id));
    else if (body.group_id) targets = devices.filter((d) => d.group_ids.includes(body.group_id));
    else if (body.location_id) targets = devices.filter((d) => d.location_id === body.location_id);
    const ops = [];
    for (const d of targets) {
      if (body.action === 'wake' && d.status !== 'online') ops.push(runWake(d, 'bulk', who.name));
      else if (body.action !== 'wake' && d.power_method !== 'none' && d.status === 'online') ops.push(runPower(body.action, d, body.delay_secs || 0, 'bulk', who.name));
    }
    addAudit(who.name, `bulk.${body.action}`, `${ops.length} devices`);
    return sendJson(res, 200, { operations: ops });
  }
  if (p === '/operations') {
    let list = operations.filter((o) => (!q.get('device_id') || o.device_id === q.get('device_id')) && (!q.get('state') || o.state === q.get('state')) && (!q.get('before') || o.started_at < Number(q.get('before'))));
    return sendJson(res, 200, list.slice(0, Number(q.get('limit') || 50)));
  }
  if ((r = p.match(/^\/operations\/([^/]+)$/))) { const o = operations.find((x) => x.id === r[1]); return o ? sendJson(res, 200, o) : err(res, 404, 'not_found', 'operation not found'); }

  // groups
  if (p === '/groups') {
    if (m === 'GET') return sendJson(res, 200, groups.map(groupView));
    if (needAdmin()) return;
    if (!body.name) return err(res, 400, 'bad_request', 'name is required');
    const g = { id: uid('g'), name: body.name, icon: body.icon || 'desktop', color: body.color || '#2ee59d' }; groups.push(g); return sendJson(res, 200, groupView(g));
  }
  if ((r = p.match(/^\/groups\/([^/]+)$/))) {
    const g = groups.find((x) => x.id === r[1]); if (!g) return err(res, 404, 'not_found', 'group not found');
    if (needAdmin()) return;
    if (m === 'PUT') { Object.assign(g, body, { id: g.id }); return sendJson(res, 200, groupView(g)); }
    if (m === 'DELETE') { groups.splice(groups.indexOf(g), 1); devices.forEach((d) => { d.group_ids = d.group_ids.filter((x) => x !== g.id); }); return sendJson(res, 200, { ok: true }); }
  }

  // schedules
  if (p === '/schedules') {
    if (m === 'GET') return sendJson(res, 200, schedules.map((s) => ({ ...s, next_run: nextRun(s) })));
    if (!operator) return err(res, 403, 'forbidden', 'operator role required');
    if (!body.name) return err(res, 400, 'bad_request', 'name is required');
    const s = { id: uid('s'), last_run: null, last_state: null, ...body }; schedules.push(s); return sendJson(res, 200, { ...s, next_run: nextRun(s) });
  }
  if ((r = p.match(/^\/schedules\/([^/]+)(\/run-now)?$/))) {
    const s = schedules.find((x) => x.id === r[1]); if (!s) return err(res, 404, 'not_found', 'schedule not found');
    if (!operator) return err(res, 403, 'forbidden', 'operator role required');
    if (r[2]) { const ds = devices.filter((d) => s.target.device_ids.includes(d.id) || d.group_ids.some((g) => s.target.group_ids.includes(g)) || s.target.location_ids.includes(d.location_id)); ds.forEach((d) => (s.action === 'wake' ? runWake(d, 'schedule', `schedule:${s.name}`) : d.power_method !== 'none' && runPower(s.action, d, s.delay_secs, 'schedule', `schedule:${s.name}`))); s.last_run = now(); s.last_state = 'succeeded'; return sendJson(res, 200, { ok: true }); }
    if (m === 'PUT') { Object.assign(s, body, { id: s.id }); return sendJson(res, 200, { ...s, next_run: nextRun(s) }); }
    if (m === 'DELETE') { schedules.splice(schedules.indexOf(s), 1); return sendJson(res, 200, { ok: true }); }
  }

  // discovery
  if (p === '/discovery' && m === 'POST') {
    if (!operator) return err(res, 403, 'forbidden', 'operator role required');
    const bits = Number((body.subnet || '/24').split('/')[1]); if (bits < 17) return err(res, 400, 'bad_request', 'subnets larger than /17 are refused');
    return sendJson(res, 200, { job_id: startJob(body) });
  }
  if ((r = p.match(/^\/discovery\/([^/]+)$/))) {
    const j = jobs.get(r[1]); if (!j) return err(res, 404, 'not_found', 'job not found');
    if (m === 'DELETE') { clearInterval(j._t); if (j.state === 'running') j.state = 'cancelled'; return sendJson(res, 200, { ok: true }); }
    const { _t, ...pub } = j; return sendJson(res, 200, pub);
  }

  // connectivity
  if (p === '/connectivity') return sendJson(res, 200, { tailscale, addresses: [{ address: '192.168.1.2', kind: 'lan', interface: 'Ethernet' }, { address: '100.64.1.2', kind: 'tailscale', interface: 'Tailscale' }, { address: '10.8.0.1', kind: 'wireguard', interface: 'wg0' }], port: 8443, allow_public: settings.allow_public });
  if (p === '/wireguard-profiles') {
    if (m === 'GET') return sendJson(res, 200, wg);
    if (needAdmin()) return; const w = { id: uid('w'), ...body }; wg.push(w); return sendJson(res, 200, w);
  }
  if ((r = p.match(/^\/wireguard-profiles\/([^/]+)$/))) {
    if (needAdmin()) return; const w = wg.find((x) => x.id === r[1]); if (!w) return err(res, 404, 'not_found', 'profile not found');
    if (m === 'PUT') { Object.assign(w, body, { id: w.id }); return sendJson(res, 200, w); }
    if (m === 'DELETE') { wg.splice(wg.indexOf(w), 1); return sendJson(res, 200, { ok: true }); }
  }
  if (p === '/diagnostics' && m === 'GET') {
    const l = q.get('location_id') && locations.find((x) => x.id === q.get('location_id'));
    const checks = [
      { name: 'Controller service', status: 'ok', detail: 'Listening on 8443 (HTTPS) and 127.0.0.1:8480', duration_ms: 1 },
      { name: 'Database', status: 'ok', detail: 'SQLite integrity check passed', duration_ms: 12 },
      { name: 'Scheduler', status: 'ok', detail: `${schedules.filter((s) => s.enabled).length} active schedules, next run planned`, duration_ms: 2 },
      { name: 'Tailscale', status: 'ok', detail: 'Running, address 100.64.1.2', duration_ms: 40 },
      { name: 'WireGuard', status: 'info', detail: 'No WireGuard interface detected; profile is descriptive only', duration_ms: 3 },
      ...locations.filter((x) => !l || x.id === l.id).filter((x) => x.relay_mode === 'remote').map((x) => (x.relay_status === 'online' ? { name: `Relay ${x.relay_name}`, status: 'ok', detail: 'Reachable, latency 23 ms', duration_ms: 23 } : { name: `Relay ${x.relay_name}`, status: 'fail', detail: 'Connection timed out. Check that the relay computer is on and connected to Tailscale/VPN.', duration_ms: 3000 })),
      { name: 'Public access', status: settings.allow_public ? 'warn' : 'ok', detail: settings.allow_public ? 'Public addresses are accepted. This is not recommended.' : 'Only private and VPN addresses are accepted', duration_ms: 0 },
    ];
    return sendJson(res, 200, { checks, generated_at: now() });
  }
  if ((r = p.match(/^\/diagnostics\/(ping|dns|tcp)$/)) && m === 'POST') {
    if (!body.host) return err(res, 400, 'bad_request', 'host is required');
    const bad = /bad|nope/.test(body.host);
    return sendJson(res, 200, { ok: !bad, detail: bad ? `${r[1]} to ${body.host} failed: no response` : r[1] === 'dns' ? `${body.host} -> 192.168.1.20` : `${body.host} reachable`, duration_ms: bad ? 3000 : 7 });
  }

  // mobile
  if (p === '/pairing' && m === 'POST') {
    if (needAdmin()) return;
    const ttl = Math.min(600, body.ttl_secs || 300); const token = uid('pt_');
    pairings.set(token, body.role);
    return sendJson(res, 200, { pairing_token: token, expires_at: now() + ttl, qr_payload: JSON.stringify({ v: 1, id: 'ctrl-demo-0001', name: settings.name, addrs: ['https://100.64.1.2:8443', 'https://192.168.1.2:8443'], fp: 'a'.repeat(64), token, exp: now() + ttl }), addresses: ['https://100.64.1.2:8443', 'https://192.168.1.2:8443'] });
  }
  if (p === '/mobile-devices' && m === 'GET') return sendJson(res, 200, mobiles);
  if ((r = p.match(/^\/mobile-devices\/([^/]+)$/))) {
    if (needAdmin()) return; const x = mobiles.find((y) => y.id === r[1]); if (!x) return err(res, 404, 'not_found', 'device not found');
    if (m === 'PATCH') { Object.assign(x, body, { id: x.id }); return sendJson(res, 200, x); }
    if (m === 'DELETE') { x.revoked = true; addAudit('admin', 'mobile.revoke', x.name); return sendJson(res, 200, { ok: true }); }
  }

  // audit, notifications, settings, backups
  if (p === '/audit' && m === 'GET') { if (needAdmin()) return; return sendJson(res, 200, audit.filter((a) => !q.get('before') || a.ts < Number(q.get('before'))).slice(0, Number(q.get('limit') || 100))); }
  if (p === '/notifications') return sendJson(res, 200, notifications);
  if (p === '/settings') {
    if (m === 'GET') return sendJson(res, 200, settings);
    if (needAdmin()) return; settings = { ...settings, ...body }; addAudit('admin', 'settings.update', null); return sendJson(res, 200, settings);
  }
  if (p === '/backups') {
    if (m === 'GET') return sendJson(res, 200, backups);
    if (needAdmin()) return; const b = { name: `lwm-${new Date().toISOString().slice(0, 16).replace(/[-:T]/g, '')}-manual.db`, created_at: now(), size_bytes: 490000, kind: 'manual' }; backups.unshift(b); return sendJson(res, 200, b);
  }
  if ((r = p.match(/^\/backups\/([^/]+)\/restore$/)) && m === 'POST') { if (needAdmin()) return; if (!backups.some((b) => b.name === decodeURIComponent(r[1]))) return err(res, 404, 'not_found', 'backup not found'); addAudit('admin', 'backup.restore', r[1]); return sendJson(res, 200, { ok: true }); }
  if (p === '/export' && m === 'GET') { if (needAdmin()) return; res.writeHead(200, { 'Content-Type': 'application/json', 'Content-Disposition': 'attachment; filename="lwm-export.json"' }); return res.end(JSON.stringify({ version: 1, exported_at: now(), locations: locations.map(({ ...l }) => l), devices: devices.map(publicDevice), groups, schedules, settings }, null, 2)); }
  if (p === '/import' && m === 'POST') { if (needAdmin()) return; return sendJson(res, 200, { ok: true }); }

  return err(res, 404, 'not_found', `no such endpoint: ${m} ${p}`);
}

/* ---------------- static ---------------- */
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.ico': 'image/x-icon' };
function serveStatic(req, res, url) {
  let file = path.normalize(path.join(DIST, decodeURIComponent(url.pathname)));
  if (!file.startsWith(DIST)) { res.writeHead(403); return res.end(); }
  if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) file = path.join(DIST, 'index.html');
  if (!fs.existsSync(file)) { res.writeHead(503, { 'Content-Type': 'text/plain' }); return res.end('ui/dist is missing. Run: npm run build'); }
  res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-cache' });
  fs.createReadStream(file).pipe(res);
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, 'http://x');
  if (url.pathname.startsWith('/api/v1')) {
    try { await api(req, res, url); } catch (e) { console.error(e); err(res, 500, 'internal', String(e)); }
  } else serveStatic(req, res, url);
});
server.on('upgrade', handleUpgrade);
server.listen(PORT, '127.0.0.1', () => console.log(`Mock controller on http://127.0.0.1:${PORT}  (password: ${PASSWORD}${setupRequired ? ', first-run setup mode' : ''})`));

// ambient: a device flips state now and then so live updates are visible
setInterval(() => {
  const d = devices.find((x) => x.id === 'd9');
  if (d && Math.random() < 0.3) setStatus(d, d.status === 'online' ? 'offline' : 'online');
}, 20000);
