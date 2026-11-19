# LAN Wake Manager - dashboard UI

React + TypeScript + Vite single-page app. Talks only to same-origin `/api/v1` (contract: `../docs/API.md`), hash routing, no CDN or external fonts. Served by the Rust controller and shown in the Tauri window.

## Commands
- `npm install`
- `npm run dev` - Vite on :5173, proxies `/api` (incl. WebSocket) to http://127.0.0.1:8480
- `npm run build` - typecheck + build to `dist/` (relative asset URLs)
- `npm run typecheck`, `npm test` (vitest: recurrence text, formatting, validation, API client errors)
- `npm run mock` - mock controller on :8480 (password `demo-password-123`; `MOCK_SETUP=1` for first-run flow; `PORT=` to change), serves `dist/`
- `npm run shots -- 1440 900 desktop dark` - Edge/puppeteer-core screenshots of every page into `../docs/screenshots/ui-scratch/`

## Layout
- `src/api` - `types.ts` (API.md types), `client.ts` (fetch wrapper, ApiError, 401 handler), `events.ts` (WebSocket with reconnect)
- `src/store.tsx` - auth, shared data (locations/devices/groups/ops), live events, toasts, theme, location filter
- `src/pages` - one file per page; `src/components` - shared UI; `src/lib` - pure logic (format, recurrence, validate)
- `dev/` - mock server and screenshot scripts
