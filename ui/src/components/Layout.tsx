import { useEffect, useState } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { useApp } from '../store';
import { Icon } from './Icon';
import { Toasts } from './ui';

const NAV: { to: string; label: string; icon: string; admin?: boolean; group?: string }[] = [
  { to: '/', label: 'Dashboard', icon: 'dashboard', group: 'Overview' },
  { to: '/devices', label: 'Devices', icon: 'devices' },
  { to: '/groups', label: 'Groups', icon: 'groups' },
  { to: '/discovery', label: 'Discovery', icon: 'discover' },
  { to: '/schedules', label: 'Schedules', icon: 'schedule', group: 'Automation' },
  { to: '/history', label: 'Operations', icon: 'history' },
  { to: '/locations', label: 'Locations', icon: 'location', group: 'Network' },
  { to: '/mobile', label: 'Mobile Access', icon: 'mobile' },
  { to: '/remote', label: 'Remote & Diagnostics', icon: 'vpn' },
  { to: '/backup', label: 'Backup & Restore', icon: 'backup', group: 'System' },
  { to: '/audit', label: 'Audit Log', icon: 'audit', admin: true },
  { to: '/settings', label: 'Settings', icon: 'settings' },
];

export function Layout() {
  const { info, me, isAdmin, locations, locationId, setLocationId, live, theme, toggleTheme, signOut, loadError, reload } = useApp();
  const [open, setOpen] = useState(false);
  const loc = useLocation();
  useEffect(() => { setOpen(false); }, [loc.pathname]);
  const liveText = live === 'live' ? 'Live' : live === 'connecting' ? 'Connecting' : 'Disconnected';

  return (
    <div className={`shell${open ? ' nav-open' : ''}`}>
      <a href="#main" className="skip">Skip to content</a>
      <aside className="sidebar" aria-label="Main navigation">
        <div className="brand">
          <span className="brand-mark"><Icon name="bolt" size={20} /></span>
          <div>
            <strong>LAN Wake Manager</strong>
            <small>{info?.name ?? 'Controller'}</small>
          </div>
        </div>
        <nav>
          {NAV.filter((n) => !n.admin || isAdmin).map((n) => (
            <div key={n.to}>
              {n.group && <div className="nav-group">{n.group}</div>}
              <NavLink to={n.to} end={n.to === '/'} className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
                <Icon name={n.icon} /><span>{n.label}</span>
              </NavLink>
            </div>
          ))}
        </nav>
        <div className="side-foot">
          <span>v{info?.version ?? '?'}</span>
          <span className="muted">API {info?.api_version ?? 1}</span>
        </div>
      </aside>
      {open && <div className="scrim" onClick={() => setOpen(false)} aria-hidden="true" />}

      <div className="main-col">
        <header className="topbar">
          <button className="icon-btn menu-btn" onClick={() => setOpen((o) => !o)} aria-label="Toggle navigation" aria-expanded={open}><Icon name="menu" /></button>
          <label className="loc-select">
            <Icon name="location" size={16} />
            <span className="sr-only">Network location</span>
            <select value={locationId} onChange={(e) => setLocationId(e.target.value)} aria-label="Network location">
              <option value="">All locations</option>
              {locations.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
            </select>
          </label>
          <div className="grow" />
          <span className={`live live-${live}`} role="status" title={live === 'live' ? 'Receiving live updates' : 'Live updates are not connected; data refreshes periodically'}>
            <span className="dot" aria-hidden="true" /><span className="live-text">{liveText}</span>
          </span>
          <button className="icon-btn" onClick={toggleTheme} aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} theme`} title="Toggle theme"><Icon name={theme === 'dark' ? 'sun' : 'moon'} /></button>
          <div className="user">
            <span className="user-name">{me?.name ?? 'admin'}<small>{me?.role}</small></span>
            <button className="btn sm" onClick={() => void signOut()}><Icon name="logout" size={15} /><span className="hide-s">Log out</span></button>
          </div>
        </header>
        <main id="main" className="content" tabIndex={-1}>
          {loadError && (
            <div className="errorbox" role="alert"><Icon name="warn" /><div className="grow">{loadError}</div><button className="btn sm" onClick={() => void reload()}>Retry</button></div>
          )}
          <Outlet />
        </main>
      </div>
      <Toasts />
    </div>
  );
}
