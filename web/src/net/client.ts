import { account, authClient, restoreAccount, websocketURL } from '../app/account';
import { content } from '../app/content';
import { appForeground } from '../app/session';
import type { ClientMessage, ServerMessage } from '../generated/proto/game/v1/envelope_pb';
import { language } from '../i18n';
import { decodeServer, encodeClient } from './wire';

export type ClientBody = ClientMessage['body'];
export type ServerBody = ServerMessage['body'];
export type Status =
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'offline'
  | 'replaced'
  | 'incompatible'
  | 'signed-out';
export class NetError extends Error {
  constructor(public code: string) {
    super(code);
  }
}
type Pending = {
  resolve: (body: ServerBody) => void;
  reject: (error: NetError) => void;
  timeout: number;
};

export class OnlineClient {
  private socket?: WebSocket;
  private active = false;
  private generation = 0;
  private seq = 0;
  private serverSeq = 0;
  private resume?: string;
  private retry?: number;
  private attempt = 0;
  private pending = new Map<number, Pending>();
  private ready = false;
  private fatal?: Status;
  onMessage: (body: ServerBody) => void = () => {};
  onStatus: (status: Status) => void = () => {};
  private offline = () => {
    this.rejectPending('DISCONNECTED');
    this.socket?.close();
    this.ready = false;
    this.onStatus('offline');
  };
  private online = () => {
    if (!this.active || this.ready || this.fatal) return;
    if (this.retry !== undefined) clearTimeout(this.retry);
    this.retry = undefined;
    this.socket?.close();
    void this.connect();
  };

