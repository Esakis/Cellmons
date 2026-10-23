import { useBeforeLeave } from '@solidjs/router';
import {
  createEffect,
  createMemo,
  createSignal,
  createUniqueId,
  For,
  onCleanup,
  onMount,
  Show,
  untrack,
} from 'solid-js';
import { avatars, type Card, fluentKey } from '../../app/content';
import { setDuelActive, setOnlineDuelActive } from '../../app/session';
import { Modal } from '../../components/Modal';
import type {
  DuelEnd,
  DuelMovePreview,
  DuelSnapshot,
} from '../../generated/proto/game/v1/online_duel_pb';
import type { DuelEvent, DuelMove, TrainingView } from '../../generated/proto/game/v1/training_pb';
import { hasTranslation, playerName, t } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';
// The board comes with the offline bundle the app already keeps, not as a second copy.
import {
  boardStyles,
  CardDialog,
  ChronicleDialog,
  cardName,
  createSimulation,
  DecisionTable,
  errorText,
  eventText,
  legendOpen,
  loadPictures,
  quietEvents,
  type RoundRecord,
  SimulationBoard,
  useDuelKeys,
} from '../offline';

const move = (value: Partial<DuelMove>): DuelMove => ({
  $typeName: 'game.v1.DuelMove',
  attacks: [],
  defenses: [],
  action: 0,
  handIndex: 0,
  ...value,
});

