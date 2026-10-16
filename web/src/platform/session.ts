// This module is imported only inside Android build branches; web and guests do not use a vault.
import { native } from './bridge';

let queue: Promise<unknown> = Promise.resolve();
function ordered<T>(action: () => Promise<T>): Promise<T> {
  const result = queue.then(action);
  queue = result.catch(() => {});
  return result;
}
export const loadSession = () =>
  ordered(() => native.loadSession()).then((value) =>
    value.token ? { token: value.token, rotatedAt: value.rotatedAt } : undefined,
  );
export const saveSession = (token: string, rotatedAt: number) =>
  ordered(() => native.saveSession({ token, rotatedAt }));
export const clearSession = () => ordered(() => native.clearSession());