  start() {
    if (account()?.kind !== 'account') {
      this.onStatus('signed-out');
      return;
    }
    if (this.active) return;
    this.active = true;
    window.addEventListener('online', this.online);
    window.addEventListener('offline', this.offline);
    this.fatal = undefined;
    void this.connect();
  }
  stop() {
    window.removeEventListener('online', this.online);
    window.removeEventListener('offline', this.offline);
    this.active = false;
    this.generation++;
    this.ready = false;
    if (this.retry !== undefined) clearTimeout(this.retry);
    this.retry = undefined;
    this.socket?.close();
    this.socket = undefined;
    this.rejectPending('DISCONNECTED');
  }
  restart() {
    this.stop();
    this.resume = undefined;
    this.attempt = 0;
    this.start();
  }
  private rejectPending(code: string) {
    for (const job of this.pending.values()) {
      clearTimeout(job.timeout);
      job.reject(new NetError(code));
    }
    this.pending.clear();
  }
  private schedule() {
    if (!this.active || this.fatal || account()?.kind !== 'account') return;
    if (import.meta.env.VITE_PLATFORM === 'android' && !appForeground()) return;
    this.onStatus(navigator.onLine ? 'reconnecting' : 'offline');
    const delay =
      Math.min(8000, 500 * 2 ** Math.min(this.attempt++, 4)) * (0.8 + Math.random() * 0.2);
    this.retry = window.setTimeout(() => {
      this.retry = undefined;
      void this.connect();
    }, delay);
  }
  private async connect() {
    const generation = ++this.generation;
    this.ready = false;
    if (import.meta.env.VITE_PLATFORM === 'android' && !appForeground()) return;
    if (!navigator.onLine) {
      this.schedule();
      return;
    }
    this.onStatus(this.attempt ? 'reconnecting' : 'connecting');
    try {
      const url = await websocketURL();
      if (import.meta.env.VITE_PLATFORM === 'android' && !appForeground()) return;
      if (!this.active || generation !== this.generation || account()?.kind !== 'account') return;
      const socket = new WebSocket(url);
      this.socket = socket;
      socket.binaryType = 'arraybuffer';
      this.seq = 0;
      this.serverSeq = 0;
      socket.onopen = () => {
        if (generation !== this.generation || !this.active) {
          socket.close();
          return;
        }
        const body: ClientBody = this.resume
          ? { case: 'resume', value: { $typeName: 'game.v1.Resume', resumeToken: this.resume } }
          : {
              case: 'hello',
              value: {
                $typeName: 'game.v1.Hello',
                protocolMajor: 1,
                protocolMinor: 0,
                build: '0.2.0',
                contentVersion: Uint8Array.from(content.version.match(/../g) ?? [], (byte) =>
                  Number.parseInt(byte, 16),
                ),
                locale: language(),
                clientKind: authClient(),
                nativeVersion:
                  authClient() === 'android'
                    ? (import.meta.env.VITE_NATIVE_VERSION ?? '1.0.0')
                    : '',
              },
            };
        void this.send(body, true).catch((error: NetError) => {
          if (generation !== this.generation || !this.active) return;
          if (error.code === 'RESUME_EXPIRED') this.resume = undefined;
          else if (error.code === 'VERSION_MISMATCH') {
            if (import.meta.env.VITE_PLATFORM === 'android')
              window.dispatchEvent(new Event('game-version-mismatch'));
            this.fatal = 'incompatible';
            this.onStatus('incompatible');
          }
          socket.close();
        });
      };
      socket.onmessage = (event: MessageEvent<ArrayBuffer>) => {
        if (
          generation !== this.generation ||
          !this.active ||
          !(event.data instanceof ArrayBuffer) ||
          event.data.byteLength > 256 * 1024
        )
          return;
        try {
          const reply = decodeServer(new Uint8Array(event.data));
          if (reply.seq <= this.serverSeq) {
            socket.close();
            return;
          }
          this.serverSeq = reply.seq;
          const body = reply.body;
          if (import.meta.env.VITE_PLATFORM === 'android' && body.case === 'duelSnapshot') {
            window.dispatchEvent(new CustomEvent('game-duel-feedback', { detail: body.value }));
          }
          if (body.case === 'welcome') {
            this.resume = body.value.resumeToken;
            this.ready = true;
            this.attempt = 0;
            this.onStatus('connected');
          }
          if (body.case === 'sessionReplaced') {
            this.fatal = 'replaced';
            this.onStatus('replaced');
          }
          if (body.case === 'error' && body.value.code === 'UNAUTHENTICATED') void restoreAccount();
          const pending = this.pending.get(reply.replyTo);
          if (pending) {
            this.pending.delete(reply.replyTo);
            clearTimeout(pending.timeout);
            if (body.case === 'error') pending.reject(new NetError(body.value.code));
            else pending.resolve(body);
          }
          this.onMessage(body);
        } catch {
          socket.close();
        }
      };
      socket.onclose = () => {
        if (generation !== this.generation) return;
        this.ready = false;
        this.rejectPending('DISCONNECTED');
        if (account()?.kind !== 'account') {
          this.onStatus('signed-out');
          return;
        }
        this.schedule();
      };
      socket.onerror = () => socket.close();
    } catch {
      if (this.active && generation === this.generation) {
        await restoreAccount();
        this.schedule();
      }
    }
  }
  send(body: ClientBody, handshake = false): Promise<ServerBody> {
    const socket = this.socket;
    if (
      !this.active ||
      account()?.kind !== 'account' ||
      socket?.readyState !== WebSocket.OPEN ||
      (!this.ready && !handshake)
    )
      return Promise.reject(new NetError('DISCONNECTED'));
    if (this.pending.size >= 60) return Promise.reject(new NetError('RATE_LIMITED'));
    const seq = ++this.seq;
    if (seq > 0xffffffff) {
      this.restart();
      return Promise.reject(new NetError('DISCONNECTED'));
    }
    const bytes = encodeClient({ $typeName: 'game.v1.ClientMessage', seq, body });
    if (bytes.byteLength > 16384) return Promise.reject(new NetError('INVALID_MESSAGE'));
    return new Promise((resolve, reject) => {
      const timeout = window.setTimeout(() => {
        this.pending.delete(seq);
        reject(new NetError('TIMEOUT'));
        socket.close();
      }, 10000);
      this.pending.set(seq, { resolve, reject, timeout });
      try {
        socket.send(bytes);
      } catch {
        clearTimeout(timeout);
        this.pending.delete(seq);
        reject(new NetError('DISCONNECTED'));
      }
    });
  }
}
