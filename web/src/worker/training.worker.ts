/// <reference lib="webworker" />
import init, { Training } from '../generated/engine/engine_wasm';

const scope = self as unknown as DedicatedWorkerGlobalScope;
let session: Training | undefined;
const ready = init().then(() => {
  session = new Training();
});
// Serialize initialization and commands: messages arriving during WASM load keep their order.
let queue = ready;
scope.onmessage = (event: MessageEvent<ArrayBuffer>) => {
  queue = queue.then(() => {
    if (!session) throw new Error('WASM initialization failed');
    const reply = session.dispatch(new Uint8Array(event.data));
    scope.postMessage(reply.buffer, [reply.buffer]);
  });
  queue.catch(() => {
    scope.postMessage({ error: 'worker_failed' });
  });
};
