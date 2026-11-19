export const MAC_RE = /^([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}$|^[0-9A-Fa-f]{12}$|^([0-9A-Fa-f]{4}\.){2}[0-9A-Fa-f]{4}$/;

export function isValidMac(s: string): boolean {
  return MAC_RE.test(s.trim());
}

export function isValidIPv4(s: string): boolean {
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(s.trim());
  if (!m) return false;
  return m.slice(1).every((p) => Number(p) <= 255 && String(Number(p)) === p);
}

export function isValidPort(n: number): boolean {
  return Number.isInteger(n) && n >= 1 && n <= 65535;
}

export function isValidCidr(s: string): boolean {
  const [ip, bits] = s.trim().split('/');
  if (!ip || bits === undefined || !isValidIPv4(ip) || !/^\d+$/.test(bits)) return false;
  return Number(bits) >= 0 && Number(bits) <= 32;
}

export function isValidTime(s: string): boolean {
  return /^([01]\d|2[0-3]):[0-5]\d$/.test(s);
}

export function isValidDate(s: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(s)) return false;
  const d = new Date(`${s}T00:00:00Z`);
  return !Number.isNaN(d.getTime()) && d.toISOString().slice(0, 10) === s;
}

export function isValidHexColor(s: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(s);
}

export function isValidHost(s: string): boolean {
  const t = s.trim();
  if (!t) return false;
  return isValidIPv4(t) || /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$/.test(t);
}
