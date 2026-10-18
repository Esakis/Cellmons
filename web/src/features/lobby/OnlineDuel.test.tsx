import { create } from '@bufbuild/protobuf';
import { cleanup, fireEvent, render } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { afterEach, expect, it, vi } from 'vitest';
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
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

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
      { kind: 'hit', amount: damage },
      round === 5 ? { kind: 'end' } : { kind: 'round', amount: round + 1 },
    ],
  });
}

it('labels completed rounds correctly, including the final round, and resets for a new duel', () => {
  setLanguage('en');
  const [value, setValue] = createSignal(snapshot(1, 11));
  const { container } = render(() => (
    <OnlineDuel snapshot={value()} connected={false} now={0} send={vi.fn()} close={vi.fn()} />
  ));
  const headings = () => [...container.querySelectorAll('details h3')].map((h) => h.textContent);
  expect(headings()).toEqual(['Round 1 / 5']);
  setValue(snapshot(5, 55, 8));
  expect(headings()).toEqual(['Round 1 / 5', 'Round 5 / 5']);
  setValue(snapshot(1, 22, 0, 'second'));
  expect(headings()).toEqual(['Round 1 / 5']);
  expect(container.querySelector('details')?.textContent).toContain('22 damage');
  expect(container.querySelector('details')?.textContent).not.toContain('55 damage');
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
  const { container, getByRole } = render(() => (
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
  fireEvent.click(getByRole('button', { name: 'Attack 1' }));
  await vi.advanceTimersByTimeAsync(100);
  // A late reply for an older move cannot enable submission.
  responses[0]?.(reply());
  await Promise.resolve();
  expect(submit.disabled).toBe(true);
  responses[1]?.(reply('focus'));
  await Promise.resolve();
  expect(submit.disabled).toBe(true);
  fireEvent.click(getByRole('button', { name: 'Attack 1' }));
  expect(submit.disabled).toBe(true);
  await vi.advanceTimersByTimeAsync(100);
  responses[2]?.(reply());
  await Promise.resolve();
  expect(submit.disabled).toBe(false);
  expect(container.querySelector('.online-hand small')?.textContent).toBe('K 5 · W 13');
});
