import { useState, type ReactNode } from 'react';
import { api } from '../api/client';
import type { Action, Device, Operation } from '../api/types';
import { opKindLabel } from '../lib/format';
import { useApp } from '../store';
import { Icon } from './Icon';
import { ConfirmDialog, Field, Modal, StatusBadge } from './ui';

/** Can a wake be attempted at all? */
export function wakeBlockedReason(d: Device): string | null {
  if (!d.mac) return 'No MAC address is known, so Wake-on-LAN is impossible.';
  return null;
}

export function shutdownBlockedReason(d: Device): string | null {
  if (d.power_method === 'none') return 'No power method is configured for this device.';
  if (d.power_method === 'agent' && d.agent?.state !== 'online') return 'The agent is not online, so the device cannot be told to shut down.';
  return null;
}

/** Per-device power actions with confirmation for shutdown / restart. */
export function useDeviceActions() {
  const { toast, toastError, upsertOp, canOperate } = useApp();
  const [pending, setPending] = useState<{ kind: 'shutdown' | 'restart'; device: Device } | null>(null);
  const [busy, setBusy] = useState(false);
  const [delay, setDelay] = useState(0);

  const track = (op: Operation | undefined, text: string) => {
    if (op && op.id) upsertOp(op);
    toast('ok', text);
  };

  const wake = async (d: Device) => {
    try { track(await api.wake(d.id), `Wake requested for ${d.name}. Success is reported only once the device is confirmed online.`); }
    catch (e) { toastError(e, `Wake ${d.name}`); }
  };

  const cancelShutdown = async (d: Device) => {
    try { track(await api.cancelShutdown(d.id), `Cancel requested for ${d.name}.`); }
    catch (e) { toastError(e, `Cancel shutdown ${d.name}`); }
  };

  const ask = (kind: 'shutdown' | 'restart', device: Device) => { setDelay(0); setPending({ kind, device }); };

  const run = async () => {
    if (!pending) return;
    setBusy(true);
    try {
      const op = pending.kind === 'shutdown' ? await api.shutdown(pending.device.id, delay) : await api.restart(pending.device.id, delay);
      track(op, `${opKindLabel(pending.kind)} sent to ${pending.device.name}.`);
      setPending(null);
    } catch (e) { toastError(e, `${opKindLabel(pending.kind)} ${pending.device.name}`); }
    finally { setBusy(false); }
  };

  const dialog = pending ? (
    <ConfirmDialog
      title={`${opKindLabel(pending.kind)} ${pending.device.name}?`}
      confirmLabel={opKindLabel(pending.kind)} danger busy={busy} onConfirm={run} onCancel={() => setPending(null)}>
      <p>
        {pending.kind === 'shutdown'
          ? 'The device will be told to power off. Unsaved work on it may be lost. You can wake it again with Wake-on-LAN if it supports it.'
          : 'The device will be told to restart. Unsaved work on it may be lost.'}
      </p>
      <Field label="Delay (seconds)" hint="0 = immediately. Pending delayed shutdowns can be cancelled from the device.">
        <input type="number" min={0} max={3600} value={delay} onChange={(e) => setDelay(Math.max(0, Number(e.target.value) || 0))} />
      </Field>
    </ConfirmDialog>
  ) : null;

  return { wake, ask, cancelShutdown, dialog, canOperate };
}

/** Compact row of Wake / Shutdown / Restart icon buttons for a device. */
export function DeviceButtons({ d, actions, onDetails, labels }: {
  d: Device; actions: ReturnType<typeof useDeviceActions>; onDetails?: () => void; labels?: boolean;
}) {
  const wakeBlock = wakeBlockedReason(d);
  const offBlock = shutdownBlockedReason(d);
  const can = actions.canOperate;
  const disabledWhy = (why: string | null) => (!can ? 'Your role cannot run power operations.' : why);
  return (
    <div className="row gap-xs dev-buttons" onClick={(e) => e.stopPropagation()}>
      <button className="btn sm primary-soft" disabled={!can || !!wakeBlock || d.status === 'online'} title={disabledWhy(wakeBlock) ?? (d.status === 'online' ? 'Already online' : 'Send Wake-on-LAN')} onClick={() => actions.wake(d)}>
        <Icon name="power" size={14} />{labels && 'Wake'}<span className="sr-only">Wake {d.name}</span>
      </button>
      <button className="btn sm" disabled={!can || !!offBlock} title={disabledWhy(offBlock) ?? 'Shut down'} onClick={() => actions.ask('shutdown', d)}>
        <Icon name="stop" size={14} />{labels && 'Shutdown'}<span className="sr-only">Shut down {d.name}</span>
      </button>
      <button className="btn sm" disabled={!can || !!offBlock} title={disabledWhy(offBlock) ?? 'Restart'} onClick={() => actions.ask('restart', d)}>
        <Icon name="restart" size={14} />{labels && 'Restart'}<span className="sr-only">Restart {d.name}</span>
      </button>
      {onDetails && <button className="btn sm ghost" onClick={onDetails}><Icon name="chevron" size={14} />{labels ? 'Details' : <span className="sr-only">Details of {d.name}</span>}</button>}
    </div>
  );
}

