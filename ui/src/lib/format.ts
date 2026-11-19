export function formatBytes(n: number | null | undefined): string {
  if (n === null || n === undefined || !Number.isFinite(n) || n < 0) return 'not reported';
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

export function formatDuration(secs: number | null | undefined): string {
  if (secs === null || secs === undefined || !Number.isFinite(secs) || secs < 0) return 'not reported';
  const s = Math.floor(secs);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

export function formatPercent(n: number | null | undefined): string {
  if (n === null || n === undefined || !Number.isFinite(n)) return 'not reported';
  return `${n.toFixed(n >= 10 ? 0 : 1)}%`;
}

export function formatTemp(c: number | null | undefined): string {
  if (c === null || c === undefined || !Number.isFinite(c)) return 'not reported';
  return `${c.toFixed(0)} °C`;
}

export function formatTime(ts: number | null | undefined): string {
  if (!ts) return 'never';
  return new Date(ts * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

export function formatClock(ts: number | null | undefined): string {
  if (!ts) return '';
  return new Date(ts * 1000).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

/** Relative time ("3 min ago", "in 2 h"). `now` in seconds, injectable for tests. */
export function relativeTime(ts: number | null | undefined, now = Date.now() / 1000): string {
  if (!ts) return 'never';
  const diff = Math.round(ts - now);
  const abs = Math.abs(diff);
  let text: string;
  if (abs < 5) return 'just now';
  if (abs < 60) text = `${abs} s`;
  else if (abs < 3600) text = `${Math.round(abs / 60)} min`;
  else if (abs < 86400) text = `${Math.round(abs / 3600)} h`;
  else text = `${Math.round(abs / 86400)} d`;
  return diff < 0 ? `${text} ago` : `in ${text}`;
}

export function countdown(expiresAt: number, now = Date.now() / 1000): string {
  const left = Math.max(0, Math.floor(expiresAt - now));
  const m = Math.floor(left / 60);
  const s = left % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export function normalizeMac(input: string): string {
  const hex = input.replace(/[^0-9a-fA-F]/g, '').toUpperCase();
  if (hex.length !== 12) return input.trim();
  return hex.match(/.{2}/g)!.join(':');
}

export function statusLabel(s: string): string {
  switch (s) {
    case 'online': return 'Online';
    case 'offline': return 'Offline';
    case 'waking': return 'Waking';
    case 'shutting_down': return 'Shutting down';
    case 'unknown': return 'Unknown';
    default: return s;
  }
}

export function opKindLabel(k: string): string {
  switch (k) {
    case 'wake': return 'Wake';
    case 'shutdown': return 'Shutdown';
    case 'restart': return 'Restart';
    case 'cancel_shutdown': return 'Cancel shutdown';
    default: return k;
  }
}

/** Honest wording for operation state; a wake timeout is not a success. */
export function opStateLabel(state: string, kind?: string): string {
  switch (state) {
    case 'pending': return 'Queued';
    case 'running': return kind === 'wake' ? 'Waiting for device' : 'Running';
    case 'succeeded': return kind === 'wake' ? 'Confirmed online' : 'Done';
    case 'failed': return 'Failed';
    case 'timeout': return kind === 'wake' ? 'Packet sent, not online' : 'Timed out';
    default: return state;
  }
}

export function parsePorts(input: string): { ports: number[]; invalid: string[] } {
  const ports: number[] = [];
  const invalid: string[] = [];
  for (const part of input.split(/[\s,;]+/).filter(Boolean)) {
    const n = Number(part);
    if (/^\d+$/.test(part) && n >= 1 && n <= 65535) { if (!ports.includes(n)) ports.push(n); } else invalid.push(part);
  }
  return { ports, invalid };
}

export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}
