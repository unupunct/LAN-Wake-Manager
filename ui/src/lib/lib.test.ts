import { describe, expect, it } from 'vitest';
import { describeRecurrence, describeSchedule } from './recurrence';
import { countdown, formatBytes, formatDuration, normalizeMac, opStateLabel, parsePorts, relativeTime } from './format';
import { isValidCidr, isValidDate, isValidIPv4, isValidMac, isValidPort, isValidTime } from './validate';

describe('describeRecurrence', () => {
  it('daily and once', () => {
    expect(describeRecurrence({ kind: 'daily', at: '07:30' })).toBe('Every day at 07:30');
    expect(describeRecurrence({ kind: 'once', at: '08:00', date: '2026-12-24' })).toBe('Once on 2026-12-24 at 08:00');
  });
  it('weekly variants', () => {
    expect(describeRecurrence({ kind: 'weekly', at: '07:30', weekdays: [1, 2, 3, 4, 5] })).toContain('Every weekday');
    expect(describeRecurrence({ kind: 'weekly', at: '10:00', weekdays: [6, 7] })).toContain('weekend');
    expect(describeRecurrence({ kind: 'weekly', at: '10:00', weekdays: [1, 3] })).toBe('Every Monday and Wednesday at 10:00');
    expect(describeRecurrence({ kind: 'weekly', at: '10:00', weekdays: [2, 3, 4] })).toBe('Every Tuesday to Thursday at 10:00');
    expect(describeRecurrence({ kind: 'weekly', at: '10:00', weekdays: [] })).toContain('no days');
  });
  it('monthly and custom', () => {
    expect(describeRecurrence({ kind: 'monthly', at: '01:00', day_of_month: 1 })).toBe('On the 1st of every month at 01:00');
    expect(describeRecurrence({ kind: 'monthly', at: '01:00', day_of_month: 22 })).toContain('22nd');
    expect(describeRecurrence({ kind: 'monthly', at: '01:00', day_of_month: 31 })).toContain('shorter months');
    expect(describeRecurrence({ kind: 'custom', interval: { every: 6, unit: 'hours' } })).toBe('Every 6 hours');
    expect(describeRecurrence({ kind: 'custom', interval: { every: 1, unit: 'days' } })).toBe('Every day');
  });
  it('handles null', () => {
    expect(describeRecurrence(null)).toBe('No recurrence');
  });
  it('describes range and exceptions', () => {
    const t = describeSchedule({ recurrence: { kind: 'daily', at: '07:00' }, start_date: '2026-01-01', end_date: '2026-12-31', exceptions: ['2026-12-25'] });
    expect(t).toBe('Every day at 07:00, from 2026-01-01 until 2026-12-31, except 2026-12-25');
  });
});

describe('format', () => {
  it('bytes never invent values', () => {
    expect(formatBytes(null)).toBe('not reported');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(16 * 1024 ** 3)).toBe('16.0 GB');
  });
  it('duration', () => {
    expect(formatDuration(null)).toBe('not reported');
    expect(formatDuration(59)).toBe('59s');
    expect(formatDuration(3700)).toBe('1h 1m');
    expect(formatDuration(90000)).toBe('1d 1h');
  });
  it('relative time', () => {
    expect(relativeTime(1000, 1100)).toBe('2 min ago');
    expect(relativeTime(1000 + 7200, 1000)).toBe('in 2 h');
    expect(relativeTime(null)).toBe('never');
  });
  it('countdown', () => {
    expect(countdown(100, 40)).toBe('1:00');
    expect(countdown(100, 200)).toBe('0:00');
  });
  it('mac + ports', () => {
    expect(normalizeMac('aa-bb-cc-dd-ee-ff')).toBe('AA:BB:CC:DD:EE:FF');
    expect(parsePorts('22, 3389 70000 x 22')).toEqual({ ports: [22, 3389], invalid: ['70000', 'x'] });
  });
  it('honest wake state', () => {
    expect(opStateLabel('timeout', 'wake')).toMatch(/not online/);
    expect(opStateLabel('succeeded', 'wake')).toBe('Confirmed online');
  });
});

describe('validate', () => {
  it('mac', () => {
    expect(isValidMac('AA:BB:CC:DD:EE:FF')).toBe(true);
    expect(isValidMac('aa-bb-cc-dd-ee-ff')).toBe(true);
    expect(isValidMac('AABBCCDDEEFF')).toBe(true);
    expect(isValidMac('AA:BB:CC:DD:EE')).toBe(false);
    expect(isValidMac('GG:BB:CC:DD:EE:FF')).toBe(false);
  });
  it('ip/port/cidr/time/date', () => {
    expect(isValidIPv4('192.168.1.20')).toBe(true);
    expect(isValidIPv4('256.1.1.1')).toBe(false);
    expect(isValidIPv4('1.2.3')).toBe(false);
    expect(isValidPort(0)).toBe(false);
    expect(isValidPort(65535)).toBe(true);
    expect(isValidCidr('192.168.1.0/24')).toBe(true);
    expect(isValidCidr('192.168.1.0/33')).toBe(false);
    expect(isValidTime('07:30')).toBe(true);
    expect(isValidTime('7:30')).toBe(false);
    expect(isValidDate('2026-02-30')).toBe(false);
    expect(isValidDate('2026-12-24')).toBe(true);
  });
});
