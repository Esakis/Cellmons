import { createSignal } from 'solid-js';
import { useEphemeralPreferences } from './preferences';
import { setActivityBusy, setSessionKnown } from './session';

export type Profile = {
  id: string;
  login: string;
  email: string;
  email_verified: boolean;
  start_school: string;
  role: string;
  busy?: boolean;
  avatar: {
    id: string;
    template_id: number;
    path_id?: number | null;
    tier?: number;
    level: number;
    xp: number;
  } | null;
};
export type Account = { kind: 'guest' } | { kind: 'account'; profile: Profile } | null;
export const [account, setAccount] = createSignal<Account>(null);
let generation = 0;
export const accountEpoch = () => generation;
let controller = new AbortController();
let appToken: string | undefined;
let rotatedAt = 0;
let rotation: Promise<void> | undefined;
let restoredNative = false;
let restoringNative: Promise<void> | undefined;
const android = import.meta.env.VITE_PLATFORM === 'android';
const apiOrigin = android ? (import.meta.env.VITE_API_ORIGIN ?? '') : '';
// Keep the platform boundary directly on the import so browser builds omit the native chunk.
const nativeStore =
  import.meta.env.VITE_PLATFORM === 'android'
    ? () => import('../platform/session')
    : (): never => {
        throw new Error('Native session storage unavailable');
      };

export function enterGuest() {
  controller.abort();
  controller = new AbortController();
  generation++;
  appToken = undefined;
  useEphemeralPreferences(true);
  setAccount({ kind: 'guest' });
  setActivityBusy(false);
  setSessionKnown(true);
}

export function leaveGuest() {
  generation++;
  useEphemeralPreferences(false);
  setAccount(null);
  setSessionKnown(false);
}

export class ApiError extends Error {
  constructor(
    public code: string,
    public retryAfter = 0,
  ) {
    super(code);
  }
}

