import { afterEach, expect, it, vi } from 'vitest';
import { enterGuest, forgetAccount } from '../../app/account';
import { guestMutation, guestState } from './state';

afterEach(() => {
  forgetAccount();
  vi.restoreAllMocks();
});
it('keeps guest characters and decks only in RAM and isolates each guest session', () => {
  const network = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('unexpected network'));
  const storage = vi.spyOn(Storage.prototype, 'setItem');
  enterGuest();
  const initial = guestState();
  const id = crypto.randomUUID();
  const created = guestMutation({ action: 'create', id, template_id: initial.templates[0] ?? 1 });
  expect(created.avatars).toHaveLength(2);
  const card = initial.collection[0]?.card_id ?? 1;
  const saved = guestMutation({ action: 'deck', id, revision: 0, cards: [card, card, card, card] });
  expect(saved.avatars.find((a) => a.id === id)?.deck).toEqual([card, card, card, card]);
  expect(() =>
    guestMutation({ action: 'deck', id, revision: 1, cards: [card, card, card, card, card] }),
  ).toThrow('INVALID_DECK');
  expect(() => guestMutation({ action: 'deck', id, revision: 0, cards: [] })).toThrow('CONFLICT');
  saved.avatars.length = 0;
  expect(guestState().avatars).toHaveLength(2);
  guestMutation({ action: 'activate', id });
  const original = guestState().avatars.find((a) => a.id !== id);
  if (!original) throw new Error('missing original avatar');
  guestMutation({ action: 'delete', id: original.id, revision: original.revision });
  const reused = guestMutation({
    action: 'create',
    id: crypto.randomUUID(),
    template_id: initial.templates[0] ?? 1,
  });
  expect(reused.avatars.filter((a) => a.active)).toHaveLength(1);
  forgetAccount();
  enterGuest();
  expect(guestState().avatars).toHaveLength(1);
  expect(network).not.toHaveBeenCalled();
  expect(storage).not.toHaveBeenCalled();
});
