import { useRef, useState } from 'react';
import { api } from '../api/client';
import type { BackupEntry } from '../api/types';
import { Icon } from '../components/Icon';
import { ConfirmDialog, Empty, ErrorBox, PageHeader, Panel, Pill, Spinner, useLoad } from '../components/ui';
import { downloadBlob, formatBytes, formatTime, relativeTime } from '../lib/format';
import { useApp } from '../store';

export function BackupPage() {
  const { isAdmin, toast, toastError, reload: reloadApp } = useApp();
  const { data, error, loading, reload } = useLoad(() => api.backups(), []);
  const [busy, setBusy] = useState<string | null>(null);
  const [restore, setRestore] = useState<BackupEntry | null>(null);
  const [importDoc, setImportDoc] = useState<{ name: string; doc: unknown; summary: string } | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const create = async () => {
    setBusy('create');
    try { const b = await api.createBackup(); toast('ok', `Backup ${b?.name ?? ''} created.`); await reload(true); }
    catch (e) { toastError(e, 'Create backup'); }
    finally { setBusy(null); }
  };
  const doRestore = async () => {
    if (!restore) return;
    setBusy('restore');
    try { await api.restoreBackup(restore.name); toast('ok', 'Backup restored. Data was reloaded.'); setRestore(null); await reload(true); await reloadApp(); }
    catch (e) { toastError(e, 'Restore'); }
    finally { setBusy(null); }
  };
  const doExport = async () => {
    setBusy('export');
    try { const blob = await api.exportConfig(); downloadBlob(blob, `lan-wake-manager-config-${new Date().toISOString().slice(0, 10)}.json`); toast('ok', 'Configuration exported. It contains no passwords, keys or tokens.'); }
    catch (e) { toastError(e, 'Export'); }
    finally { setBusy(null); }
  };
  const pick = async (file: File | undefined) => {
    if (!file) return;
    try {
      const text = await file.text();
      const doc = JSON.parse(text) as Record<string, unknown>;
      if (!doc || typeof doc !== 'object' || Array.isArray(doc)) throw new Error('not an object');
      const n = (k: string) => (Array.isArray(doc[k]) ? (doc[k] as unknown[]).length : 0);
      setImportDoc({ name: file.name, doc, summary: `${n('locations')} locations, ${n('devices')} devices, ${n('groups')} groups, ${n('schedules')} schedules` });
    } catch { toast('err', 'That file is not a valid configuration export (it must be a JSON file created by “Export configuration”).'); }
    finally { if (fileRef.current) fileRef.current.value = ''; }
  };
  const doImport = async () => {
    if (!importDoc) return;
    setBusy('import');
    try { await api.importConfig(importDoc.doc); toast('ok', 'Configuration imported and merged.'); setImportDoc(null); await reloadApp(); await reload(true); }
    catch (e) { toastError(e, 'Import'); }
    finally { setBusy(null); }
  };

  const list = data ?? [];
  return (
    <>
      <PageHeader title="Backup & restore" subtitle="Backups are full snapshots of the controller database. Exports are portable configuration files without any secrets.">
        <button className="btn primary" onClick={create} disabled={!isAdmin || busy === 'create'}><Icon name="backup" size={16} />{busy === 'create' ? 'Creating…' : 'Create backup now'}</button>
      </PageHeader>
      <Panel title="Backups">
        {loading && !data ? <Spinner /> : error && !data ? <ErrorBox message={error} onRetry={() => void reload()} /> : list.length === 0 ? (
          <Empty icon="backup" title="No backups yet">Create one now, or enable automatic backups in Settings.</Empty>
        ) : (
          <div className="table-wrap"><table className="tbl stackm">
            <thead><tr><th>Name</th><th>Created</th><th>Size</th><th>Type</th><th aria-label="Actions" /></tr></thead>
            <tbody>{list.map((b) => (
              <tr key={b.name}><td className="mono">{b.name}</td><td>{formatTime(b.created_at)} <span className="muted small">({relativeTime(b.created_at)})</span></td><td>{formatBytes(b.size_bytes)}</td><td><Pill tone={b.kind === 'auto' ? 'muted' : 'cyan'}>{b.kind}</Pill></td>
                <td className="actions"><button className="btn sm" disabled={!isAdmin} onClick={() => setRestore(b)}><Icon name="restart" size={13} />Restore</button></td></tr>
            ))}</tbody>
          </table></div>
        )}
      </Panel>
      <div className="grid cols-2">
        <Panel title="Export configuration">
          <div className="stack"><p className="muted">Download devices, groups, locations, relay configuration (without tokens), schedules and settings as a JSON file. Passwords, keys and tokens are never included.</p>
            <div><button className="btn" onClick={doExport} disabled={!isAdmin || busy === 'export'}><Icon name="download" size={15} />{busy === 'export' ? 'Preparing…' : 'Export configuration'}</button></div></div>
        </Panel>
        <Panel title="Import configuration">
          <div className="stack"><p className="muted">Merge a previously exported file into this controller. Items with the same ID are updated; nothing is deleted.</p>
            <div><input ref={fileRef} type="file" accept="application/json,.json" hidden onChange={(e) => void pick(e.target.files?.[0])} />
              <button className="btn" onClick={() => fileRef.current?.click()} disabled={!isAdmin || busy === 'import'}><Icon name="upload" size={15} />Choose file…</button></div></div>
        </Panel>
      </div>
      {restore && (
        <ConfirmDialog title="Restore this backup?" confirmLabel="Restore backup" danger typeToConfirm="restore" busy={busy === 'restore'} onConfirm={doRestore} onCancel={() => setRestore(null)}>
          <p>The controller’s current data (devices, schedules, settings, history) is <strong>replaced</strong> by the snapshot <span className="mono">{restore.name}</span> from {formatTime(restore.created_at)}. Anything changed since then is lost. Consider creating a fresh backup first.</p>
        </ConfirmDialog>
      )}
      {importDoc && (
        <ConfirmDialog title="Import this configuration?" confirmLabel="Import and merge" busy={busy === 'import'} onConfirm={doImport} onCancel={() => setImportDoc(null)}>
          <p>File <span className="mono">{importDoc.name}</span> contains {importDoc.summary}. Existing items with the same ID will be overwritten by the file’s version.</p>
        </ConfirmDialog>
      )}
    </>
  );
}