// The same board as offline training, driven by the server: every resolved round is simulated
// from the view at its start, and the opponent's previous settings sit between the trays.
export function OnlineDuel(props: {
  snapshot: DuelSnapshot;
  end?: DuelEnd;
  connected: boolean;
  now: number;
  send: (body: ClientBody) => Promise<ServerBody | undefined>;
  close: () => void;
  replay?: boolean;
  tutorial?: boolean;
}) {
  const view = () => props.snapshot.view;
  const [draft, setDraft] = createSignal(move({}));
  const [preview, setPreview] = createSignal<DuelMovePreview>();
  const [sending, setSending] = createSignal(false);
  const [confirm, setConfirm] = createSignal(false);
  const [inspect, setInspect] = createSignal<Card>();
  const [showChronicle, setShowChronicle] = createSignal(false);
  // Resolved rounds, newest first; `events` is the newest one and `start` the view before it.
  const [history, setHistory] = createSignal<RoundRecord[]>([]);
  const [events, setEvents] = createSignal<DuelEvent[]>([]);
  const [start, setStart] = createSignal<TrainingView>();
  const sim = createSimulation();
  const titleId = createUniqueId();
  let root!: HTMLElement;
  let currentRound = '';
  let currentDuel = '';
  let lastResolved = 0;
  let lastView: TrainingView | undefined;
  let disposed = false;
  let generation = 0;
  onMount(() => {
    loadPictures();
    if (!props.replay) root.scrollIntoView({ block: 'start' });
  });
  const finished = () => !!view()?.outcome;
  const locked = () =>
    !props.connected ||
    props.snapshot.submitted ||
    finished() ||
    sending() ||
    !!props.replay ||
    sim.animating();
  const canSubmit = () => !locked() && !!preview()?.preview && !preview()?.preview?.error;
  const remaining = createMemo(() =>
    Math.max(0, Math.ceil((Number(props.snapshot.deadlineMs) - props.now) / 1000)),
  );
  const logins = (): [string, string] => [
    playerName(props.snapshot.you?.login ?? ''),
    playerName(props.snapshot.opponent?.login ?? ''),
  ];
  const avatarIds = (): [number, number] => [
    props.snapshot.you?.pathId || props.snapshot.you?.templateId || 0,
    props.snapshot.opponent?.pathId || props.snapshot.opponent?.templateId || 0,
  ];
  const text = (event: DuelEvent) => eventText(event, logins());
  const log = () =>
    [...history()]
      .reverse()
      .map((record) => ({
        round: record.round,
        events: record.events.filter(
          (e) => !quietEvents.includes(e.kind) && hasTranslation(`event-${e.kind}`),
        ),
      }))
      .filter((record) => record.events.length);
  createEffect(() => {
    const snapshot = props.snapshot;
    const current = snapshot.view;
    if (snapshot.duelId !== currentDuel) {
      currentDuel = snapshot.duelId;
      lastResolved = 0;
      lastView = undefined;
      sim.skip();
      setHistory([]);
      setEvents([]);
      setStart(undefined);
      setConfirm(false);
    }
    const key = `${snapshot.duelId}:${current?.round}`;
    if (key !== currentRound) {
      currentRound = key;
      setDraft(
        move({
          attacks: [...(snapshot.yourMove?.attacks ?? current?.me?.attacks ?? [])],
          defenses: [...(snapshot.yourMove?.defenses ?? current?.me?.defenses ?? [])],
          action: snapshot.yourMove?.action ?? 0,
          handIndex: snapshot.yourMove?.handIndex ?? 0,
        }),
      );
      setPreview(undefined);
    }
    // Every resolution starts with the reveal; a forfeit alone only appends the end of the duel.
    if (snapshot.events.some((e) => e.kind === 'reveal')) {
      const next = snapshot.events.find((e) => e.kind === 'round')?.amount;
      const round = Math.max(1, next === undefined ? (current?.round ?? 1) : next - 1);
      if (props.replay || round > lastResolved) {
        lastResolved = round;
        // The previous snapshot is the round start when the duel (or a replay) moved one round on.
        // After a reload there is none: the round can still be replayed, without the health drop.
        const begin = lastView?.round === round ? lastView : undefined;
        setHistory((rows) =>
          props.replay
            ? [{ round, events: snapshot.events }]
            : [{ round, events: snapshot.events }, ...rows.filter((r) => r.round !== round)],
        );
        setEvents(snapshot.events);
        setStart(begin);
        untrack(() => (begin ? void sim.animate(snapshot.events, begin) : sim.skip()));
      }
    } else if (props.replay) {
      sim.skip();
      setHistory([]);
      setEvents([]);
    }
    lastView = current;
    if (!props.replay) {
      setDuelActive(!finished());
      setOnlineDuelActive(!finished());
    }
  });
  createEffect(() => {
    const mv = draft();
    const snapshot = props.snapshot;
    const request = ++generation;
    setPreview(undefined);
    if (locked() || mv.attacks.length !== 3) return;
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
    if (!props.replay) {
      setDuelActive(false);
      setOnlineDuelActive(false);
    }
  });
  async function submit() {
    if (!canSubmit()) return;
    setSending(true);
    try {
      const response = await props.send({
        case: 'duelSubmitMove',
        value: {
          $typeName: 'game.v1.DuelSubmitMove',
          duelId: props.snapshot.duelId,
          round: view()?.round ?? 1,
          move: draft(),
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
  const replayRound = () => void sim.animate(events(), start());
  const change = (next: DuelMove) => {
    if (!locked()) setDraft(next);
  };
  useDuelKeys({
    ignore: () => !!inspect() || showChronicle() || confirm() || legendOpen(),
    animating: sim.animating,
    skip: sim.skip,
    canReplay: () => events().length > 0 && !sim.animating(),
    replay: replayRound,
    locked,
    fixed: () => false,
    pending: () => false,
    view,
    draft,
    castOptions: () => preview()?.castOptions ?? [],
    change,
    inspect: setInspect,
    submit: () => void submit(),
  });
  const opponentStatus = () =>
    !props.snapshot.opponentConnected
      ? t('online-opponent-offline', {
          seconds: Math.max(0, Math.ceil((Number(props.snapshot.graceUntilMs) - props.now) / 1000)),
        })
      : t(props.snapshot.opponentReady ? 'online-opponent-ready' : 'online-opponent-thinking');
  const title = (round: number) => (
    <h2 id={titleId} class={boardStyles.boardTitle}>
      {t(props.replay ? 'online-replay' : 'online-duel')} ·{' '}
      {t('online-round', { round, total: view()?.totalRounds ?? 5 })}
    </h2>
  );
  return (
    <section
      class={`${boardStyles.duel} ${boardStyles.embedded}`}
      aria-label={t('online-duel')}
      data-testid="online-duel"
      ref={root}
    >
      <Show when={view()}>
        {(state) => (
          <Show
            when={sim.animating()}
            fallback={
              <DecisionTable
                view={state()}
                plates={logins()}
                title={title(state().round)}
                heading={
                  <h2>{t('online-round', { round: state().round, total: state().totalRounds })}</h2>
                }
                labelledBy={titleId}
                avatars={avatarIds()}
                labels={logins()}
                draft={draft()}
                preview={preview()?.preview}
                castOptions={preview()?.castOptions ?? []}
                previous={state().round > 1}
                locked={locked()}
                pending={false}
                clock={`${remaining()} s`}
                clockLabel={t('online-deadline', { seconds: remaining() })}
                result={
                  <section class={`${boardStyles.result} online-result`} role="status">
                    <h2>
                      {t(
                        state().outcome?.winner === 0
                          ? 'online-draw'
                          : state().outcome?.winner === 1
                            ? 'online-win'
                            : 'online-loss',
                      )}
                    </h2>
                    <p>{t(`online-reason-${state().outcome?.reason}`)}</p>
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
                            {t('online-reward', {
                              xp: end().xpGained,
                              rating: end().ratingNew,
                              delta: `${end().ratingDelta >= 0 ? '+' : ''}${end().ratingDelta}`,
                              ducats: end().ducatsGained,
                            })}
                          </p>
                          <Show when={end().unlockedTemplate}>
                            <p>
                              {t('online-unlocked', {
                                avatar: t(
                                  fluentKey(avatars.get(end().unlockedTemplate)?.key ?? ''),
                                ),
                              })}
                            </p>
                          </Show>
                        </>
                      )}
                    </Show>
                    <Show when={finished() && !props.replay}>
                      <button type="button" class="primary" onClick={props.close}>
                        {t('close')}
                      </button>
                    </Show>
                  </section>
                }
                message={
                  <Show when={!finished() && !props.replay}>
                    {props.snapshot.submitted
                      ? t('online-waiting')
                      : preview()?.preview?.error
                        ? errorText(preview()?.preview?.error ?? '')
                        : draft().action
                          ? `${t(draft().action === 1 ? 'cast' : 'discard')}: ${cardName(state().hand[draft().handIndex] ?? 0)}`
                          : // The tutorial's single hint replaces the usual prompt, inside the board.
                            t(
                              !props.tutorial
                                ? 'choose-sequence'
                                : state().round === 1
                                  ? 'online-tutorial-first'
                                  : 'online-tutorial-next',
                            )}{' '}
                    · {opponentStatus()}
                  </Show>
                }
                buttons={
                  <>
                    <Show when={!finished() && !props.replay}>
                      <button
                        type="button"
                        class="quiet"
                        disabled={locked()}
                        onClick={() => setConfirm(true)}
                      >
                        {t('online-forfeit')}
                      </button>
                    </Show>
                    {/* The chronicle and the replay exist from the second round on. */}
                    <Show when={events().length}>
                      <button
                        type="button"
                        class="secondary"
                        onClick={() => setShowChronicle(true)}
                      >
                        {t('chronicle')}
                      </button>
                      <button type="button" class="secondary" onClick={replayRound}>
                        {t('replay')}
                      </button>
                    </Show>
                    <Show
                      when={!finished() && !props.replay}
                      fallback={
                        <Show when={props.replay}>
                          <button type="button" class="primary" onClick={props.close}>
                            {t('close')}
                          </button>
                        </Show>
                      }
                    >
                      <button
                        type="button"
                        class="primary"
                        disabled={!canSubmit()}
                        onClick={() => void submit()}
                      >
                        {t('online-submit')}
                        <span aria-hidden="true">✓</span>
                      </button>
                    </Show>
                  </>
                }
                onChange={change}
                onInspect={setInspect}
              />
            }
          >
            <SimulationBoard
              sim={sim}
              view={start() ?? state()}
              start={start()}
              plates={logins()}
              title={title(history()[0]?.round ?? state().round)}
              labelledBy={titleId}
              avatars={avatarIds()}
              labels={logins()}
              events={events()}
              history={history()}
              text={text}
              message={
                <>
                  {t('simulation')}
                  <Show when={!finished() && !props.replay}>
                    {' · '}
                    {t('online-deadline', { seconds: remaining() })}
                  </Show>
                </>
              }
            />
          </Show>
        )}
      </Show>
      {/* A live duel has its chronicle; the written log belongs to replays. */}
      <Show when={props.replay && log().length}>
        <details>
          <summary>{t('online-log')}</summary>
          <For each={log()}>
            {(r) => (
              <div>
                <h3>{t('online-round', { round: r.round, total: view()?.totalRounds ?? 5 })}</h3>
                <For each={r.events}>{(event) => <p>{text(event)}</p>}</For>
              </div>
            )}
          </For>
        </details>
      </Show>
      <Show when={showChronicle()}>
        <ChronicleDialog
          history={history()}
          events={events()}
          text={text}
          close={() => setShowChronicle(false)}
        />
      </Show>
      <Show when={inspect()}>
        {(card) => <CardDialog card={card()} close={() => setInspect(undefined)} />}
      </Show>
      <Show when={confirm()}>
        <Modal title={t('online-forfeit')} close={() => setConfirm(false)}>
          <p>{t('online-forfeit-confirm')}</p>
          <div class="modal-actions">
            <button type="button" class="secondary" onClick={() => setConfirm(false)}>
              {t('cancel')}
            </button>
            <button
              type="button"
              class="primary"
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
          </div>
        </Modal>
      </Show>
    </section>
  );
}

import '../../app/cardCatalog';