/**
 * Bulk action modal. Lists all affected devices. Shutdown/restart need the user to type the action word;
 * `confirm: true` is sent only after that.
 */
export function BulkDialog({ action, devices, scope, groupId, onClose, onDone }: {
  action: Action; devices: Device[]; scope: string; groupId?: string; onClose: () => void; onDone?: () => void;
}) {
  const { toast, toastError, upsertOp } = useApp();
  const [busy, setBusy] = useState(false);
  const [typed, setTyped] = useState('');
  const dangerous = action !== 'wake';
  const ok = !dangerous || typed.trim().toLowerCase() === action;
  const skipped = devices.filter((d) => (action === 'wake' ? !!wakeBlockedReason(d) || d.status === 'online' : !!shutdownBlockedReason(d) || d.status === 'offline'));
  const targets = devices.filter((d) => !skipped.includes(d));

  const run = async () => {
    setBusy(true);
    try {
      const body = groupId
        ? { action, confirm: dangerous ? true : undefined }
        : { action, device_ids: devices.map((d) => d.id), confirm: dangerous ? true : undefined };
      const res = groupId ? await api.groupAction(groupId, body) : await api.bulk(body);
      (res?.operations ?? []).forEach(upsertOp);
      toast('ok', `${opKindLabel(action)} sent to ${res?.operations?.length ?? targets.length} device(s).`);
      onDone?.();
      onClose();
    } catch (e) { toastError(e, `Bulk ${action}`); }
    finally { setBusy(false); }
  };

  return (
    <Modal title={`${opKindLabel(action)} ${scope}`} onClose={onClose} locked={busy}
      footer={<>
        <button className="btn" onClick={onClose} disabled={busy}>Cancel</button>
        <button className={`btn ${dangerous ? 'danger' : 'primary'}`} disabled={!ok || busy || targets.length === 0} onClick={run}>
          {busy ? 'Sending…' : `${opKindLabel(action)} ${targets.length} device${targets.length === 1 ? '' : 's'}`}
        </button>
      </>}>
      <div className="stack">
        {devices.length === 0 ? <p className="muted">There are no devices in this selection.</p> : (
          <>
            <p>
              {action === 'wake' && 'A Wake-on-LAN packet will be sent to each device below. Devices already online are skipped by the controller.'}
              {action === 'shutdown' && 'Each device below will be told to shut down. Unsaved work on them may be lost.'}
              {action === 'restart' && 'Each device below will be told to restart. Unsaved work on them may be lost.'}
            </p>
            <ul className="affected" aria-label="Affected devices">
              {devices.map((d) => (
                <li key={d.id}>
                  <span className="grow">{d.name}</span>
                  <StatusBadge status={d.status} small />
                  {skipped.includes(d) && <span className="muted small">will be skipped</span>}
                </li>
              ))}
            </ul>
            {dangerous && (
              <Field label={<>Type <strong>{action}</strong> to confirm</>}>
                <input data-autofocus value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" onKeyDown={(e) => { if (e.key === 'Enter' && ok && !busy) void run(); }} />
              </Field>
            )}
          </>
        )}
      </div>
    </Modal>
  );
}

export function RoleNotice({ children }: { children?: ReactNode }) {
  return <div className="notice"><Icon name="info" size={16} /><span>{children ?? 'Your account is read-only. Ask an administrator for operator access to run actions.'}</span></div>;
}
