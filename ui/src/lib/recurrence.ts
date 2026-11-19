import type { Recurrence, ScheduleInput } from '../api/types';

const DAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
export const SHORT_DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

function ordinal(n: number): string {
  const v = n % 100;
  if (v >= 11 && v <= 13) return `${n}th`;
  switch (n % 10) {
    case 1: return `${n}st`;
    case 2: return `${n}nd`;
    case 3: return `${n}rd`;
    default: return `${n}th`;
  }
}

function joinList(items: string[]): string {
  if (items.length <= 1) return items.join('');
  return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
}

/** Human readable description of a recurrence, e.g. "Every Monday to Friday at 07:30". */
export function describeRecurrence(r: Recurrence | null | undefined): string {
  if (!r) return 'No recurrence';
  const at = r.at ? ` at ${r.at}` : '';
  switch (r.kind) {
    case 'once':
      return r.date ? `Once on ${r.date}${at}` : `Once${at}`;
    case 'daily':
      return `Every day${at}`;
    case 'weekly': {
      const days = [...new Set(r.weekdays ?? [])].filter((d) => d >= 1 && d <= 7).sort((a, b) => a - b);
      if (days.length === 0) return `Weekly${at} (no days selected)`;
      if (days.length === 7) return `Every day${at}`;
      if (days.length === 5 && days.join() === '1,2,3,4,5') return `Every weekday (Monday to Friday)${at}`;
      if (days.length === 2 && days.join() === '6,7') return `Every weekend (Saturday and Sunday)${at}`;
      const contiguous = days.length > 2 && days.every((d, i) => i === 0 || d === days[i - 1] + 1);
      if (contiguous) return `Every ${DAY_NAMES[days[0] - 1]} to ${DAY_NAMES[days[days.length - 1] - 1]}${at}`;
      return `Every ${joinList(days.map((d) => DAY_NAMES[d - 1]))}${at}`;
    }
    case 'monthly': {
      const d = r.day_of_month ?? 1;
      return `On the ${ordinal(d)} of every month${at}${d > 28 ? ' (shorter months use the last day)' : ''}`;
    }
    case 'custom': {
      const iv = r.interval;
      if (!iv || !iv.every) return 'Custom interval (not set)';
      const unit = iv.every === 1 ? iv.unit.replace(/s$/, '') : iv.unit;
      return `Every ${iv.every === 1 ? '' : `${iv.every} `}${unit}${r.at ? ` starting at ${r.at}` : ''}`;
    }
    default:
      return 'Unknown recurrence';
  }
}

export function describeSchedule(s: Pick<ScheduleInput, 'recurrence' | 'start_date' | 'end_date' | 'exceptions'>): string {
  let text = describeRecurrence(s.recurrence);
  if (s.start_date && s.end_date) text += `, from ${s.start_date} until ${s.end_date}`;
  else if (s.start_date) text += `, starting ${s.start_date}`;
  else if (s.end_date) text += `, until ${s.end_date}`;
  if (s.exceptions && s.exceptions.length) text += `, except ${joinList(s.exceptions)}`;
  return text;
}

export function emptyRecurrence(kind: Recurrence['kind'] = 'daily'): Recurrence {
  switch (kind) {
    case 'once': return { kind, at: '07:30', date: new Date(Date.now() + 86400000).toISOString().slice(0, 10) };
    case 'daily': return { kind, at: '07:30' };
    case 'weekly': return { kind, at: '07:30', weekdays: [1, 2, 3, 4, 5] };
    case 'monthly': return { kind, at: '07:30', day_of_month: 1 };
    case 'custom': return { kind, at: null, interval: { every: 6, unit: 'hours' } };
  }
}
