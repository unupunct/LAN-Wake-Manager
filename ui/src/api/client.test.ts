import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, api, buildQuery, friendlyMessage, parseError, setUnauthorizedHandler } from './client';

function mockFetch(status: number, body: unknown, headers: Record<string, string> = {}) {
  const text = typeof body === 'string' ? body : JSON.stringify(body);
  vi.stubGlobal('fetch', vi.fn(async () => new Response(status === 204 ? null : text, { status, headers })));
}

afterEach(() => {
  vi.unstubAllGlobals();
  setUnauthorizedHandler(null);
});

describe('api client', () => {
  it('builds queries skipping empty values', () => {
    expect(buildQuery({ a: 1, b: '', c: undefined, d: 'x y' })).toBe('?a=1&d=x%20y');
    expect(buildQuery()).toBe('');
  });

  it('parses error envelopes', async () => {
    const res = new Response(JSON.stringify({ error: { code: 'not_found', message: 'device not found' } }), { status: 404 });
    const e = await parseError(res);
    expect(e.code).toBe('not_found');
    expect(e.message).toBe('device not found');
  });

  it('tolerates html error bodies', async () => {
    const e = await parseError(new Response('<html>bad gateway</html>', { status: 502 }));
    expect(e.message).toMatch(/internal error/);
  });

  it('calls the unauthorized handler on 401', async () => {
    mockFetch(401, { error: { code: 'unauthorized', message: 'no' } });
    const h = vi.fn();
    setUnauthorizedHandler(h);
    await expect(api.me()).rejects.toBeInstanceOf(ApiError);
    expect(h).toHaveBeenCalledTimes(1);
  });

  it('does not trigger unauthorized handler on failed login', async () => {
    mockFetch(401, { error: { code: 'unauthorized', message: 'bad password' } });
    const h = vi.fn();
    setUnauthorizedHandler(h);
    await expect(api.login('x')).rejects.toMatchObject({ code: 'unauthorized' });
    expect(h).not.toHaveBeenCalled();
  });

  it('maps network failure', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => { throw new TypeError('failed'); }));
    await expect(api.info()).rejects.toMatchObject({ code: 'network' });
  });

  it('reads Retry-After for 429', async () => {
    mockFetch(429, { error: { code: 'rate_limited', message: 'slow' } }, { 'Retry-After': '30' });
    try { await api.login('x'); } catch (e) { expect(friendlyMessage(e)).toContain('30 seconds'); }
  });

  it('returns undefined for 204 and sends JSON body', async () => {
    mockFetch(204, '');
    await expect(api.deleteDevice('a/b')).resolves.toBeUndefined();
    const f = fetch as unknown as ReturnType<typeof vi.fn>;
    expect(f.mock.calls[0][0]).toBe('/api/v1/devices/a%2Fb');
  });
});
