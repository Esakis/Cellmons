import { afterEach, expect, it, vi } from 'vitest';
import { TrainingClient } from './client';

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function worker() {
  const instance = { postMessage: vi.fn(), terminate: vi.fn() };
  vi.stubGlobal(
    'Worker',
    class {
      postMessage = instance.postMessage;
      terminate = instance.terminate;
    },
  );
  return instance;
}
const request = { case: 'forfeit', value: { $typeName: 'game.v1.TrainingForfeit' } } as const;

it('terminates a stalled worker and rejects every pending and future request', async () => {
  vi.useFakeTimers();
  const instance = worker();
  const client = new TrainingClient();
  const first = expect(client.request(request)).rejects.toThrow('worker_failed');
  const second = expect(client.request(request)).rejects.toThrow('worker_failed');
  await vi.advanceTimersByTimeAsync(30_000);
  await Promise.all([first, second]);
  await expect(client.request(request)).rejects.toThrow('worker_failed');
  expect(instance.postMessage).toHaveBeenCalledTimes(2);
  expect(instance.terminate).toHaveBeenCalledTimes(1);
  expect(vi.getTimerCount()).toBe(0);
  client.dispose();
  expect(instance.terminate).toHaveBeenCalledTimes(1);
});

it('cleans up a synchronous postMessage failure immediately', async () => {
  vi.useFakeTimers();
  const instance = worker();
  instance.postMessage.mockImplementation(() => {
    throw new Error('DataCloneError');
  });
  const client = new TrainingClient();
  await expect(client.request(request)).rejects.toThrow('worker_failed');
  expect(instance.terminate).toHaveBeenCalledTimes(1);
  expect(vi.getTimerCount()).toBe(0);
  await expect(client.request(request)).rejects.toThrow('worker_failed');
  expect(instance.postMessage).toHaveBeenCalledTimes(1);
});
