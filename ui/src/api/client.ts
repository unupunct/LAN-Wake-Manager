import type * as T from './types';

const TOKEN_KEY = 'lwm.token';
export const API_BASE = '/api/v1';

export class ApiError extends Error {
  status: number;
  code: string;
  retryAfter?: number;
  constructor(status: number, code: string, message: string, retryAfter?: number) {
    super(message);
    this.status = status;
    this.code = code;
    this.retryAfter = retryAfter;
  }
}

export function friendlyMessage(err: unknown): string {
  if (err instanceof ApiError) {
    switch (err.code) {
      case 'unauthorized': return 'Your session has expired or the credentials are wrong. Please sign in again.';
      case 'forbidden': return 'Your account role does not allow this action.';
      case 'rate_limited': return `Too many attempts. Please wait${err.retryAfter ? ` ${err.retryAfter} seconds` : ' a moment'} and try again.`;
      case 'relay_unreachable': return `The relay at that location cannot be reached. ${err.message}`.trim();
      case 'confirmation_required': return 'This action needs an explicit confirmation.';
      default: return err.message || `The controller reported an error (${err.status}).`;
    }
  }
  if (err instanceof Error) return err.message;
  return 'Something went wrong.';
}

let onUnauthorized: (() => void) | null = null;
export function setUnauthorizedHandler(h: (() => void) | null) { onUnauthorized = h; }

export function getToken(): string | null {
  try { return localStorage.getItem(TOKEN_KEY); } catch { return null; }
}
export function setToken(t: string | null) {
  try { if (t) localStorage.setItem(TOKEN_KEY, t); else localStorage.removeItem(TOKEN_KEY); } catch { /* ignore */ }
}

export function buildQuery(params?: Record<string, string | number | boolean | null | undefined>): string {
  if (!params) return '';
  const q = Object.entries(params)
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
    .join('&');
  return q ? `?${q}` : '';
}

/** Parse an error response into ApiError, tolerating non-JSON bodies. */
export async function parseError(res: Response): Promise<ApiError> {
  let code = 'internal';
  let message = '';
  try {
    const text = await res.text();
    if (text) {
      try {
        const j = JSON.parse(text);
        if (j && typeof j === 'object' && j.error) {
          code = String(j.error.code ?? code);
          message = String(j.error.message ?? '');
        } else message = text.slice(0, 200);
      } catch { message = text.slice(0, 200); }
    }
  } catch { /* ignore */ }
  if (!message || message.startsWith('<')) {
    message = res.status === 404 ? 'Not found.' : res.status >= 500 ? 'The controller had an internal error.' : `Request failed (${res.status}).`;
  }
  if (res.status === 401) code = 'unauthorized';
  const ra = Number(res.headers.get('Retry-After'));
  return new ApiError(res.status, code, message, Number.isFinite(ra) && ra > 0 ? ra : undefined);
}

export interface ReqOpts {
  method?: string;
  body?: unknown;
  query?: Record<string, string | number | boolean | null | undefined>;
  auth?: boolean;
  signal?: AbortSignal;
}

