import type { Device, DeviceInput } from '../api/types';

/** Convert a Device back to the editable input shape (for membership edits etc). */
export function deviceToInput(d: Device, overrides: Partial<DeviceInput> = {}): DeviceInput {
  return {
    location_id: d.location_id, name: d.name, hostname: d.hostname, ip: d.ip, mac: d.mac, os: d.os, device_type: d.device_type,
    group_ids: d.group_ids ?? [], tcp_check_ports: d.tcp_check_ports ?? [], power_method: d.power_method, ssh: d.ssh, notes: d.notes,
    ...overrides,
  };
}
