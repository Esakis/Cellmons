import { create, toBinary } from '@bufbuild/protobuf';
import { afterEach, expect, it, vi } from 'vitest';
import { enterGuest, forgetAccount, leaveGuest, setAccount } from '../app/account';
import { ServerMessageSchema } from '../generated/proto/game/v1/envelope_pb';
import { OnlineClient } from './client';

afterEach(() => {
  leaveGuest();
  forgetAccount();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function sockets() {
  const instances: Socket[] = [];
  class Socket {
    static OPEN = 1;
    readyState = 0;
    onopen?: () => void;
    onclose?: () => void;
    onmessage?: (event: { data: ArrayBuffer }) => void;
    send = vi.fn();
    close = vi.fn(() => {
      this.readyState = 3;
    });
    constructor() {
      instances.push(this);
    }
    open() {
      this.readyState = 1;
      this.onopen?.();
    }
    welcome() {
      const bytes = toBinary(
        ServerMessageSchema,
        create(ServerMessageSchema, {
          seq: 1,
          replyTo: 1,
          body: { case: 'welcome', value: { resumeToken: 'a'.repeat(64) } },
        }),
      );
      this.onmessage?.({ data: bytes.buffer as ArrayBuffer });
    }
  }
  vi.stubGlobal('WebSocket', Socket);
  vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(true);
  setAccount({
    kind: 'account',
    profile: {
      id: 'fixture',
      login: 'Fixture',
      email: 'fixture@example.test',
      email_verified: true,
      start_school: 'combat',
      role: 'player',
      avatar: null,
    },
  });
  return instances;
}

it('reconnecting during the handshake clears the old request and its timeout', async () => {
  vi.useFakeTimers();
  const instances = sockets();
  const client = new OnlineClient();
  const status = vi.fn();
  client.onStatus = status;
  try {
    client.start();
    await Promise.resolve();
    instances[0]?.open();
    expect(vi.getTimerCount()).toBe(1);
    window.dispatchEvent(new Event('online'));
    await Promise.resolve();
    expect(instances).toHaveLength(2);
    instances[1]?.open();
    instances[1]?.welcome();
    await Promise.resolve();
    expect(status).toHaveBeenLastCalledWith('connected');
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(10_001);
    expect(instances[1]?.close).not.toHaveBeenCalled();
  } finally {
    client.stop();
  }
});

it('going offline cancels connection setup before a socket opens', async () => {
  vi.useFakeTimers();
  const instances = sockets();
  const client = new OnlineClient();
  try {
    client.start();
    window.dispatchEvent(new Event('offline'));
    await Promise.resolve();
    expect(instances).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
    window.dispatchEvent(new Event('online'));
    await Promise.resolve();
    expect(instances).toHaveLength(1);
  } finally {
    client.stop();
  }
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
