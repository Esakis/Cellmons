import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  account,
  api,
  enterGuest,
  forgetAccount,
  leaveGuest,
  type Profile,
  restoreAccount,
  signIn,
  signOut,
} from './account';
import { readPreference, writePreference } from './preferences';

const profile: Profile = {
  id: 'user-id',
  login: 'PlayerOne',
  email: 'one@example.test',
  email_verified: false,
  start_school: 'combat',
  role: 'player',
  avatar: null,
};
afterEach(() => {
  leaveGuest();
  forgetAccount();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  localStorage.clear();
});

describe('account boundaries', () => {
  it('guest uses memory for preferences and cannot call the API', async () => {
    const fetch = vi.fn();
    vi.stubGlobal('fetch', fetch);
    localStorage.setItem('game.avatar', '2');
    const before = { ...localStorage };
    enterGuest();
    writePreference('avatar', '6');
    writePreference('language', 'en');
    expect(readPreference('avatar', '1')).toBe('6');
    expect({ ...localStorage }).toEqual(before);
    await expect(api('/api/me')).rejects.toMatchObject({ code: 'GUEST_LOCAL' });
    await restoreAccount();
    expect(fetch).not.toHaveBeenCalled();
    leaveGuest();
    expect(readPreference('avatar', '1')).toBe('2');
    expect(account()).toBeNull();
  });

  it('an old profile response cannot replace a guest entered while it was pending', async () => {
    let complete!: (response: Response) => void;
    vi.stubGlobal(
      'fetch',
      vi.fn(
        () =>
          new Promise<Response>((resolve) => {
            complete = resolve;
          }),
      ),
    );
    const restoring = restoreAccount();
    enterGuest();
    complete(new Response(JSON.stringify(profile), { status: 200 }));
    await restoring;
    expect(account()?.kind).toBe('guest');
  });

  it('web sign-in and sign-out keep credentials out of browser storage', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ profile }), { status: 200 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal('fetch', fetch);
    const write = vi.spyOn(localStorage, 'setItem');
    await signIn('PlayerOne', 'long test credential');
    expect(account()).toEqual({ kind: 'account', profile });
    expect(fetch.mock.calls[0]?.[1]).toMatchObject({
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { 'X-Requested-With': 'app' },
    });
    expect(fetch.mock.calls[0]?.[1].headers.Authorization).toBeUndefined();
    await signOut();
    expect(account()).toBeNull();
    expect(write).not.toHaveBeenCalled();
  });
});
