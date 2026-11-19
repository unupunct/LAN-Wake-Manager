export function loadPref<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`lwm.${key}`);
    if (raw === null) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export function savePref(key: string, value: unknown) {
  try { localStorage.setItem(`lwm.${key}`, JSON.stringify(value)); } catch { /* ignore */ }
}
