import { create } from '@bufbuild/protobuf';
import { cleanup, fireEvent, render } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { setSpeed } from '../../app/session';
import {
  DuelMovePreviewSchema,
  DuelSnapshotSchema,
} from '../../generated/proto/game/v1/online_duel_pb';
import { addTranslations, setLanguage } from '../../i18n';
import type { ServerBody } from '../../net/client';
import en from './en.ftl?raw';
import { OnlineDuel } from './OnlineDuel';
import pl from './pl.ftl?raw';

vi.mock('@solidjs/router', () => ({ useBeforeLeave: vi.fn() }));
vi.mock('../../app/cardArt', () => ({ cardPictures: new Map() }));
addTranslations({ pl, en });
beforeEach(() => {
  setSpeed('instant');
  // Browsers mark the promise that a cancelled animation rejects as handled; happy-dom does not.
  vi.spyOn(Element.prototype, 'animate').mockImplementation(
    () => ({ cancel() {}, playState: 'finished' }) as unknown as Animation,
  );
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

// The snapshot after `round` was resolved (the duel ends after round 5).
function snapshot(round: number, damage: number, revision = 7, duelId = 'first') {
  return create(DuelSnapshotSchema, {
    duelId,
    revision,
    you: { login: 'Alice' },
    opponent: { login: 'Bob' },
    opponentConnected: true,
    view: {
      round: round === 5 ? round : round + 1,
      totalRounds: 5,
      me: { hp: 100, hpStart: 100 },
      opponent: { hp: 100, hpStart: 100 },
      ...(round === 5 ? { outcome: { winner: 0, reason: 'rounds' } } : {}),
    },
    events: [
      { kind: 'reveal' },
      { kind: 'hit', amount: damage },
      round === 5 ? { kind: 'end' } : { kind: 'round', amount: round + 1 },
    ],
  });
}

it('labels completed rounds correctly, including the final round, and resets for a new duel', () => {
  setLanguage('en');
  const [value, setValue] = createSignal(snapshot(1, 11));
  const { container, getByRole } = render(() => (
    <OnlineDuel snapshot={value()} connected={false} now={0} send={vi.fn()} close={vi.fn()} />
  ));
  // A live duel keeps its rounds in the chronicle; the written log is shown only in replays.
  expect(container.querySelector('details')).toBeNull();
  fireEvent.click(getByRole('button', { name: 'Battle chronicle' }));
  const rounds = () => [...container.querySelectorAll('caption')].map((c) => c.textContent);
  const log = () => container.querySelector('dialog details')?.textContent;
  expect(rounds()).toEqual(['Round 1']);
  setValue(snapshot(5, 55, 8));
  expect(rounds()).toEqual(['Round 5', 'Round 1']);
  setValue(snapshot(1, 22, 0, 'second'));
  expect(rounds()).toEqual(['Round 1']);
  expect(log()).toContain('22 damage');
  expect(log()).not.toContain('55 damage');
});

it('updates replay events in both directions even when all snapshots share a revision', () => {
  setLanguage('en');
  const initial = create(DuelSnapshotSchema, {
    ...snapshot(1, 11),
    events: [],
  });
  const [value, setValue] = createSignal(initial);
  const { container } = render(() => (
    <OnlineDuel
      snapshot={value()}
      connected={false}
      now={0}
      replay
      send={vi.fn()}
      close={vi.fn()}
    />
  ));
  expect(container.querySelector('details')).toBeNull();
  setValue(snapshot(1, 11));
  expect(container.querySelector('details')?.textContent).toContain('11 damage');
  setValue(snapshot(2, 22));
  expect(container.querySelector('details')?.textContent).toContain('22 damage');
  expect(container.querySelector('details')?.textContent).not.toContain('11 damage');
  setValue(snapshot(1, 11));
  expect(container.querySelector('details')?.textContent).toContain('11 damage');
  expect(container.querySelector('details')?.textContent).not.toContain('22 damage');
  setLanguage('pl');
  expect(container.querySelector('details')?.textContent).toContain('obrażeń');
  setValue(initial);
  expect(container.querySelector('details')).toBeNull();
});

it('requires validation of the current draft and displays the effective spell costs', async () => {
  vi.useFakeTimers();
  setLanguage('en');
  const value = snapshot(1, 11);
  if (!value.view?.me) throw new Error('Missing fixture side');
  value.view.me.attacks = [0, 0, 0];
  value.view.me.defenses = [0, 0, 0];
  value.view.hand = [1];
  const responses: ((body: ServerBody) => void)[] = [];
  const send = vi.fn(() => new Promise<ServerBody>((resolve) => responses.push(resolve)));
  const { getByRole } = render(() => (
    <OnlineDuel snapshot={value} connected now={0} send={send} close={vi.fn()} />
  ));
  const submit = getByRole('button', { name: 'Submit move' }) as HTMLButtonElement;
  const reply = (error = ''): ServerBody => ({
    case: 'duelMovePreview',
    value: create(DuelMovePreviewSchema, {
      preview: { error },
      castOptions: [{ changes: 2, focusCost: 7, staminaCost: 13 }],
    }),
  });
  expect(submit.disabled).toBe(true);
  await vi.advanceTimersByTimeAsync(100);
  fireEvent.click(getByRole('button', { name: /^Attack 1:/ }));
  await vi.advanceTimersByTimeAsync(100);
  // A late reply for an older move cannot enable submission.
  responses[0]?.(reply());
  await Promise.resolve();
  expect(submit.disabled).toBe(true);
  responses[1]?.(reply('focus'));
  await Promise.resolve();
  expect(submit.disabled).toBe(true);
  fireEvent.click(getByRole('button', { name: /^Attack 1:/ }));
  expect(submit.disabled).toBe(true);
  await vi.advanceTimersByTimeAsync(100);
  responses[2]?.(reply());
  await Promise.resolve();
  expect(submit.disabled).toBe(false);
  // Focus 7 minus two changes at 1 K each; stamina as the server computed it.
  expect(getByRole('button', { name: /^.+\. 5 K, 13 W\./ })).toBeTruthy();
});

it('simulates each newly resolved round and then shows the opponent’s previous settings', async () => {
  vi.useFakeTimers();
  setLanguage('en');
  setSpeed('normal');
  const duel = {
    duelId: 'sim',
    you: { login: 'Alice' },
    opponent: { login: 'Bob' },
    opponentConnected: true,
  };
  const side = (hp: number, attacks = [0, 0, 0], defenses = [0, 0, 0]) => ({
    hp,
    hpStart: 30,
    attacks,
    defenses,
  });
  const start = create(DuelSnapshotSchema, {
    ...duel,
    view: { round: 1, totalRounds: 5, me: side(30), opponent: side(30) },
  });
  const resolved = create(DuelSnapshotSchema, {
    ...duel,
    revision: 1,
    view: { round: 2, totalRounds: 5, me: side(26), opponent: side(30, [1, 0, 1], [0, 1, 1]) },
    events: [
      { kind: 'reveal' },
      { kind: 'hit', side: 1, slot: 0, attackKind: 1, amount: 4 },
      { kind: 'round', amount: 2 },
    ],
  });
  const [value, setValue] = createSignal(start);
  const { getByRole, getByTitle, queryByRole } = render(() => (
    <OnlineDuel snapshot={value()} connected now={0} send={vi.fn()} close={vi.fn()} />
  ));
  const alts = (title: string) =>
    [...getByTitle(title).querySelectorAll('img')].map((image) => image.alt);
  expect(alts("Opponent's defenses last round")).toEqual([]);
  setValue(resolved);
  // The reveal is shown first, then the strike.
  await vi.advanceTimersByTimeAsync(600);
  expect(getByRole('status').textContent).toContain('Bob: attack 1, Magical, deals 4 damage.');
  fireEvent.click(getByRole('button', { name: 'Skip animation' }));
  expect(queryByRole('button', { name: 'Skip animation' })).toBeNull();
  expect(alts("Opponent's defenses last round")).toEqual(['Physical', 'Magical', 'Magical']);
  expect(alts("Opponent's attacks last round")).toEqual(['Magical', 'Physical', 'Magical']);
  // A later snapshot of the same resolution (opponent ready, a forfeit) does not replay it.
  setValue(create(DuelSnapshotSchema, { ...resolved, revision: 2, opponentReady: true }));
  expect(queryByRole('button', { name: 'Skip animation' })).toBeNull();
  fireEvent.click(getByRole('button', { name: 'Replay round' }));
  expect(getByRole('button', { name: 'Skip animation' })).toBeTruthy();
});

it('does not animate a round resolved before the screen opened', () => {
  setLanguage('en');
  setSpeed('normal');
  const { queryByRole, getByRole } = render(() => (
    <OnlineDuel snapshot={snapshot(2, 9)} connected now={0} send={vi.fn()} close={vi.fn()} />
  ));
  expect(queryByRole('button', { name: 'Skip animation' })).toBeNull();
  expect((getByRole('button', { name: 'Replay round' }) as HTMLButtonElement).disabled).toBe(false);
});
