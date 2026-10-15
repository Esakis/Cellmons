import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import {
  type TrainingReply,
  TrainingReplySchema,
  type TrainingRequest,
  TrainingRequestSchema,
} from '../generated/proto/game/v1/training_pb';

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
        const reply = fromBinary(TrainingReplySchema, new Uint8Array(event.data));
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
    const seq = ++this.seq;
    const bytes = toBinary(TrainingRequestSchema, create(TrainingRequestSchema, { seq, body }));
    return new Promise<TrainingReply>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(seq);
        reject(new Error('worker_failed'));
      }, 30_000);
      this.pending.set(seq, { resolve, reject, timer });
      this.worker.postMessage(bytes.buffer, [bytes.buffer]);
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