async function request<T>(
  path: string,
  body?: unknown,
  method = body === undefined ? 'GET' : 'POST',
): Promise<T> {
  if (account()?.kind === 'guest') throw new ApiError('GUEST_LOCAL');
  if (android && !apiOrigin) throw new ApiError('SERVER_UNAVAILABLE');
  const headers: Record<string, string> = { 'X-Requested-With': 'app' };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  if (android) {
    headers['X-Native-Version'] = import.meta.env.VITE_NATIVE_VERSION ?? '1.0.0';
    if (appToken) headers.Authorization = `Bearer ${appToken}`;
  }
  const source = controller.signal;
  const local = new AbortController();
  const abort = () => local.abort();
  source.addEventListener('abort', abort, { once: true });
  if (source.aborted) abort();
  const timer = window.setTimeout(abort, 15000);
  try {
    const response = await fetch(`${apiOrigin}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: android ? 'omit' : 'same-origin',
      cache: 'no-store',
      signal: local.signal,
    });
    if (response.status === 204) return undefined as T;
    const result = await response.json().catch(() => null);
    if (android && result?.error === 'VERSION_MISMATCH')
      window.dispatchEvent(new Event('game-version-mismatch'));
    if (!response.ok)
      throw new ApiError(
        result?.error ?? 'SERVER_UNAVAILABLE',
        Number(response.headers.get('Retry-After')) || 0,
      );
    if (!result) throw new ApiError('SERVER_UNAVAILABLE');
    return result as T;
  } catch (error) {
    throw error instanceof ApiError ? error : new ApiError('SERVER_UNAVAILABLE');
  } finally {
    clearTimeout(timer);
    source.removeEventListener('abort', abort);
  }
}

export async function api<T>(path: string, body?: unknown, method?: string): Promise<T> {
  const epoch = generation;
  const rights =
    path === '/api/me/privacy' ||
    path === '/api/me/export' ||
    path === '/api/notices' ||
    path === '/api/auth/logout' ||
    path === '/api/auth/deletion-status' ||
    path === '/api/auth/cancel-deletion' ||
    path === '/api/auth/privacy-login' ||
    (path === '/api/me' && method === 'DELETE');
  if (
    android &&
    appToken &&
    Date.now() - rotatedAt >= 7 * 86400000 &&
    path !== '/api/auth/rotate' &&
    !rights
  ) {
    rotation ??= request<{ token: string }>('/api/auth/rotate', {})
      .then(async (rotated) => {
        if (epoch === generation) {
          appToken = rotated.token;
          rotatedAt = Date.now();
          if (android) {
            const store = await nativeStore();
            if (epoch === generation) await store.saveSession(rotated.token, rotatedAt);
          }
        }
      })
      .finally(() => {
        rotation = undefined;
      });
    await rotation;
  }
  if (epoch !== generation) throw new ApiError('GUEST_LOCAL');
  return request<T>(path, body, method);
}

export async function restoreAccount() {
  if (account()?.kind === 'guest') return;
  const epoch = generation;
  while (android && !restoredNative && epoch === generation) {
    restoringNative ??= nativeStore()
      .then(async (store) => {
        const stored = await store.loadSession();
        if (epoch === generation && account()?.kind !== 'guest') {
          appToken = stored?.token;
          rotatedAt = stored?.rotatedAt ?? 0;
          restoredNative = true;
        }
      })
      .finally(() => {
        restoringNative = undefined;
      });
    try {
      await restoringNative;
    } catch {
      return;
    }
  }
  if (epoch !== generation) return;
  if (android && !appToken) {
    setSessionKnown(true);
    return;
  }
  try {
    const profile = await api<Profile>('/api/me');
    if (epoch === generation) {
      setAccount({ kind: 'account', profile });
      setActivityBusy(Boolean(profile.busy));
      setSessionKnown(true);
    }
  } catch (error) {
    if (epoch === generation && error instanceof ApiError && error.code === 'UNAUTHENTICATED') {
      // A ban restricts gameplay while the still-valid own credential permits notices and rights.
      try {
        const own = await request<{ status: string; busy: boolean }>('/api/me/privacy');
        if (epoch !== generation) return;
        if (own.status === 'banned' || own.status === 'active') {
          setAccount(null);
          setActivityBusy(own.busy);
          setSessionKnown(true);
          return;
        }
      } catch {
        /* An expired/revoked credential is cleared below. */
      }
      if (epoch !== generation) return;
      appToken = undefined;
      setAccount(null);
      setActivityBusy(false);
      setSessionKnown(true);
      if (android) {
        const store = await nativeStore();
        if (epoch === generation) await store.clearSession();
      }
    }
  }
}

export async function signIn(identity: string, password: string, privacy = false) {
  controller.abort();
  controller = new AbortController();
  generation++;
  const epoch = generation;
  const result = await request<{ profile?: Profile; token?: string }>(
    privacy ? '/api/auth/privacy-login' : '/api/auth/login',
    {
      identity,
      password,
      client: android ? 'android' : 'web',
    },
  );
  if (epoch !== generation) return;
  appToken = android ? result.token : undefined;
  rotatedAt = Date.now();
  if (android) {
    restoredNative = true;
    if (!result.token) throw new ApiError('SERVER_UNAVAILABLE');
    try {
      const store = await nativeStore();
      if (epoch !== generation) return;
      await store.saveSession(result.token, rotatedAt);
    } catch {
      if (epoch !== generation) return;
      appToken = undefined;
      throw new ApiError('SECURE_STORAGE');
    }
    if (epoch !== generation) return;
  }
  useEphemeralPreferences(false);
  if (!privacy && !result.profile) throw new ApiError('SERVER_UNAVAILABLE');
  setAccount(result.profile ? { kind: 'account', profile: result.profile } : null);
  setActivityBusy(Boolean(result.profile?.busy));
  setSessionKnown(true);
}

export async function signOut() {
  const epoch = generation;
  await api('/api/auth/logout', {});
  if (epoch !== generation) return;
  if (android) {
    const store = await nativeStore();
    if (epoch !== generation) return;
    await store.clearSession();
  }
  if (epoch !== generation) return;
  forgetAccount();
}

export function forgetAccount() {
  controller.abort();
  controller = new AbortController();
  generation++;
  appToken = undefined;
  restoredNative = true;
  setAccount(null);
  setActivityBusy(false);
  setSessionKnown(true);
  const epoch = generation;
  if (android)
    void nativeStore()
      .then((store) => {
        if (epoch === generation) return store.clearSession();
      })
      .catch(() => {});
}

export const authClient = () => (android ? 'android' : 'web');

export async function websocketURL() {
  if (account()?.kind !== 'account') throw new ApiError('UNAUTHENTICATED');
  const url = new URL('/ws', android ? apiOrigin : location.origin);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  if (android) {
    const { ticket } = await api<{ ticket: string }>('/api/auth/ws-ticket', {});
    url.searchParams.set('ticket', ticket);
  }
  return url;
}
