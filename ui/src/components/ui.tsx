import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { friendlyMessage } from '../api/client';
import type { Check, DeviceStatus, OpState } from '../api/types';
import { opStateLabel, statusLabel } from '../lib/format';
import { useApp } from '../store';
import { Icon } from './Icon';

/* ---------- data loading hook ---------- */
export function useLoad<T>(fn: () => Promise<T>, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const fnRef = useRef(fn);
  fnRef.current = fn;
  const seq = useRef(0);
  const reload = useCallback(async (silent = false) => {
    const my = ++seq.current;
    if (!silent) setLoading(true);
    try {
      const d = await fnRef.current();
      if (my === seq.current) { setData(d); setError(null); }
    } catch (e) {
      if (my === seq.current) setError(friendlyMessage(e));
    } finally {
      if (my === seq.current) setLoading(false);
    }
  }, []);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { void reload(); }, deps);
  return { data, setData, error, loading, reload };
}

/* ---------- badges ---------- */
export function StatusBadge({ status, small }: { status: DeviceStatus | string; small?: boolean }) {
  return (
    <span className={`badge st-${status}${small ? ' sm' : ''}`}>
      <span className="dot" aria-hidden="true" />
      {statusLabel(status)}
    </span>
  );
}

export function OpBadge({ state, kind }: { state: OpState | string; kind?: string }) {
  return <span className={`badge op-${state}`}><span className="dot" aria-hidden="true" />{opStateLabel(state, kind)}</span>;
}

export function Pill({ children, tone = 'muted', title }: { children: ReactNode; tone?: 'muted' | 'green' | 'cyan' | 'amber' | 'red'; title?: string }) {
  return <span className={`pill pill-${tone}`} title={title}>{children}</span>;
}

export function CheckBadge({ status }: { status: Check['status'] }) {
  const label = { ok: 'OK', warn: 'Warning', fail: 'Failed', info: 'Info' }[status] ?? status;
  return <span className={`badge ck-${status}`}><span className="dot" aria-hidden="true" />{label}</span>;
}

/* ---------- states ---------- */
export function Spinner({ label = 'Loading' }: { label?: string }) {
  return <div className="loading" role="status"><span className="spinner" aria-hidden="true" />{label}…</div>;
}

export function Empty({ icon = 'info', title, children, action }: { icon?: string; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="empty">
      <div className="empty-icon"><Icon name={icon} size={26} /></div>
      <h3>{title}</h3>
      {children && <p>{children}</p>}
      {action}
    </div>
  );
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="errorbox" role="alert">
      <Icon name="warn" />
      <div className="grow">{message}</div>
      {onRetry && <button className="btn sm" onClick={onRetry}>Try again</button>}
    </div>
  );
}

export function PageHeader({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children?: ReactNode }) {
  return (
    <header className="page-head">
      <div>
        <h1>{title}</h1>
        {subtitle && <p className="sub">{subtitle}</p>}
      </div>
      {children && <div className="page-actions">{children}</div>}
    </header>
  );
}

export function Panel({ title, actions, children, className = '' }: { title?: ReactNode; actions?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`panel ${className}`}>
      {(title || actions) && (
        <div className="panel-head">
          <h2>{title}</h2>
          {actions && <div className="row gap-s">{actions}</div>}
        </div>
      )}
      {children}
    </section>
  );
}

