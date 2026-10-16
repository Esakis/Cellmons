import { trainingPlayer } from '../app/trainingPlayer';
import type { TrainingReply, TrainingRequest } from '../generated/proto/game/v1/training_pb';
import { decodeReply, encodeRequest } from './wire';

export class TrainingClient {
  private worker = new Worker(new URL('./training.worker.ts', import.meta.url), { type: 'module' });
  private seq = 0;
  private unavailable = false;
  private pending = new Map<
    number,
    {
      resolve: (reply: TrainingReply) => void;
      reject: (e: Error) => void;
      timer: ReturnType<typeof setTimeout>;
    }
  >();

  constructor() {
    this.worker.onmessage = (event: MessageEvent<ArrayBuffer | { error: string }>) => {
      if (!(event.data instanceof ArrayBuffer)) {
        this.fail();
        return;
      }
      try {
        const reply = decodeReply(new Uint8Array(event.data));
        const waiting = this.pending.get(reply.seq);
        if (!waiting) return;
        clearTimeout(waiting.timer);
        this.pending.delete(reply.seq);
        if (reply.error) waiting.reject(new Error(reply.error));
        else waiting.resolve(reply);
      } catch {
        this.fail();
      }
    };
    this.worker.onerror = () => this.fail();
    this.worker.onmessageerror = () => this.fail();
  }

  request(body: TrainingRequest['body']) {
    if (this.unavailable) return Promise.reject<TrainingReply>(new Error('worker_failed'));
    if (body.case === 'start') {
      const player = trainingPlayer(body.value.avatarId);
      if (player)
        body = {
          case: 'start',
          value: {
            ...body.value,
            custom: {
              $typeName: 'game.v1.TrainingCustomPlayer',
              level: player.level,
              spent: player.spent,
              deck: player.deck,
              specialization: player.specialization ?? '',
              giftId: player.gift_id ?? 0,
            },
          },
        };
    }
    const seq = ++this.seq;
    const bytes = encodeRequest({ $typeName: 'game.v1.TrainingRequest', seq, body });
    return new Promise<TrainingReply>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(seq);
        reject(new Error('worker_failed'));
      }, 30_000);
      this.pending.set(seq, { resolve, reject, timer });
      this.worker.postMessage(bytes.buffer, [bytes.buffer]);
    }).then((reply) => {
      if (
        import.meta.env.VITE_PLATFORM === 'android' &&
        (body.case === 'play' || body.case === 'forfeit')
      )
        window.dispatchEvent(new CustomEvent('game-feedback', { detail: reply.events }));
      return reply;
    });
  }

  private fail() {
    this.unavailable = true;
    for (const waiting of this.pending.values()) {
      clearTimeout(waiting.timer);
      waiting.reject(new Error('worker_failed'));
    }
    this.pending.clear();
  }

  dispose() {
    this.worker.terminate();
    this.fail();
  }
}
