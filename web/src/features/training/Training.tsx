import { A, useBeforeLeave, useSearchParams } from '@solidjs/router';
import { batch, createEffect, createSignal, onCleanup, onMount, Show } from 'solid-js';
import { type Card, content, isPlayableAvatar } from '../../app/content';
import { readPreference, writePreference } from '../../app/preferences';
import { setDuelActive } from '../../app/session';
import { Modal } from '../../components/Modal';
import type {
  DuelEvent,
  DuelMove,
  MovePreview,
  TrainingReply,
  TrainingView,
} from '../../generated/proto/game/v1/training_pb';
import { t } from '../../i18n';
import { TrainingClient } from '../../worker/client';
import {
  create,
  DuelMoveSchema,
  TrainingForfeitSchema,
  TrainingSetupSchema,
} from '../../worker/wire';
import {
  CardDialog,
  ChronicleDialog,
  cardName,
  createSimulation,
  DecisionTable,
  errorText,
  eventText,
  loadPictures,
  type RoundRecord,
  SimulationBoard,
  useDuelKeys,
} from './Board';
import styles from './Training.module.css';

const profiles = ['novice', 'apprentice', 'tactician', 'archmage'];

export function Duel() {
  const [params] = useSearchParams();
  const avatarId = () => (isPlayableAvatar(Number(params.avatar)) ? Number(params.avatar) : 1);
  const profile = () => (profiles.includes(String(params.bot)) ? String(params.bot) : 'novice');
  const demo = () => params.demo === '1';
  const [view, setView] = createSignal<TrainingView>();
  const [draft, setDraft] = createSignal(create(DuelMoveSchema));
  const [preview, setPreview] = createSignal<MovePreview>();
  const [castOptions, setCastOptions] = createSignal<MovePreview[]>([]);
  const [pendingPreview, setPendingPreview] = createSignal(false);
  const [busy, setBusy] = createSignal(true);
  const [error, setError] = createSignal('');
  const [inspect, setInspect] = createSignal<Card>();
  const [confirmLeave, setConfirmLeave] = createSignal(false);
  const [showChronicle, setShowChronicle] = createSignal(false);
  const [events, setEvents] = createSignal<DuelEvent[]>([]);
  const [history, setHistory] = createSignal<RoundRecord[]>([]);
  const sim = createSimulation();
  let lastRoundStart: TrainingView | undefined;
  const [seconds, setSeconds] = createSignal(0);
  let client: TrainingClient;
  let previewGeneration = 0;
  let leave: (() => void) | undefined;
  let disposed = false;
  const locked = () => busy() || sim.animating() || !!view()?.outcome;
  const labels = (): [string, string] => [t('you'), t('opponent')];
  const plates = (): [string, string] => [demo() ? 'AI' : t('you'), t(profile())];
  const text = (event: DuelEvent) => eventText(event, labels());

  function apply(reply: TrainingReply) {
    if (disposed || !reply.view?.me) return;
    previewGeneration++;
    batch(() => {
      setView(reply.view);
      setDraft(
        create(DuelMoveSchema, {
          attacks: [...(reply.view?.me?.attacks ?? [])],
          defenses: [...(reply.view?.me?.defenses ?? [])],
        }),
      );
      setPreview(reply.preview);
      setCastOptions(reply.castOptions);
      setPendingPreview(false);
      setSeconds(0);
      setDuelActive(!reply.view?.outcome);
    });
  }
  async function start() {
    if (error()) {
      client.dispose();
      client = new TrainingClient();
    }
    setBusy(true);
    setError('');
    sim.skip();
    setEvents([]);
    setHistory([]);
    try {
      const seed = crypto.getRandomValues(new Uint8Array(32));
      const rotation = Number(readPreference('rotation', '0'));
      const safeRotation = Number.isSafeInteger(rotation) && rotation >= 0 ? rotation : 0;
      const reply = await client.request({
        case: 'start',
        value: create(TrainingSetupSchema, {
          avatarId: avatarId(),
          profileId: profile(),
          seed,
          rotation: safeRotation % 6,
          autoplay: demo(),
          contentVersion: content.version,
        }),
      });
      apply(reply);
      writePreference('rotation', String((safeRotation + 1) % 6));
    } catch (e) {
      if (!disposed) setError(e instanceof Error ? e.message : 'generic');
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  async function updateDraft(next: DuelMove) {
    if (locked()) return;
    const generation = ++previewGeneration;
    batch(() => {
      setDraft(next);
      setPendingPreview(true);
      setError('');
    });
    try {
      const reply = await client.request({ case: 'preview', value: next });
      if (generation !== previewGeneration || disposed) return;
      batch(() => {
        setPreview(reply.preview);
        setCastOptions(reply.castOptions);
        setPendingPreview(false);
      });
    } catch (e) {
      if (!disposed && generation === previewGeneration) {
        setPendingPreview(false);
        setError(e instanceof Error ? e.message : 'generic');
      }
    }
  }
  async function submit() {
    if (locked() || pendingPreview() || preview()?.error) return;
    setBusy(true);
    setError('');
    try {
      lastRoundStart = view();
      const reply = await client.request({ case: 'play', value: draft() });
      if (disposed) return;
      setEvents(reply.events);
      const round = lastRoundStart?.round ?? 0;
      setHistory((rounds) => [{ round, events: reply.events }, ...rounds]);
      await sim.animate(reply.events, lastRoundStart);
      apply(reply);
    } catch (e) {
      if (!disposed) setError(e instanceof Error ? e.message : 'generic');
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  async function forfeit() {
    setConfirmLeave(false);
    sim.skip();
    setBusy(true);
    try {
      apply(await client.request({ case: 'forfeit', value: create(TrainingForfeitSchema) }));
      leave?.();
      leave = undefined;
    } catch (e) {
      if (!disposed) setError(e instanceof Error ? e.message : 'generic');
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  useBeforeLeave((event) => {
    if (!view()?.outcome && view() && !error()) {
      event.preventDefault();
      leave = () => event.retry(true);
      setConfirmLeave(true);
    }
  });
  function cancelLeave() {
    setConfirmLeave(false);
    leave = undefined;
  }
  useDuelKeys({
    ignore: () => !!inspect() || showChronicle() || confirmLeave(),
    animating: sim.animating,
    skip: sim.skip,
    canReplay: () => events().length > 0 && !busy() && !sim.animating(),
    replay: () => void sim.animate(events(), lastRoundStart),
    locked,
    fixed: demo,
    pending: pendingPreview,
    view,
    draft,
    castOptions,
    change: (move) => void updateDraft(move),
    inspect: setInspect,
    submit: () => void submit(),
  });
  onMount(() => {
    loadPictures();
    client = new TrainingClient();
    void start();
    const clock = setInterval(() => {
      if (!locked() && !document.hidden) setSeconds((s) => s + 1);
    }, 1000);
    onCleanup(() => clearInterval(clock));
  });
  onCleanup(() => {
    disposed = true;
    client?.dispose();
    setDuelActive(false);
  });
  createEffect(() => {
    if (view()?.outcome) document.getElementById('duel-result')?.focus();
  });

  return (
    <div class={styles.duel}>
      <Show
        when={view()}
        fallback={
          <div class={styles.loading}>
            <p role="status">{t(error() ? 'error-title' : 'loading')}</p>
            <Show when={error()}>
              <p role="alert">{errorText(error())}</p>
              <button type="button" onClick={() => void start()}>
                {t('retry')}
              </button>
              <A href="/">{t('back')}</A>
            </Show>
          </div>
        }
      >
        {(state) => (
          <>
            <Show
              when={sim.animating()}
              fallback={
                <DecisionTable
                  view={state()}
                  plates={plates()}
                  title={<p class={styles.boardTitle}>{t('duel-title')}</p>}
                  heading={
                    <h1 id="round-title">
                      {t('round', { round: state().round, total: state().totalRounds })}
                    </h1>
                  }
                  labelledBy="round-title"
                  avatars={[avatarId(), state().opponentAvatarId]}
                  labels={labels()}
                  draft={draft()}
                  preview={preview()}
                  castOptions={castOptions()}
                  previous={history().length > 0}
                  locked={locked() || demo()}
                  pending={pendingPreview()}
                  clock={seconds()}
                  clockLabel={t('elapsed', { seconds: seconds() })}
                  result={
                    <Show when={state().outcome}>
                      {(result) => (
                        <section
                          class={styles.result}
                          id="duel-result"
                          tabIndex={-1}
                          aria-labelledby="result-title"
                        >
                          <h2 id="result-title">
                            {t(
                              result().winner === 1
                                ? 'victory'
                                : result().winner === 2
                                  ? 'defeat'
                                  : 'draw',
                            )}
                          </h2>
                          <p>{t(`reason-${result().reason}`)}</p>
                          <button
                            type="button"
                            class="primary"
                            onClick={() => void start()}
                            disabled={busy()}
                          >
                            {t('again')}
                          </button>
                          <A class="button secondary" href="/">
                            {t('back')}
                          </A>
                        </section>
                      )}
                    </Show>
                  }
                  message={
                    preview()?.error
                      ? errorText(preview()?.error ?? '')
                      : draft().action
                        ? `${t(draft().action === 1 ? 'cast' : 'discard')}: ${cardName(state().hand[draft().handIndex] ?? 0)}`
                        : t('choose-sequence')
                  }
                  buttons={
                    <>
                      <Show when={!state().outcome}>
                        <button
                          type="button"
                          class="quiet"
                          disabled={busy()}
                          onClick={() => setConfirmLeave(true)}
                        >
                          {t('forfeit')}
                        </button>
                      </Show>
                      <button
                        type="button"
                        class="secondary"
                        onClick={() => setShowChronicle(true)}
                      >
                        {t('chronicle')}
                      </button>
                      <button
                        type="button"
                        class="secondary"
                        disabled={busy() || !events().length}
                        onClick={() => void sim.animate(events(), lastRoundStart)}
                      >
                        {t('replay')}
                      </button>
                      <Show when={!state().outcome}>
                        <button
                          type="button"
                          class="primary"
                          disabled={locked() || pendingPreview() || !!preview()?.error || !!error()}
                          onClick={() => void submit()}
                        >
                          {busy() ? t('loading') : t(demo() ? 'next-demo' : 'submit')}
                          <span aria-hidden="true">✓</span>
                        </button>
                      </Show>
                    </>
                  }
                  onChange={(move) => void updateDraft(move)}
                  onInspect={setInspect}
                />
              }
            >
              <SimulationBoard
                sim={sim}
                view={state()}
                start={lastRoundStart}
                plates={plates()}
                title={
                  <h1 id="board-title" class={styles.boardTitle}>
                    {t('duel-title')} ·{' '}
                    {t('round', { round: state().round, total: state().totalRounds })}
                  </h1>
                }
                labelledBy="board-title"
                avatars={[avatarId(), state().opponentAvatarId]}
                labels={labels()}
                events={events()}
                history={history()}
                text={text}
                message={t('simulation')}
              />
            </Show>
            <Show when={error()}>
              <p role="alert" class={styles.error}>
                {errorText(error())}
              </p>
              <A href="/">{t('back')}</A>
            </Show>
          </>
        )}
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
      <Show when={confirmLeave()}>
        <Modal title={t('forfeit-title')} close={cancelLeave}>
          <p>{t('forfeit-text')}</p>
          <div class="modal-actions">
            <button type="button" class="secondary" onClick={cancelLeave}>
              {t('cancel')}
            </button>
            <button type="button" class="primary" onClick={() => void forfeit()}>
              {t('forfeit')}
            </button>
          </div>
        </Modal>
      </Show>
    </div>
  );
}
