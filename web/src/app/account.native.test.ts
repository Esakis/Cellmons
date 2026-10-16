import { afterEach, expect, it, vi } from 'vitest';

const vault = vi.hoisted(() => ({
  value: undefined as { token: string; rotatedAt: number } | undefined,
  reads: 0,
  writes: 0,
}));
vi.mock('../platform/session', () => ({
  loadSession: async () => {
    vault.reads++;
    return vault.value && { ...vault.value };
  },
  saveSession: async (token: string, rotatedAt: number) => {
    vault.writes++;
    vault.value = { token, rotatedAt };
  },
  clearSession: async () => {
    vault.value = undefined;
  },
}));
afterEach(() => {
  vi.unstubAllEnvs();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.resetModules();
  vault.value = undefined;
  vault.reads = 0;
  vault.writes = 0;
});
it('times out stalled HTTP headers and bodies and cancels requests when entering guest mode', async () => {
  vi.stubEnv('VITE_PLATFORM', 'web');
  vi.useFakeTimers();
  const account = await import('./account');
  let body = false;
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (_, init) => {
    const stalled = new Promise<Response>((_, reject) => {
      init?.signal?.addEventListener('abort', () => reject(new Error('aborted')), { once: true });
    });
    if (!body) return stalled;
    return { status: 200, ok: true, json: () => stalled } as unknown as Response;
  });
  for (const phase of [false, true]) {
    body = phase;
    const result = expect(account.api('/api/test')).rejects.toMatchObject({
      code: 'SERVER_UNAVAILABLE',
    });
    await vi.advanceTimersByTimeAsync(15000);
    await result;
  }
  const result = expect(account.api('/api/test')).rejects.toMatchObject({
    code: 'SERVER_UNAVAILABLE',
  });
  account.enterGuest();
  await result;
  expect(vi.getTimerCount()).toBe(0);
  account.leaveGuest();
});
it('retains a banned native credential for notices and privacy without unlocking gameplay', async () => {
  vi.stubEnv('VITE_PLATFORM', 'android');
  vi.stubEnv('VITE_API_ORIGIN', 'https://api.example.test');
  vi.resetModules();
  const token = 'c'.repeat(64);
  vault.value = { token, rotatedAt: Date.now() - 8 * 86400000 };
  let rotations = 0;
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
    expect(new Headers(init?.headers).get('authorization')).toBe(`Bearer ${token}`);
    const url = String(input);
    if (url.endsWith('/api/auth/rotate')) rotations++;
    if (url.endsWith('/api/me') || url.endsWith('/api/auth/rotate'))
      return new Response(JSON.stringify({ error: 'UNAUTHENTICATED' }), { status: 401 });
    if (url.endsWith('/api/me/privacy'))
      return new Response(JSON.stringify({ status: 'banned', busy: false }));
    if (url.endsWith('/api/notices')) return new Response(JSON.stringify({ notices: [] }));
    if (url.endsWith('/api/auth/logout')) return new Response(null, { status: 204 });
    throw new Error('Unexpected fixture request');
  });
  const account = await import('./account');
  await account.restoreAccount();
  expect(account.account()).toBeNull();
  expect(vault.value?.token).toBe(token);
  await expect(account.api('/api/notices')).resolves.toEqual({ notices: [] });
  expect(rotations).toBe(1);
  await account.signOut();
  expect(vault.value).toBeUndefined();
});
it('restores an Android session, preserves its rotation clock and never persists guest data or URL credentials', async () => {
  vi.stubEnv('VITE_PLATFORM', 'android');
  vi.stubEnv('VITE_API_ORIGIN', 'https://api.example.test');
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date('2026-10-16T00:00:00Z'));
  vi.resetModules();
  const storage = vi.spyOn(Storage.prototype, 'setItem');
  const profile = {
    id: 'native-user',
    login: 'NativeUser',
    email: 'native@example.test',
    email_verified: true,
    start_school: 'combat',
    role: 'player',
    avatar: null,
    busy: true,
  };
  const first = 'a'.repeat(64),
    second = 'b'.repeat(64);
  let rotations = 0;
  const requests: { url: string; headers: Headers; credentials: RequestCredentials | undefined }[] =
    [];
  const fetcher = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
    const url = String(input),
      headers = new Headers(init?.headers);
    requests.push({ url, headers, credentials: init?.credentials });
    const json = (value: unknown) =>
      new Response(JSON.stringify(value), { headers: { 'Content-Type': 'application/json' } });
    if (url.endsWith('/api/auth/login')) {
      expect(JSON.parse(String(init?.body)).client).toBe('android');
      return json({ profile, token: first });
    }
    if (url.endsWith('/api/auth/rotate')) {
      rotations++;
      expect(headers.get('authorization')).toBe(`Bearer ${first}`);
      return json({ token: second });
    }
    if (url.endsWith('/api/auth/logout')) return new Response(null, { status: 204 });
    if (url.endsWith('/api/me')) return json(profile);
    if (url.endsWith('/api/auth/ws-ticket')) return json({ ticket: 'one-time-test-ticket' });
    return json({ ok: true });
  });
  const original = await import('./account');
  await original.signIn('NativeUser', 'Only a fixture password!');
  expect(vault.value).toEqual({ token: first, rotatedAt: Date.now() });
  const savedAt = vault.value?.rotatedAt;
  vi.setSystemTime(Date.now() + 86400000);
  vi.resetModules();
  const restored = await import('./account');
  await restored.restoreAccount();
  expect(restored.account()?.kind).toBe('account');
  expect(vault.reads).toBe(1);
  expect(rotations).toBe(0);
  expect(vault.value?.rotatedAt).toBe(savedAt);
  const session = await import('./session');
  expect(session.updatesBlocked()).toBe(true);
  expect(session.duelActive()).toBe(false);
  vi.setSystemTime(Date.now() + 7 * 86400000);
  await restored.api('/api/test');
  expect(rotations).toBe(1);
  expect(vault.value).toEqual({ token: second, rotatedAt: Date.now() });
  const url = await restored.websocketURL();
  expect(url.protocol).toBe('wss:');
  expect(url.searchParams.get('ticket')).toBe('one-time-test-ticket');
  expect(url.href).not.toContain(second);
  expect(requests.every((r) => r.credentials === 'omit')).toBe(true);
  expect(requests.some((r) => r.url.includes(first) || r.url.includes(second))).toBe(false);
  expect(
    storage.mock.calls.some((call) =>
      call.some((value) => String(value).includes(first) || String(value).includes(second)),
    ),
  ).toBe(false);
  await restored.signOut();
  expect(vault.value).toBeUndefined();
  fetcher.mockClear();
  const reads = vault.reads,
    writes = vault.writes;
  restored.enterGuest();
  await restored.restoreAccount();
  expect(fetcher).not.toHaveBeenCalled();
  expect(vault.reads).toBe(reads);
  expect(vault.writes).toBe(writes);
  expect(session.updatesBlocked()).toBe(false);
  session.setDuelActive(true);
  expect(session.updatesBlocked()).toBe(true);
  session.setDuelActive(false);
  restored.leaveGuest();
});

