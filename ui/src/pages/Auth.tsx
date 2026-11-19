import { useState, type FormEvent } from 'react';
import { ApiError, api, friendlyMessage, setToken } from '../api/client';
import { Icon } from '../components/Icon';
import { Spinner, Toasts } from '../components/ui';
import { useApp } from '../store';

function Frame({ children }: { children: React.ReactNode }) {
  const { info } = useApp();
  return (
    <div className="auth-wrap">
      <div className="auth-card">
        <div className="brand big">
          <span className="brand-mark"><Icon name="bolt" size={24} /></span>
          <div><strong>LAN Wake Manager</strong><small>{info?.name ?? 'Controller'}</small></div>
        </div>
        {children}
      </div>
      <Toasts />
    </div>
  );
}

export function Boot() {
  return <div className="auth-wrap"><Spinner label="Connecting to the controller" /></div>;
}

export function Unreachable() {
  const { retryBoot } = useApp();
  return (
    <Frame>
      <h1>Controller not reachable</h1>
      <p className="muted">The dashboard could not contact the controller at this address. Check that the controller is running, then try again.</p>
      <button className="btn primary" onClick={retryBoot}>Try again</button>
    </Frame>
  );
}

export function Setup() {
  const { signIn } = useApp();
  const [pw, setPw] = useState('');
  const [pw2, setPw2] = useState('');
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const tooShort = pw.length > 0 && pw.length < 10;
  const mismatch = pw2.length > 0 && pw !== pw2;
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (pw.length < 10 || pw !== pw2) return;
    setBusy(true); setErr(null);
    try {
      const r = await api.setup(pw);
      await signIn(r.token);
    } catch (e2) { setToken(null); setErr(friendlyMessage(e2)); }
    finally { setBusy(false); }
  };
  return (
    <Frame>
      <h1>Set admin password</h1>
      <p className="muted">Welcome. Choose the administrator password for this controller. It protects dashboard access, so use at least 10 characters. There is no recovery by e-mail.</p>
      <form className="stack" onSubmit={submit}>
        <label className="field"><span>New password</span>
          <input type="password" autoComplete="new-password" value={pw} onChange={(e) => setPw(e.target.value)} autoFocus />
          {tooShort && <small className="err-text">At least 10 characters ({pw.length}/10).</small>}
        </label>
        <label className="field"><span>Repeat password</span>
          <input type="password" autoComplete="new-password" value={pw2} onChange={(e) => setPw2(e.target.value)} />
          {mismatch && <small className="err-text">The passwords do not match.</small>}
        </label>
        {err && <div className="errorbox" role="alert"><Icon name="warn" /><div>{err}</div></div>}
        <button className="btn primary lg" disabled={busy || pw.length < 10 || pw !== pw2}>{busy ? 'Saving…' : 'Set password and continue'}</button>
        <small className="hint">First-time setup is only possible from the computer that runs the controller.</small>
      </form>
    </Frame>
  );
}

export function Login() {
  const { signIn } = useApp();
  const [pw, setPw] = useState('');
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!pw) return;
    setBusy(true); setErr(null);
    try {
      const r = await api.login(pw);
      await signIn(r.token);
    } catch (e2) { setToken(null); setErr(e2 instanceof ApiError && e2.code === 'unauthorized' ? 'Incorrect password. Please try again.' : friendlyMessage(e2)); }
    finally { setBusy(false); }
  };
  return (
    <Frame>
      <h1>Sign in</h1>
      <p className="muted">Enter the administrator password to manage your devices.</p>
      <form className="stack" onSubmit={submit}>
        <label className="field"><span>Password</span>
          <input type="password" autoComplete="current-password" value={pw} onChange={(e) => setPw(e.target.value)} autoFocus />
        </label>
        {err && <div className="errorbox" role="alert"><Icon name="warn" /><div>{err}</div></div>}
        <button className="btn primary lg" disabled={busy || !pw}>{busy ? 'Signing in…' : 'Sign in'}</button>
      </form>
    </Frame>
  );
}
