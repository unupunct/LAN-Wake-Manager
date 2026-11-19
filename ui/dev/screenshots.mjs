// Drives Edge headless against the mock server and screenshots every page.
// Usage: node dev/screenshots.mjs [width=1440] [height=900] [suffix=desktop] [theme=dark]
import puppeteer from 'puppeteer-core';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const BASE = process.env.BASE || 'http://127.0.0.1:8480';
const [W = '1440', H = '900', suffix = 'desktop', theme = 'dark'] = process.argv.slice(2);
const OUT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'docs', 'screenshots', 'ui-scratch');
fs.mkdirSync(OUT, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const browser = await puppeteer.launch({ executablePath: EDGE, headless: true, args: ['--no-sandbox'] });
const page = await browser.newPage();
await page.setViewport({ width: Number(W), height: Number(H), deviceScaleFactor: 1 });
const problems = [];
page.on('console', (m) => { if (['error', 'warning'].includes(m.type())) problems.push(`[console.${m.type()}] ${m.text()}`); });
page.on('pageerror', (e) => problems.push(`[pageerror] ${e.message}`));
page.on('response', (r) => { if (r.status() >= 400 && r.url().includes('/api/')) problems.push(`[http ${r.status()}] ${r.request().method()} ${r.url()}`); });

const shot = async (name) => { await sleep(500); await page.screenshot({ path: path.join(OUT, `${name}-${suffix}.png`) }); };
const go = async (hash) => { await page.goto(`${BASE}/#${hash}`); await sleep(700); };
const clickText = async (sel, text) => {
  const ok = await page.evaluate((s, t) => { const el = [...document.querySelectorAll(s)].find((e) => e.textContent.trim().includes(t) && !e.disabled); if (el) { el.click(); return true; } return false; }, sel, text);
  if (!ok) problems.push(`[script] could not click ${sel} "${text}"`);
  await sleep(500);
  return ok;
};
const closeModal = async () => { await page.keyboard.press('Escape'); await sleep(300); };

// login
await page.goto(BASE);
await page.evaluate((t) => { localStorage.clear(); localStorage.setItem('lwm.theme', JSON.stringify(t)); }, theme);
await page.goto(BASE);
await page.waitForSelector('input[type=password]');
await shot('00-login');
await page.type('input[type=password]', 'demo-password-123');
await page.keyboard.press('Enter');
await page.waitForSelector('.sidebar');
await sleep(1200);
await shot('01-dashboard');

await go('/devices'); await shot('02-devices-grid');
await clickText('.segmented button', 'Table'); await shot('02-devices-table');
await clickText('.segmented button', 'List'); await shot('02-devices-list');
await clickText('.segmented button', 'Cards');

await go('/devices/d1'); await shot('03-device-drawer-overview');
for (const [tab, name] of [['System', 'system'], ['History', 'history'], ['Agent', 'agent'], ['Troubleshoot', 'trouble']]) {
  await clickText('[role=tab]', tab);
  if (name === 'agent') { await clickText('button', 'Re-enroll'); await sleep(500); }
  if (name === 'trouble') { await clickText('button', 'Run troubleshooting'); await sleep(600); }
  await shot(`03-device-${name}`);
}
await go('/devices/d2'); await clickText('[role=tab]', 'System'); await shot('03-device-nosysinfo');
await go('/devices/new'); await shot('04-device-form');
await clickText('button', 'Add device'); await shot('04-device-form-errors');
await go('/devices/d3/edit'); await shot('04-device-edit-ssh');

await go('/discovery'); await sleep(500);
await clickText('button', 'Start scan'); await sleep(1800); await shot('05-discovery-running');
await sleep(5000); await shot('05-discovery-done');

await go('/groups'); await shot('06-groups');
await clickText('button', 'Shutdown'); await shot('06-groups-confirm'); await closeModal();
await clickText('button', 'New group'); await shot('06-group-editor'); await closeModal();

await go('/locations'); await shot('07-locations');
const relayBtns = await page.$$('button.accent'); if (relayBtns[0]) { await relayBtns[0].click(); await sleep(400); await shot('07-relay-builtin'); await closeModal(); }
await clickText('button', 'Edit'); await shot('07-location-edit'); await closeModal();

await go('/schedules'); await shot('08-schedules');
await clickText('button', 'New schedule'); await shot('08-schedule-editor'); await closeModal();

await go('/history'); await shot('09-history');
await go('/mobile'); await clickText('button', 'Add mobile device'); await clickText('button', 'Show pairing QR'); await sleep(500); await shot('10-mobile-qr'); await closeModal(); await closeModal(); await shot('10-mobile');
await go('/remote'); await sleep(500); await clickText('button', 'Run diagnostics'); await sleep(600); await shot('11-remote');
await go('/backup'); await shot('12-backup');
await go('/audit'); await shot('13-audit');
await go('/settings'); await shot('14-settings');

// dashboard bulk confirm
await go('/'); await clickText('button', 'Shutdown all'); await shot('01-dashboard-bulk-confirm'); await closeModal();

// live wake flow
await go('/devices'); await sleep(500);
await clickText('.dev-card button[title="Send Wake-on-LAN"]', ''); await sleep(1500); await shot('02-devices-waking');
await sleep(5500); await shot('02-devices-after-wake');

console.log(problems.length ? `PROBLEMS (${problems.length}):\n${[...new Set(problems)].join('\n')}` : 'No console errors or failed requests.');
await browser.close();
