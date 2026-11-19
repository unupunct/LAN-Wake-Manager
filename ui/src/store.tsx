import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { api, friendlyMessage, getToken, setToken, setUnauthorizedHandler } from './api/client';
import { EventsClient, type LiveState } from './api/events';
import type { Device, Group, Info, LiveEvent, Location, Me, Operation, Role } from './api/types';
import { loadPref, savePref } from './lib/storage';

export type AuthStatus = 'loading' | 'setup' | 'login' | 'ready' | 'unreachable';
export interface Toast { id: number; kind: 'ok' | 'err' | 'info'; text: string }
export type Theme = 'dark' | 'light';

interface Ctx {
  auth: AuthStatus;
  info: Info | null;
  me: Me | null;
  role: Role;
  isAdmin: boolean;
  canOperate: boolean;
  signIn: (token: string) => Promise<void>;
  signOut: () => Promise<void>;
  retryBoot: () => void;

  locations: Location[];
  devices: Device[];
  groups: Group[];
  ops: Operation[];
  loaded: boolean;
  loadError: string | null;
  reload: () => Promise<void>;
  reloadDevices: () => Promise<void>;
  upsertDevice: (d: Device) => void;
  upsertOp: (o: Operation) => void;

  locationId: string;
  setLocationId: (id: string) => void;
  scopedDevices: Device[];

  live: LiveState;
  liveTick: number;

  toasts: Toast[];
  toast: (kind: Toast['kind'], text: string) => void;
  toastError: (e: unknown, prefix?: string) => void;
  dismissToast: (id: number) => void;

  theme: Theme;
  toggleTheme: () => void;
}

const C = createContext<Ctx | null>(null);

