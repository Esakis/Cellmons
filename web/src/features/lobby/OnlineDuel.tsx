import { useBeforeLeave } from '@solidjs/router';
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Index,
  onCleanup,
  onMount,
  Show,
} from 'solid-js';
import { avatars, cardArgs, cards, fluentKey, portrait } from '../../app/content';
import { setDuelActive } from '../../app/session';
import { Modal } from '../../components/Modal';
import type {
  DuelEnd,
  DuelMovePreview,
  DuelSnapshot,
} from '../../generated/proto/game/v1/online_duel_pb';
import type { DuelMove, DuelSide } from '../../generated/proto/game/v1/training_pb';
import { hasTranslation, t } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';

export function OnlineDuel(props: {
  snapshot: DuelSnapshot;
  end?: DuelEnd;
  connected: boolean;
  now: number;
  send: (body: ClientBody) => Promise<ServerBody | undefined>;
  close: () => void;
  replay?: boolean;
}) {
  const view = () => props.snapshot.view;
  const [attacks, setAttacks] = createSignal<number[]>([]);
  const [defenses, setDefenses] = createSignal<number[]>([]);
  const [action, setAction] = createSignal(0);
  const [handIndex, setHandIndex] = createSignal(0);
  const [preview, setPreview] = createSignal<DuelMovePreview>();
  const [sending, setSending] = createSignal(false);
  const [confirm, setConfirm] = createSignal(false);
  const [chronicle, setChronicle] = createSignal<{ round: number; lines: string[] }[]>([]);
  const [pictures, setPictures] = createSignal(new Map<string, string>());
  let current = '';
  let disposed = false;
  let generation = 0;
  let lastRevision = -1;
  onMount(() => {
    void import('../../app/cardArt')
      .then((module) => {
        if (!disposed) setPictures(module.cardPictures);
      })
      .catch(() => {});
  });
  const finished = () => !!view()?.outcome;
  const locked = () =>
    !props.connected || props.snapshot.submitted || finished() || sending() || !!props.replay;
  const move = (): DuelMove => ({
    $typeName: 'game.v1.DuelMove',
    attacks: attacks(),
    defenses: defenses(),
    action: action(),
    handIndex: handIndex(),
  });
  const remaining = createMemo(() =>
    Math.max(0, Math.ceil((Number(props.snapshot.deadlineMs) - props.now) / 1000)),
  );
  createEffect(() => {
    const snapshot = props.snapshot;
    const key = `${snapshot.duelId}:${snapshot.view?.round}`;
    if (key !== current) {
      current = key;
      setAttacks([...(snapshot.yourMove?.attacks ?? snapshot.view?.me?.attacks ?? [])]);
      setDefenses([...(snapshot.yourMove?.defenses ?? snapshot.view?.me?.defenses ?? [])]);
      setAction(snapshot.yourMove?.action ?? 0);
      setHandIndex(snapshot.yourMove?.handIndex ?? 0);
      setPreview(undefined);
    }
    if (snapshot.revision > lastRevision && snapshot.events.length) {
      lastRevision = snapshot.revision;
      const lines = snapshot.events
        .filter((e) => !['draw', 'round', 'reveal', 'end'].includes(e.kind))
        .map((e) => {
          const key = `event-${e.kind}`;
          const card = cards.get(e.cardId);
          return hasTranslation(key)
            ? t(key, {
                amount: e.amount,
                slot: e.slot + 1,
                old: e.oldValue,
                new: e.newValue,
                card: card ? t(fluentKey(card.key)) : '',
                name: e.side === 0 ? (snapshot.you?.login ?? '') : (snapshot.opponent?.login ?? ''),
                detail: e.detail,
                kind: t(e.attackKind === 0 ? 'online-physical' : 'online-magical'),
              })
            : '';
        })
        .filter(Boolean);
      const resolvedRound =
        snapshot.events.find((e) => e.kind === 'round')?.amount ??
        Math.max(1, (snapshot.view?.round ?? 1) - (finished() ? 0 : 1));
      if (lines.length)
        setChronicle((rows) =>
          [...rows.filter((r) => r.round !== resolvedRound), { round: resolvedRound, lines }].slice(
            -10,
          ),
        );
    }
    if (!props.replay) setDuelActive(!finished());
  });
  createEffect(() => {
    const mv = move();
    const snapshot = props.snapshot;
    const request = ++generation;
    if (locked() || attacks().length !== 3) return;
    const timer = setTimeout(async () => {
      const reply = await props.send({
        case: 'duelPreviewMove',
        value: {
          $typeName: 'game.v1.DuelPreviewMove',
          duelId: snapshot.duelId,
          round: snapshot.view?.round ?? 1,
          move: mv,
        },
      });
      if (!disposed && request === generation && reply?.case === 'duelMovePreview')
        setPreview(reply.value);
    }, 100);
    onCleanup(() => clearTimeout(timer));
  });
  useBeforeLeave((event) => {
    if (finished() || props.replay || window.confirm(t('online-leave-warning'))) return;
    event.preventDefault();
  });
  onCleanup(() => {
    disposed = true;
    generation++;
    if (!props.replay) setDuelActive(false);
  });
  async function submit() {
    setSending(true);
    try {
      const response = await props.send({
        case: 'duelSubmitMove',
        value: {
          $typeName: 'game.v1.DuelSubmitMove',
          duelId: props.snapshot.duelId,
          round: view()?.round ?? 1,
          move: move(),
        },
      });
      if (response)
        await props.send({
          case: 'duelSnapshot',
          value: { $typeName: 'game.v1.DuelTarget', duelId: props.snapshot.duelId },
        });
    } finally {
      if (!disposed) setSending(false);
    }
  }
  function side(data: DuelSide | undefined, login: string, template: number) {
    return (
      <article class="online-side">
        <Show when={avatars.get(template)}>
          {(a) => <img width="72" height="72" alt="" src={portrait(a().key)} />}
        </Show>
        <h3>{login}</h3>
        <p>
          HP {data?.hp} / {data?.hpStart} · W {data?.stamina} · K {data?.focus}
        </p>
        <progress
          aria-label={`${login} HP`}
          max={Math.max(1, data?.hpStart ?? 1)}
          value={Math.max(0, data?.hp ?? 0)}
        />
        <p>
          {t('online-deck', { count: data?.deckLen ?? 0 })} ·{' '}
          {t('online-hand', { count: data?.handLen ?? 0 })}
        </p>
      </article>
    );
  }
  return (
    <section
      class="online-duel lobby-panel"
      aria-label={t('online-duel')}
      data-testid="online-duel"
    >
      <div class="lobby-chat-heading">
        <h2>
          {t(props.replay ? 'online-replay' : 'online-duel')} ·{' '}
          {t('online-round', { round: view()?.round ?? 1, total: view()?.totalRounds ?? 5 })}
        </h2>
        <Show when={finished() || props.replay}>
          <button type="button" onClick={props.close}>
            {t('close')}
          </button>
        </Show>
      </div>
      <div class="online-sides">
        {side(view()?.me, props.snapshot.you?.login ?? '', props.snapshot.you?.templateId ?? 0)}
        {side(
          view()?.opponent,
          props.snapshot.opponent?.login ?? '',
          props.snapshot.opponent?.templateId ?? 0,
        )}
      </div>
      <Show when={!finished() && !props.replay}>
        <p role="status">
          {t(props.snapshot.submitted ? 'online-waiting' : 'online-deadline', {
            seconds: remaining(),
          })}{' '}
          · {t(props.snapshot.opponentReady ? 'online-opponent-ready' : 'online-opponent-thinking')}
        </p>
        <Show when={!props.snapshot.opponentConnected}>
          <p role="status">
            {t('online-opponent-offline', {
              seconds: Math.max(
                0,
                Math.ceil((Number(props.snapshot.graceUntilMs) - props.now) / 1000),
              ),
            })}
          </p>
        </Show>
        <fieldset disabled={locked()} class="online-controls">
          <legend>{t('online-slots')}</legend>
          <For each={['attacks', 'defenses'] as const}>
            {(kind) => (
              <div>
                <strong>{t(`online-${kind}`)}</strong>
                <Index each={kind === 'attacks' ? attacks() : defenses()}>
                  {(value, index) => (
                    <button
                      type="button"
                      aria-label={t(`online-${kind}-slot`, { slot: index + 1 })}
                      aria-pressed={value() === 1}
                      onClick={() => {
                        const update = (rows: number[]) =>
                          rows.map((v, i) => (i === index ? 1 - v : v));
                        if (kind === 'attacks') setAttacks(update);
                        else setDefenses(update);
                      }}
                    >
                      {t(value() === 0 ? 'online-physical' : 'online-magical')}
                    </button>
                  )}
                </Index>
              </div>
            )}
          </For>
        </fieldset>
        <fieldset disabled={locked()}>
          <legend>{t('online-card-action')}</legend>
          <label>
            {t('online-action')}{' '}
            <select value={action()} onChange={(e) => setAction(Number(e.currentTarget.value))}>
              <option value="0">{t('online-none')}</option>
              <option value="1">{t('online-cast')}</option>
              <option value="2">{t('online-discard')}</option>
            </select>
          </label>
          <div class="online-hand">
            <For each={view()?.hand}>
              {(id, index) => {
                const card = cards.get(id);
                return (
                  <button
                    type="button"
                    aria-pressed={handIndex() === index()}
                    onClick={() => setHandIndex(index())}
                  >
                    <strong>{card ? t(fluentKey(card.key)) : id}</strong>
                    <Show when={card && pictures().get(card.key)}>
                      {(source) => <img src={source()} width="128" height="96" alt="" />}
                    </Show>
                    <Show when={card}>
                      {(selected) => (
                        <>
                          <p>{t(fluentKey(selected().key, 'text'), cardArgs(selected()))}</p>
                          <small>
                            K {selected().cost.focus} · W {selected().cost.stamina}
                          </small>
                        </>
                      )}
                    </Show>
                  </button>
                );
              }}
            </For>
          </div>
        </fieldset>
        <Show when={preview()?.preview}>
          {(p) => (
            <p role="status">
              {t('online-cost', { focus: p().focusCost, stamina: p().staminaCost })}{' '}
              <Show when={p().error}>
                {t(hasTranslation(`error-${p().error}`) ? `error-${p().error}` : 'error-generic')}
              </Show>
            </p>
          )}
        </Show>
        <div class="lobby-actions">
          <button
            class="primary"
            type="button"
            disabled={locked() || !!preview()?.preview?.error}
            onClick={() => void submit()}
          >
            {t('online-submit')}
          </button>
          <button type="button" disabled={locked()} onClick={() => setConfirm(true)}>
            {t('online-forfeit')}
          </button>
        </div>
      </Show>
      <Show when={finished()}>
        <div class="online-result" role="status">
          <h3>
            {t(
              view()?.outcome?.winner === 0
                ? 'online-draw'
                : view()?.outcome?.winner === 1
                  ? 'online-win'
                  : 'online-loss',
            )}
          </h3>
          <p>{t(`online-reason-${view()?.outcome?.reason}`)}</p>
          <Show
            when={props.end}
            fallback={
              <Show when={!props.replay}>
                <p>{t('online-settling')}</p>
              </Show>
            }
          >
            {(end) => (
              <>
                <p>
                  +{end().xpGained} PD · {end().ratingDelta >= 0 ? '+' : ''}
                  {end().ratingDelta} RP · {end().ratingNew} RP · +{end().ducatsGained}{' '}
                  {t('store-ducats')}
                </p>
                <Show when={end().unlockedTemplate}>
                  <p>
                    {t('online-unlocked', {
                      avatar: t(fluentKey(avatars.get(end().unlockedTemplate)?.key ?? '')),
                    })}
                  </p>
                </Show>
              </>
            )}
          </Show>
        </div>
      </Show>
      <Show when={chronicle().length}>
        <details>
          <summary>{t('online-log')}</summary>
          <For each={chronicle()}>
            {(r) => (
              <div>
                <h3>{t('online-round', { round: r.round, total: view()?.totalRounds ?? 5 })}</h3>
                <For each={r.lines}>{(line) => <p>{line}</p>}</For>
              </div>
            )}
          </For>
        </details>
      </Show>
      <Show when={confirm()}>
        <Modal title={t('online-forfeit')} close={() => setConfirm(false)}>
          <p>{t('online-forfeit-confirm')}</p>
          <button
            type="button"
            onClick={async () => {
              setConfirm(false);
              await props.send({
                case: 'duelForfeit',
                value: { $typeName: 'game.v1.DuelTarget', duelId: props.snapshot.duelId },
              });
            }}
          >
            {t('online-forfeit')}
          </button>
        </Modal>
      </Show>
    </section>
  );
}