/* ---------- modal ---------- */
export function Modal({ title, onClose, children, footer, wide, drawer, locked }: {
  title: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean; drawer?: boolean; locked?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const focusable = () => Array.from(el?.querySelectorAll<HTMLElement>('button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])') ?? []);
    const first = el?.querySelector<HTMLElement>('[data-autofocus]') ?? (drawer ? el : focusable().find((x) => !x.classList.contains('modal-x')) ?? el);
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !locked) { e.stopPropagation(); onClose(); }
      if (e.key === 'Tab' && el) {
        const f = focusable();
        if (!f.length) return;
        const a = f[0], z = f[f.length - 1];
        if (e.shiftKey && document.activeElement === a) { e.preventDefault(); z.focus(); }
        else if (!e.shiftKey && document.activeElement === z) { e.preventDefault(); a.focus(); }
      }
    };
    document.addEventListener('keydown', onKey, true);
    document.body.classList.add('modal-open');
    return () => {
      document.removeEventListener('keydown', onKey, true);
      document.body.classList.remove('modal-open');
      prev?.focus?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div className={`overlay${drawer ? ' drawer-wrap' : ''}`} onMouseDown={(e) => { if (e.target === e.currentTarget && !locked) onClose(); }}>
      <div className={`modal${wide ? ' wide' : ''}${drawer ? ' drawer' : ''}`} role="dialog" aria-modal="true" aria-labelledby={titleId} ref={ref} tabIndex={-1}>
        <div className="modal-head">
          <h2 id={titleId}>{title}</h2>
          <button className="icon-btn modal-x" onClick={onClose} aria-label="Close dialog" disabled={locked}><Icon name="x" /></button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  );
}

/** Simple confirm; if `typeToConfirm` is given the user must type that word first. */
export function ConfirmDialog({ title, children, confirmLabel, danger, typeToConfirm, busy, onConfirm, onCancel }: {
  title: string; children: ReactNode; confirmLabel: string; danger?: boolean; typeToConfirm?: string; busy?: boolean;
  onConfirm: () => void; onCancel: () => void;
}) {
  const [typed, setTyped] = useState('');
  const ok = !typeToConfirm || typed.trim().toLowerCase() === typeToConfirm.toLowerCase();
  return (
    <Modal title={title} onClose={onCancel} locked={busy}
      footer={<>
        <button className="btn" onClick={onCancel} disabled={busy}>Cancel</button>
        <button className={`btn ${danger ? 'danger' : 'primary'}`} onClick={onConfirm} disabled={!ok || busy}>{busy ? 'Working…' : confirmLabel}</button>
      </>}>
      <div className="stack">
        {children}
        {typeToConfirm && (
          <label className="field">
            <span>Type <strong>{typeToConfirm}</strong> to confirm</span>
            <input data-autofocus value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" onKeyDown={(e) => { if (e.key === 'Enter' && ok && !busy) onConfirm(); }} />
          </label>
        )}
      </div>
    </Modal>
  );
}