export async function request<R>(path: string, opts: ReqOpts = {}): Promise<R> {
  const headers: Record<string, string> = { Accept: 'application/json' };
  if (opts.body !== undefined) headers['Content-Type'] = 'application/json';
  const tok = getToken();
  if (tok && opts.auth !== false) headers.Authorization = `Bearer ${tok}`;
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}${buildQuery(opts.query)}`, {
      method: opts.method ?? (opts.body !== undefined ? 'POST' : 'GET'),
      headers,
      body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
      signal: opts.signal,
    });
  } catch (err) {
    if (err instanceof DOMException && err.name === 'AbortError') throw err;
    throw new ApiError(0, 'network', 'Cannot reach the controller. Check that it is running and your network connection.');
  }
  if (!res.ok) {
    const apiErr = await parseError(res);
    if (res.status === 401 && opts.auth !== false && onUnauthorized) onUnauthorized();
    throw apiErr;
  }
  if (res.status === 204) return undefined as R;
  const text = await res.text();
  if (!text) return undefined as R;
  try { return JSON.parse(text) as R; } catch { throw new ApiError(res.status, 'internal', 'The controller sent an unreadable response.'); }
}

async function download(path: string): Promise<Blob> {
  const tok = getToken();
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, { headers: tok ? { Authorization: `Bearer ${tok}` } : {} });
  } catch {
    throw new ApiError(0, 'network', 'Cannot reach the controller.');
  }
  if (!res.ok) {
    const apiErr = await parseError(res);
    if (res.status === 401 && onUnauthorized) onUnauthorized();
    throw apiErr;
  }
  return res.blob();
}

const get = <R>(p: string, query?: ReqOpts['query']) => request<R>(p, { query });
const post = <R>(p: string, body: unknown = {}) => request<R>(p, { method: 'POST', body });
const put = <R>(p: string, body: unknown) => request<R>(p, { method: 'PUT', body });
const patch = <R>(p: string, body: unknown) => request<R>(p, { method: 'PATCH', body });
const del = <R = unknown>(p: string) => request<R>(p, { method: 'DELETE' });
const e = encodeURIComponent;

export const api = {
  info: () => request<T.Info>('/info', { auth: false }),
  setup: (password: string) => request<T.AuthResult>('/auth/setup', { method: 'POST', body: { password }, auth: false }),
  login: (password: string) => request<T.AuthResult>('/auth/login', { method: 'POST', body: { password }, auth: false }),
  logout: () => post('/auth/logout'),
  changePassword: (old_password: string, new_password: string) => post('/auth/password', { old_password, new_password }),
  me: () => get<T.Me>('/me'),
  dashboard: () => get<T.Dashboard>('/dashboard'),

  locations: () => get<T.Location[]>('/locations'),
  createLocation: (b: T.LocationInput) => post<T.Location>('/locations', b),
  updateLocation: (id: string, b: T.LocationInput) => put<T.Location>(`/locations/${e(id)}`, b),
  deleteLocation: (id: string) => del(`/locations/${e(id)}`),
  relayProbe: (id: string, url: string) => post<T.RelayProbe>(`/locations/${e(id)}/relay/probe`, { url }),
  relayPair: (id: string, url: string, code: string, fingerprint: string) => post<T.Location>(`/locations/${e(id)}/relay/pair`, { url, code, fingerprint }),
  relayUnpair: (id: string) => del<T.Location>(`/locations/${e(id)}/relay`),
  interfaces: (id: string) => get<T.NetInterface[]>(`/locations/${e(id)}/interfaces`),

  devices: (q?: { location_id?: string; group_id?: string }) => get<T.Device[]>('/devices', q),
  device: (id: string) => get<T.Device>(`/devices/${e(id)}`),
  createDevice: (b: T.DeviceInput) => post<T.Device>('/devices', b),
  updateDevice: (id: string, b: T.DeviceInput) => put<T.Device>(`/devices/${e(id)}`, b),
  deleteDevice: (id: string) => del(`/devices/${e(id)}`),
  metrics: (id: string, minutes = 1440) => get<T.MetricPoint[]>(`/devices/${e(id)}/metrics`, { minutes }),
  troubleshoot: (id: string) => get<T.Troubleshoot>(`/devices/${e(id)}/troubleshoot`),
  refreshDevice: (id: string) => post<T.Device>(`/devices/${e(id)}/refresh`),
  enroll: (id: string) => post<T.Enrollment>(`/devices/${e(id)}/agent-enrollment`),
  revokeAgent: (id: string) => del(`/devices/${e(id)}/agent`),

  wake: (id: string) => post<T.Operation>(`/devices/${e(id)}/wake`),
  shutdown: (id: string, delay_secs = 0) => post<T.Operation>(`/devices/${e(id)}/shutdown`, { delay_secs }),
  restart: (id: string, delay_secs = 0) => post<T.Operation>(`/devices/${e(id)}/restart`, { delay_secs }),
  cancelShutdown: (id: string) => post<T.Operation>(`/devices/${e(id)}/cancel-shutdown`),
  bulk: (b: T.BulkRequest) => post<{ operations: T.Operation[] }>('/bulk', b),
  groupAction: (id: string, b: Omit<T.BulkRequest, 'device_ids' | 'group_id' | 'location_id'>) => post<{ operations: T.Operation[] }>(`/groups/${e(id)}/action`, b),
  operations: (q?: { limit?: number; before?: number; device_id?: string; state?: string }) => get<T.Operation[]>('/operations', q),
  operation: (id: string) => get<T.Operation>(`/operations/${e(id)}`),

  groups: () => get<T.Group[]>('/groups'),
  createGroup: (b: T.GroupInput) => post<T.Group>('/groups', b),
  updateGroup: (id: string, b: T.GroupInput) => put<T.Group>(`/groups/${e(id)}`, b),
  deleteGroup: (id: string) => del(`/groups/${e(id)}`),

  schedules: () => get<T.Schedule[]>('/schedules'),
  createSchedule: (b: T.ScheduleInput) => post<T.Schedule>('/schedules', b),
  updateSchedule: (id: string, b: T.ScheduleInput) => put<T.Schedule>(`/schedules/${e(id)}`, b),
  deleteSchedule: (id: string) => del(`/schedules/${e(id)}`),
  runSchedule: (id: string) => post(`/schedules/${e(id)}/run-now`),

  startDiscovery: (b: T.DiscoveryRequest) => post<{ job_id: string }>('/discovery', b),
  discovery: (id: string) => get<T.DiscoveryJob>(`/discovery/${e(id)}`),
  cancelDiscovery: (id: string) => del(`/discovery/${e(id)}`),

  connectivity: () => get<T.Connectivity>('/connectivity'),
  wgProfiles: () => get<T.WgProfile[]>('/wireguard-profiles'),
  createWg: (b: T.WgProfileInput) => post<T.WgProfile>('/wireguard-profiles', b),
  updateWg: (id: string, b: T.WgProfileInput) => put<T.WgProfile>(`/wireguard-profiles/${e(id)}`, b),
  deleteWg: (id: string) => del(`/wireguard-profiles/${e(id)}`),
  diagnostics: (location_id?: string) => get<T.Diagnostics>('/diagnostics', { location_id }),
  ping: (host: string, location_id?: string) => post<T.ToolResult>('/diagnostics/ping', { host, location_id }),
  dns: (host: string, location_id?: string) => post<T.ToolResult>('/diagnostics/dns', { host, location_id }),
  tcp: (host: string, port: number, location_id?: string) => post<T.ToolResult>('/diagnostics/tcp', { host, port, location_id }),

  createPairing: (role: T.Role, ttl_secs = 300) => post<T.Pairing>('/pairing', { role, ttl_secs }),
  mobileDevices: () => get<T.MobileDevice[]>('/mobile-devices'),
  updateMobile: (id: string, b: { name?: string; role?: T.Role }) => patch<T.MobileDevice>(`/mobile-devices/${e(id)}`, b),
  revokeMobile: (id: string) => del(`/mobile-devices/${e(id)}`),

  audit: (q?: { limit?: number; before?: number }) => get<T.AuditEntry[]>('/audit', q),
  notifications: (q?: { after?: number | string; limit?: number }) => get<T.Notification[]>('/notifications', q),
  settings: () => get<T.Settings>('/settings'),
  saveSettings: (b: T.Settings) => put<T.Settings>('/settings', b),
  backups: () => get<T.BackupEntry[]>('/backups'),
  createBackup: () => post<T.BackupEntry>('/backups'),
  restoreBackup: (name: string) => post(`/backups/${e(name)}/restore`),
  exportConfig: () => download('/export'),
  importConfig: (doc: unknown) => post('/import', doc),
};