export function useApp(): Ctx {
  const v = useContext(C);
  if (!v) throw new Error('AppProvider missing');
  return v;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [auth, setAuth] = useState<AuthStatus>('loading');
  const [info, setInfo] = useState<Info | null>(null);
  const [me, setMe] = useState<Me | null>(null);
  const [locations, setLocations] = useState<Location[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [ops, setOps] = useState<Operation[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [locationId, setLocId] = useState<string>(() => loadPref('location', ''));
  const [live, setLive] = useState<LiveState>('disconnected');
  const [liveTick, setLiveTick] = useState(0);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [theme, setTheme] = useState<Theme>(() => loadPref<Theme>('theme', 'dark'));
  const toastId = useRef(0);
  const evRef = useRef<EventsClient | null>(null);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    savePref('theme', theme);
  }, [theme]);

  const dismissToast = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);
  const toast = useCallback((kind: Toast['kind'], text: string) => {
    const id = ++toastId.current;
    setToasts((t) => [...t.slice(-3), { id, kind, text }]);
    window.setTimeout(() => dismissToast(id), kind === 'err' ? 8000 : 4500);
  }, [dismissToast]);
  const toastError = useCallback((e: unknown, prefix?: string) => {
    toast('err', `${prefix ? `${prefix}: ` : ''}${friendlyMessage(e)}`);
  }, [toast]);

  const boot = useCallback(async () => {
    setAuth('loading');
    try {
      const i = await api.info();
      setInfo(i);
      if (i.setup_required) { setToken(null); setAuth('setup'); return; }
      if (!getToken()) { setAuth('login'); return; }
      try {
        const m = await api.me();
        setMe(m);
        setAuth('ready');
      } catch (e) {
        if ((e as { status?: number }).status === 401) { setToken(null); setAuth('login'); } else throw e;
      }
    } catch {
      setAuth('unreachable');
    }
  }, []);

  useEffect(() => { void boot(); }, [boot]);

  const logoutLocal = useCallback(() => {
    setToken(null);
    setMe(null);
    setLoaded(false);
    setDevices([]); setLocations([]); setGroups([]); setOps([]);
    setAuth((a) => (a === 'setup' ? a : 'login'));
  }, []);

  useEffect(() => {
    setUnauthorizedHandler(() => logoutLocal());
    return () => setUnauthorizedHandler(null);
  }, [logoutLocal]);

  const signIn = useCallback(async (token: string) => {
    setToken(token);
    const m = await api.me();
    setMe(m);
    setAuth('ready');
  }, []);

  const signOut = useCallback(async () => {
    try { await api.logout(); } catch { /* ignore */ }
    logoutLocal();
  }, [logoutLocal]);

  const reload = useCallback(async () => {
    try {
      const [l, d, g, o] = await Promise.all([api.locations(), api.devices(), api.groups(), api.operations({ limit: 50 })]);
      setLocations(l ?? []); setDevices(d ?? []); setGroups(g ?? []); setOps(o ?? []);
      setLoadError(null);
    } catch (e) {
      if ((e as { status?: number }).status !== 401) setLoadError(friendlyMessage(e));
    } finally {
      setLoaded(true);
    }
  }, []);

  const reloadDevices = useCallback(async () => {
    try {
      const [d, g, l] = await Promise.all([api.devices(), api.groups(), api.locations()]);
      setDevices(d ?? []); setGroups(g ?? []); setLocations(l ?? []);
    } catch { /* keep the old data */ }
  }, []);

  const upsertDevice = useCallback((d: Device) => {
    setDevices((list) => (list.some((x) => x.id === d.id) ? list.map((x) => (x.id === d.id ? d : x)) : [...list, d]));
  }, []);
  const upsertOp = useCallback((o: Operation) => {
    setOps((list) => {
      const i = list.findIndex((x) => x.id === o.id);
      if (i >= 0) { const c = list.slice(); c[i] = o; return c; }
      return [o, ...list].slice(0, 200);
    });
  }, []);

  useEffect(() => {
    if (auth === 'ready') void reload();
  }, [auth, reload]);

  // Live events
  const onEvent = useCallback((e: LiveEvent) => {
    switch (e.type) {
      case 'device_status':
        setDevices((list) => list.map((d) => (d.id === e.device_id ? { ...d, status: e.status, last_seen: e.last_seen ?? d.last_seen, status_changed_at: Math.floor(Date.now() / 1000) } : d)));
        setLiveTick((n) => n + 1);
        break;
      case 'operation':
        if (e.operation) upsertOp(e.operation);
        setLiveTick((n) => n + 1);
        break;
      case 'relay_status':
        setLocations((list) => list.map((l) => (l.id === e.location_id ? { ...l, relay_status: e.relay_status } : l)));
        break;
      case 'notification':
        if (e.notification && e.notification.severity !== 'info') toast(e.notification.severity === 'error' ? 'err' : 'info', e.notification.title);
        break;
      case 'schedule_fired':
        setLiveTick((n) => n + 1);
        break;
      default:
        break;
    }
  }, [toast, upsertOp]);

  useEffect(() => {
    if (auth !== 'ready') return;
    const c = new EventsClient(onEvent, (s, reconnected) => {
      setLive(s);
      if (s === 'live' && reconnected) { void reload(); setLiveTick((n) => n + 1); }
    });
    evRef.current = c;
    c.start();
    return () => { c.stop(); evRef.current = null; setLive('disconnected'); };
  }, [auth, onEvent, reload]);

  // REST fallback poll while the socket is not live.
  useEffect(() => {
    if (auth !== 'ready' || live === 'live') return;
    const t = window.setInterval(() => { void reloadDevices(); }, 15000);
    return () => window.clearInterval(t);
  }, [auth, live, reloadDevices]);

  const setLocationId = useCallback((id: string) => { setLocId(id); savePref('location', id); }, []);
  // If the saved location no longer exists, fall back to all.
  useEffect(() => {
    if (loaded && locationId && !locations.some((l) => l.id === locationId)) setLocationId('');
  }, [loaded, locations, locationId, setLocationId]);

  const scopedDevices = useMemo(() => (locationId ? devices.filter((d) => d.location_id === locationId) : devices), [devices, locationId]);

  const role: Role = me?.role ?? 'viewer';
  const value: Ctx = {
    auth, info, me, role, isAdmin: role === 'admin', canOperate: role === 'admin' || role === 'operator',
    signIn, signOut, retryBoot: () => void boot(),
    locations, devices, groups, ops, loaded, loadError, reload, reloadDevices, upsertDevice, upsertOp,
    locationId, setLocationId, scopedDevices, live, liveTick,
    toasts, toast, toastError, dismissToast,
    theme, toggleTheme: () => setTheme((t) => (t === 'dark' ? 'light' : 'dark')),
  };
  return <C.Provider value={value}>{children}</C.Provider>;
}
