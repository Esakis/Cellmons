import { createSignal } from 'solid-js';
import { useEphemeralPreferences } from './preferences';

export type Profile = {
  id: string;
  login: string;
  email: string;
  email_verified: boolean;
  start_school: string;
  role: string;
  avatar: { id: string; template_id: number; level: number; xp: number } | null;
};
export type Account = { kind: 'guest' } | { kind: 'account'; profile: Profile } | null;
export const [account, setAccount] = createSignal<Account>(null);
let generation = 0;
let controller = new AbortController();
let appToken: string | undefined;
let rotatedAt = 0;
let rotation: Promise<void> | undefined;
const android = import.meta.env.VITE_PLATFORM === 'android';
const apiOrigin = android ? (import.meta.env.VITE_API_ORIGIN ?? '') : '';

export function enterGuest() {
  controller.abort();
  controller = new AbortController();
  generation++;
  appToken = undefined;
  useEphemeralPreferences(true);
  setAccount({ kind: 'guest' });
}

export function leaveGuest() {
  generation++;
  useEphemeralPreferences(false);
  setAccount(null);
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
    headers['X-Native-Version'] = '1.0.0';
    if (appToken) headers.Authorization = `Bearer ${appToken}`;
  }
  let response: Response;
  try {
    response = await fetch(`${apiOrigin}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: android ? 'omit' : 'same-origin',
      cache: 'no-store',
      signal: controller.signal,
    });
  } catch {
    throw new ApiError('SERVER_UNAVAILABLE');
  }
  if (response.status === 204) return undefined as T;
  const result = await response.json().catch(() => null);
  if (!response.ok)
    throw new ApiError(
      result?.error ?? 'SERVER_UNAVAILABLE',
      Number(response.headers.get('Retry-After')) || 0,
    );
  if (!result) throw new ApiError('SERVER_UNAVAILABLE');
  return result as T;
}

export async function api<T>(path: string, body?: unknown, method?: string): Promise<T> {
  const epoch = generation;
  if (
    android &&
    appToken &&
    Date.now() - rotatedAt >= 7 * 86400000 &&
    path !== '/api/auth/rotate'
  ) {
    rotation ??= request<{ token: string }>('/api/auth/rotate', {})
      .then((rotated) => {
        if (epoch === generation) {
          appToken = rotated.token;
          rotatedAt = Date.now();
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
  if (account()?.kind === 'guest' || (android && !appToken)) return;
  const epoch = generation;
  try {
    const profile = await api<Profile>('/api/me');
    if (epoch === generation) setAccount({ kind: 'account', profile });
  } catch (error) {
    if (epoch === generation && error instanceof ApiError && error.code === 'UNAUTHENTICATED') {
      appToken = undefined;
      setAccount(null);
    }
  }
}

export async function signIn(identity: string, password: string) {
  const epoch = generation;
  const result = await api<{ profile: Profile; token?: string }>('/api/auth/login', {
    identity,
    password,
    client: android ? 'android' : 'web',
  });
  if (epoch !== generation) return;
  appToken = android ? result.token : undefined;
  rotatedAt = Date.now();
  useEphemeralPreferences(false);
  setAccount({ kind: 'account', profile: result.profile });
}

export async function signOut() {
  await api('/api/auth/logout', {});
  forgetAccount();
}

export function forgetAccount() {
  generation++;
  appToken = undefined;
  setAccount(null);
}

export const authClient = () => (android ? 'android' : 'web');
