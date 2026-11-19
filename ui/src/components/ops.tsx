import { useState } from 'react';
import type { Operation } from '../api/types';
import { formatClock, formatDuration, opKindLabel, relativeTime } from '../lib/format';
import { Icon } from './Icon';
import { OpBadge } from './ui';

const KIND_ICON: Record<string, string> = { wake: 'power', shutdown: 'stop', restart: 'restart', cancel_shutdown: 'x' };

export function OpRow({ op, expandable = true, showDevice = true }: { op: Operation; expandable?: boolean; showDevice?: boolean }) {
  const [open, setOpen] = useState(false);
  const steps = op.steps ?? [];
  const dur = op.finished_at && op.started_at ? op.finished_at - op.started_at : null;
  const by = op.requested_by ? ` · by ${op.requested_by}` : '';
  return (
    <div className="op-block">
      <div className="op-row">
        <div className={`kind-ico kind-${op.kind}`}><Icon name={KIND_ICON[op.kind] ?? 'bolt'} size={16} /></div>
        <div className="what">
          <strong>{opKindLabel(op.kind)}{showDevice ? ` · ${op.device_name}` : ''}</strong>
          <span title={op.message ?? ''}>{op.message || (op.state === 'running' || op.state === 'pending' ? 'In progress…' : 'No details reported')}</span>
          <span className="faint">{relativeTime(op.started_at)}{dur !== null ? ` · took ${formatDuration(dur)}` : ''}{by}{op.source === 'schedule' ? ' · scheduled' : op.source === 'bulk' ? ' · bulk' : ''}</span>
        </div>
        <OpBadge state={op.state} kind={op.kind} />
        {expandable && steps.length > 0 && (
          <button className="icon-btn" onClick={() => setOpen((o) => !o)} aria-expanded={open} aria-label={open ? 'Hide steps' : 'Show steps'} title="Steps">
            <Icon name="chevron" size={16} className={open ? 'rot' : ''} />
          </button>
        )}
      </div>
      {open && (
        <ol className="steps" aria-label="Operation steps">
          {steps.map((s, i) => <li key={i}><time>{formatClock(s.t)}</time><span>{s.text}</span></li>)}
        </ol>
      )}
    </div>
  );
}
