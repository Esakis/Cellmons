import { createEffect, createSignal, For, lazy, onCleanup, Show, untrack } from 'solid-js';
import { ApiError, account, accountEpoch, api } from '../../app/account';
import { avatars, content, portrait } from '../../app/content';
import { onlineIcon } from '../../app/onlineIcons';
import { Modal } from '../../components/Modal';
import { addTranslations, hasTranslation, language, t } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import type { Mutation, Snapshot } from './types';
import './expeditions.css';

addTranslations({ pl, en });
const ExpeditionCard = lazy(() =>
  import('../../app/CardCatalogView').then((m) => ({ default: m.ExpeditionCard })),
);
export function Expeditions(props: {
  avatar: string;
  verified: boolean;
  active: boolean;
  connected: boolean;
  room: string;
  send: (body: ClientBody) => Promise<ServerBody | undefined>;
}) {
  const [data, setData] = createSignal<Snapshot>();
  const [selected, setSelected] = createSignal<string>();
  const [difficulty, setDifficulty] = createSignal('walk');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [pending, setPending] = createSignal<Mutation>();
  const [confirm, setConfirm] = createSignal<Mutation>();
  let disposed = false;
  let sequence = 0;
  let inFlight = false;
  let queued = false;
  let identity = '';
  const run = () => data()?.run;
  const user = () => {
    const current = account();
    return current?.kind === 'account' ? current.profile.id : '';
  };
  function failure(reason: unknown) {
    const code = reason instanceof ApiError ? reason.code : 'UNAVAILABLE';
    setError(t(hasTranslation(`exp-error-${code}`) ? `exp-error-${code}` : 'exp-error-default'));
  }
  async function load() {
    if (disposed || !user()) return;
    if (inFlight) {
      queued = true;
      return;
    }
    inFlight = true;
    const request = ++sequence;
    const epoch = accountEpoch();
    try {
      const value = await api<Snapshot>(`/api/expeditions${selected() ? `?id=${selected()}` : ''}`);
      if (disposed || request !== sequence || epoch !== accountEpoch()) return;
      if (value.content_version !== content.version) throw new ApiError('CONTENT_MISMATCH');
      setData(value);
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) failure(reason);
    } finally {
      inFlight = false;
      if (queued) {
        queued = false;
        void load();
      }
    }
  }
  createEffect(() => {
    const key = `${accountEpoch()}:${user()}`;
    if (identity === key) return;
    identity = key;
    sequence++;
    setData(undefined);
    setSelected(undefined);
    setPending(undefined);
    setConfirm(undefined);
    setError('');
    if (user()) void load();
  });
  createEffect(() => {
    if (props.active && props.connected) untrack(() => void load());
  });
  const timer = window.setInterval(() => {
    if (props.active && document.visibilityState === 'visible') void load();
  }, 5000);
  onCleanup(() => {
    disposed = true;
    sequence++;
    clearInterval(timer);
  });
  async function mutate(mutation: Mutation) {
    if (loading() || !user()) return;
    setLoading(true);
    setPending(mutation);
    setError('');
    const epoch = accountEpoch();
    try {
      await api('/api/expeditions', mutation);
      if (disposed || epoch !== accountEpoch()) return;
      setPending(undefined);
      sequence++;
      setSelected(mutation.id);
      await load();
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) {
        if (
          reason instanceof ApiError &&
          !['SERVER_UNAVAILABLE', 'INTERNAL', 'UNAVAILABLE', 'TIMEOUT'].includes(reason.code)
        )
          setPending(undefined);
        failure(reason);
        await load();
      }
    } finally {
      if (!disposed && epoch === accountEpoch()) setLoading(false);
    }
  }
  const percent = (bps: number) => (bps / 100).toLocaleString(language());
  const avatarName = (id: number) => {
    const avatar = avatars.get(id);
    return avatar ? t(`${avatar.key.replace('.', '-')}-name`) : t('exp-opponent');
  };
  const busy = () => loading() || !!pending() || !props.connected;
  return (
    <section class="expeditions" data-testid="expeditions">
      <h2>{t('exp-title')}</h2>
      <p>{t('exp-help')}</p>
      <p>
        {t('exp-rewards', {
          avatar: percent(data()?.avatar_chance_bps ?? 4000),
          card: percent(data()?.card_chance_bps ?? 300),
          ducats: percent(data()?.ducats_chance_bps ?? 100),
        })}
      </p>
      <Show when={!props.verified}>
        <p>{t('exp-verify')}</p>
      </Show>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <Show when={pending()}>
        {(mutation) => (
          <p>
            <button type="button" disabled={loading()} onClick={() => void mutate(mutation())}>
              {t('exp-retry')}
            </button>
          </p>
        )}
      </Show>
      <button type="button" disabled={loading()} onClick={() => void load()}>
        {t('refresh')}
      </button>
      <Show when={run()?.status !== 'active'}>
        <fieldset disabled={busy() || !props.verified || !props.avatar}>
          <legend>{t('exp-difficulty')}</legend>
          <For each={data()?.difficulties}>
            {(option) => (
              <label class="exp-difficulty">
                <input
                  name="difficulty"
                  type="radio"
                  value={option.id}
                  disabled={option.id === 'ordeal' && (data()?.account_level ?? 1) < 25}
                  checked={difficulty() === option.id}
                  onChange={() => setDifficulty(option.id)}
                />
                <strong>{t(`exp-difficulty-${option.id}`)}</strong>
                <span>
                  {t('exp-offer', {
                    stages: option.stages,
                    fixed: option.fixed_ducats,
                    random: option.random_ducats,
                    rarity: t(`exp-rarity-${option.rarity}`),
                  })}
                </span>
              </label>
            )}
          </For>
          <button
            type="button"
            onClick={() =>
              void mutate({
                action: 'start',
                id: crypto.randomUUID(),
                avatar: props.avatar,
                difficulty: difficulty(),
              })
            }
          >
            {t('exp-start')}
          </button>
        </fieldset>
      </Show>
      <Show when={run()}>
        {(current) => (
          <article class="exp-run">
            <h3>
              {t(`exp-difficulty-${current().difficulty}`)} · {t(`exp-status-${current().status}`)}
            </h3>
            <p>
              <img class="icon" src={onlineIcon('xp')} alt="" />
              {t('exp-progress', {
                stage: Math.min(current().stage + 1, current().stages),
                total: current().stages,
                xp: current().xp_accum,
              })}
            </p>
            <Show when={current().status === 'active'}>
              <p>{t('exp-escrow', { xp: current().retreat_xp })}</p>
              <Show when={props.room !== 'expedition'}>
                <button
                  type="button"
                  disabled={busy()}
                  onClick={() =>
                    void props.send({
                      case: 'roomJoin',
                      value: { $typeName: 'game.v1.RoomJoin', room: 'expedition' },
                    })
                  }
                >
                  {t('exp-chat')}
                </button>
              </Show>
              <Show
                when={current().active_duel}
                fallback={
                  <>
                    <div class="exp-choices">
                      <For each={current().choices}>
                        {(enemy) => (
                          <article class={`exp-enemy exp-enemy-${enemy.color}`}>
                            <Show when={avatars.get(enemy.template_id)}>
                              {(avatar) => (
                                <img src={portrait(avatar().key)} width="72" height="72" alt="" />
                              )}
                            </Show>
                            <h4>{avatarName(enemy.template_id)}</h4>
                            <p>
                              {t('exp-enemy-level', { level: enemy.level })} ·{' '}
                              {t(`exp-color-${enemy.color}`)}
                            </p>
                            <button
                              type="button"
                              disabled={busy() || !props.verified}
                              onClick={() =>
                                void mutate({
                                  action: 'choose',
                                  id: current().id,
                                  request: crypto.randomUUID(),
                                  revision: current().revision,
                                  choice: enemy.choice,
                                })
                              }
                            >
                              {t(enemy.boss ? 'exp-fight-boss' : 'exp-fight', {
                                name: avatarName(enemy.template_id),
                              })}
                            </button>
                          </article>
                        )}
                      </For>
                    </div>
                    <button
                      type="button"
                      disabled={busy()}
                      onClick={() =>
                        setConfirm({
                          action: 'retreat',
                          id: current().id,
                          request: crypto.randomUUID(),
                          revision: current().revision,
                        })
                      }
                    >
                      {t('exp-retreat')}
                    </button>
                  </>
                }
              >
                <p>{t('exp-in-fight')}</p>
                <button
                  type="button"
                  disabled={busy()}
                  onClick={() =>
                    void props.send({
                      case: 'duelSnapshot',
                      value: {
                        $typeName: 'game.v1.DuelTarget',
                        duelId: current().active_duel ?? '',
                      },
                    })
                  }
                >
                  {t('exp-resume')}
                </button>
              </Show>
            </Show>
            <Show when={current().award}>
              {(award) => (
                <div role="status" class="exp-award">
                  <p>
                    <img class="icon" src={onlineIcon('ducats')} alt="" />
                    {t('exp-award', { xp: award().xp, ducats: award().ducats })}
                  </p>
                  <Show when={award().template}>
                    {(id) => <p>{t('exp-award-avatar', { name: avatarName(id()) })}</p>}
                  </Show>
                  <Show when={award().card}>{(id) => <ExpeditionCard id={id()} />}</Show>
                </div>
              )}
            </Show>
          </article>
        )}
      </Show>
      <Show when={confirm()}>
        {(mutation) => (
          <Modal title={t('exp-retreat')} close={() => setConfirm(undefined)}>
            <p>{t('exp-retreat-confirm', { xp: run()?.retreat_xp ?? 0 })}</p>
            <button
              type="button"
              onClick={() => {
                const value = mutation();
                setConfirm(undefined);
                void mutate(value);
              }}
            >
              {t('exp-confirm')}
            </button>
          </Modal>
        )}
      </Show>
    </section>
  );
}