it('authenticates a fresh password without rotating a revoked previous native token', async () => {
  vi.stubEnv('VITE_PLATFORM', 'android');
  vi.stubEnv('VITE_API_ORIGIN', 'https://api.example.test');
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date('2026-10-16T00:00:00Z'));
  let rotations = 0;
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
    const url = String(input);
    if (url.endsWith('/api/auth/rotate')) {
      rotations++;
      return new Response(JSON.stringify({ error: 'UNAUTHENTICATED' }), { status: 401 });
    }
    if (url.endsWith('/api/auth/login')) {
      const { identity } = JSON.parse(String(init?.body));
      return new Response(
        JSON.stringify({
          token: (identity === 'Previous' ? 'a' : 'b').repeat(64),
          profile: {
            id: identity,
            login: identity,
            email: 'fixture@example.test',
            email_verified: true,
            start_school: 'combat',
            role: 'player',
            avatar: null,
          },
        }),
      );
    }
    throw new Error('Unexpected fixture request');
  });
  const account = await import('./account');
  await account.signIn('Previous', 'Only a fixture password!');
  vi.setSystemTime(Date.now() + 8 * 86400000);
  await account.signIn('Fresh', 'Another fixture password!');
  expect(rotations).toBe(0);
  expect(account.account()).toMatchObject({ kind: 'account', profile: { login: 'Fresh' } });
  expect(vault.value?.token).toBe('b'.repeat(64));
});
