import { afterEach, expect, it, vi } from 'vitest';
import { enterGuest, forgetAccount, leaveGuest } from '../app/account';
import { OnlineClient } from './client';

afterEach(() => {
  leaveGuest();
  forgetAccount();
  vi.unstubAllGlobals();
});

it('manual reconnect cannot open a socket or request credentials without an account', () => {
  const socket = vi.fn();
  const fetch = vi.fn();
  vi.stubGlobal('WebSocket', socket);
  vi.stubGlobal('fetch', fetch);
  for (const guest of [false, true]) {
    forgetAccount();
    if (guest) enterGuest();
    const client = new OnlineClient();
    const status = vi.fn();
    client.onStatus = status;
    client.restart();
    expect(status).toHaveBeenLastCalledWith('signed-out');
    expect(socket).not.toHaveBeenCalled();
    expect(fetch).not.toHaveBeenCalled();
    client.stop();
  }
});