/* ---------- toasts ---------- */
export function Toasts() {
  const { toasts, dismissToast } = useApp();
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.kind}`} role={t.kind === 'err' ? 'alert' : 'status'}>
          <Icon name={t.kind === 'err' ? 'warn' : t.kind === 'ok' ? 'check' : 'info'} />
          <span className="grow">{t.text}</span>
          <button className="icon-btn" onClick={() => dismissToast(t.id)} aria-label="Dismiss"><Icon name="x" size={14} /></button>
        </div>
      ))}
    </div>
  );
}

/* ---------- form helpers ---------- */
export function Field({ label, hint, error, children, className = '' }: { label: ReactNode; hint?: ReactNode; error?: string | null; children: ReactNode; className?: string }) {
  return (
    <label className={`field ${error ? 'has-error' : ''} ${className}`}>
      <span>{label}</span>
      {children}
      {error ? <small className="err-text">{error}</small> : hint ? <small className="hint">{hint}</small> : null}
    </label>
  );
}

export function Toggle({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <label className={`toggle${disabled ? ' disabled' : ''}`}>
      <input type="checkbox" role="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span className="track" aria-hidden="true"><span className="thumb" /></span>
      <span className="toggle-label">{label}</span>
    </label>
  );
}

export function Segmented<T extends string>({ value, onChange, options, label }: { value: T; onChange: (v: T) => void; options: { value: T; label: ReactNode; title?: string }[]; label: string }) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" className={value === o.value ? 'on' : ''} aria-pressed={value === o.value} title={o.title} onClick={() => onChange(o.value)}>{o.label}</button>
      ))}
    </div>
  );
}

export function CopyButton({ text, label = 'Copy' }: { text: string; label?: string }) {
  const { toast } = useApp();
  const [done, setDone] = useState(false);
  const copy = async () => {
    try {
      if (navigator.clipboard && window.isSecureContext) await navigator.clipboard.writeText(text);
      else {
        const ta = document.createElement('textarea');
        ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0';
        document.body.appendChild(ta); ta.select(); document.execCommand('copy'); ta.remove();
      }
      setDone(true);
      window.setTimeout(() => setDone(false), 1500);
    } catch { toast('err', 'Could not copy to the clipboard. Select the text and copy it manually.'); }
  };
  return <button type="button" className="btn sm" onClick={copy}><Icon name={done ? 'check' : 'copy'} size={14} />{done ? 'Copied' : label}</button>;
}

export function CodeBlock({ text, label }: { text: string; label?: string }) {
  return (
    <div className="codeblock">
      <div className="codeblock-head"><span>{label}</span><CopyButton text={text} /></div>
      <pre tabIndex={0}>{text}</pre>
    </div>
  );
}

export function Tabs<T extends string>({ value, onChange, tabs }: { value: T; onChange: (v: T) => void; tabs: { value: T; label: string }[] }) {
  return (
    <div className="tabs" role="tablist">
      {tabs.map((t) => (
        <button key={t.value} role="tab" aria-selected={value === t.value} className={value === t.value ? 'on' : ''} onClick={() => onChange(t.value)}>{t.label}</button>
      ))}
    </div>
  );
}

/* ---------- sparkline ---------- */
export function Sparkline({ points, color = 'var(--green)', height = 56, max, min = 0, unit = '', label }: {
  points: { t: number; v: number | null | undefined }[]; color?: string; height?: number; max?: number; min?: number; unit?: string; label: string;
}) {
  const valid = points.filter((p): p is { t: number; v: number } => typeof p.v === 'number' && Number.isFinite(p.v));
  if (valid.length < 2) {
    return <div className="spark-empty" style={{ height }}>No history reported yet</div>;
  }
  const w = 300;
  const t0 = valid[0].t, t1 = valid[valid.length - 1].t || t0 + 1;
  const hi = max ?? Math.max(...valid.map((p) => p.v), min + 1);
  const lo = Math.min(min, ...valid.map((p) => p.v));
  const x = (t: number) => ((t - t0) / Math.max(1, t1 - t0)) * w;
  const y = (v: number) => height - 4 - ((v - lo) / Math.max(1e-9, hi - lo)) * (height - 8);
  const line = valid.map((p, i) => `${i ? 'L' : 'M'}${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`).join(' ');
  const area = `${line} L${w},${height} L0,${height} Z`;
  const last = valid[valid.length - 1].v;
  const gid = `g${label.replace(/\W/g, '')}`;
  return (
    <div className="spark">
      <svg viewBox={`0 0 ${w} ${height}`} preserveAspectRatio="none" role="img" aria-label={`${label}: latest ${last.toFixed(0)}${unit}, range ${Math.min(...valid.map((p) => p.v)).toFixed(0)}–${Math.max(...valid.map((p) => p.v)).toFixed(0)}${unit}`} style={{ height }}>
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={color} stopOpacity="0.35" />
            <stop offset="1" stopColor={color} stopOpacity="0" />
          </linearGradient>
        </defs>
        <path d={area} fill={`url(#${gid})`} />
        <path d={line} fill="none" stroke={color} strokeWidth="1.6" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
      </svg>
      <div className="spark-meta"><span>{label}</span><strong>{last.toFixed(0)}{unit}</strong></div>
    </div>
  );
}

export function Meter({ value, tone }: { value: number | null | undefined; tone?: 'green' | 'amber' | 'red' }) {
  if (value === null || value === undefined || !Number.isFinite(value)) return <div className="meter off" />;
  const v = Math.max(0, Math.min(100, value));
  const t = tone ?? (v > 90 ? 'red' : v > 75 ? 'amber' : 'green');
  return <div className={`meter ${t}`} role="presentation"><span style={{ width: `${v}%` }} /></div>;
}
