import type { LiveEvent } from './types';
import { getToken } from './client';

export type LiveState = 'connecting' | 'live' | 'disconnected';

/** WebSocket client for /api/v1/events with first-message auth and automatic reconnect. */
export class EventsClient {
  private ws: WebSocket | null = null;
  private timer: number | null = null;
  private attempt = 0;
  private stopped = false;
  private authed = false;

  constructor(
    private onEvent: (e: LiveEvent) => void,
    private onState: (s: LiveState, reconnected: boolean) => void,
  ) {}

  static url(): string {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${proto}//${location.host}/api/v1/events`;
  }

  start() {
    this.stopped = false;
    this.connect(false);
  }

  stop() {
    this.stopped = true;
    if (this.timer) window.clearTimeout(this.timer);
    this.timer = null;
    if (this.ws) { this.ws.onclose = null; this.ws.close(); }
    this.ws = null;
  }

  private connect(reconnect: boolean) {
    if (this.stopped) return;
    this.authed = false;
    this.onState('connecting', false);
    let ws: WebSocket;
    try { ws = new WebSocket(EventsClient.url()); } catch { this.schedule(); return; }
    this.ws = ws;
    ws.onopen = () => {
      const token = getToken();
      if (!token) { ws.close(); return; }
      ws.send(JSON.stringify({ type: 'auth', token }));
    };
    ws.onmessage = (m) => {
      let ev: LiveEvent;
      try { ev = JSON.parse(String(m.data)) as LiveEvent; } catch { return; }
      if (!ev || typeof ev.type !== 'string') return;
      if (ev.type === 'hello') {
        this.authed = true;
        this.attempt = 0;
        this.onState('live', reconnect || this.hadConnection);
        this.hadConnection = true;
      }
      this.onEvent(ev);
    };
    ws.onclose = () => {
      this.ws = null;
      if (this.stopped) return;
      this.onState('disconnected', false);
      this.schedule();
    };
    ws.onerror = () => { /* onclose follows */ };
  }

  private hadConnection = false;

  private schedule() {
    if (this.stopped) return;
    this.attempt += 1;
    const delay = Math.min(15000, 1000 * 2 ** Math.min(this.attempt, 4));
    this.timer = window.setTimeout(() => this.connect(true), delay);
  }

  get isAuthed() { return this.authed; }
}
