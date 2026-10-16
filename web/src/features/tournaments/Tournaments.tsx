import { createEffect, createSignal, For, lazy, onCleanup, Show, untrack } from 'solid-js';
import { ApiError, account, accountEpoch, api } from '../../app/account';
import { avatars, content, fluentKey } from '../../app/content';
import { Modal } from '../../components/Modal';
import { addTranslations, hasTranslation, t } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import type { Mutation, Pair, Profile, Snapshot, Terms } from './types';
import './tournaments.css';

addTranslations({ pl, en });
const TournamentCard = lazy(() =>
  import('../../app/CardCatalogView').then((m) => ({ default: m.TournamentCard })),
);
export function Tournaments(props: {
  send: (body: ClientBody) => Promise<ServerBody | undefined>;
  connected: boolean;
  avatar: string;
  active: boolean;
}) {
  const [data, setData] = createSignal<Snapshot>();
  const [selected, setSelected] = createSignal<string>();
  const [page, setPage] = createSignal(0);
  const [error, setError] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [pending, setPending] = createSignal<Mutation>();
  const [confirm, setConfirm] = createSignal<Mutation>();
  const [showCreate, setShowCreate] = createSignal(false);
  const [clanMode, setClanMode] = createSignal(false);
  const [allowed, setAllowed] = createSignal<string[]>([]);
  const [tiers, setTiers] = createSignal([0, 1, 2, 3]);
  let disposed = false;
  let sequence = 0;
  let identity = '';
  let inFlight = false;
  let queued = false;
  const user = () => {
    const current = account();
    return current?.kind === 'account' ? current.profile : undefined;
  };
  const profile = () => data()?.profile;
  const ownEntry = () =>
    profile()?.entries.find((e) => e.user === user()?.id && e.status === 'enrolled');
  function failure(reason: unknown) {
    const code = reason instanceof ApiError ? reason.code : 'UNAVAILABLE';
    setError(
      t(
        hasTranslation(`tourney-error-${code}`) ? `tourney-error-${code}` : 'tourney-error-default',
      ),
    );
  }
  async function load() {
    if (disposed || account()?.kind !== 'account') return;
    if (inFlight) {
      queued = true;
      return;
    }
    inFlight = true;
    const request = ++sequence;
    const epoch = accountEpoch();
    try {
      const value = await api<Snapshot>(
        `/api/tournaments?page=${page()}${selected() ? `&id=${selected()}` : ''}`,
      );
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
  async function pulse() {
    if (!props.active || !props.connected || document.visibilityState !== 'visible' || !data()?.own)
      return;
    await props.send({
      case: 'tournamentReady',
      value: { $typeName: 'game.v1.TournamentReady', ready: true },
    });
  }
  createEffect(() => {
    const current = user();
    const key = `${accountEpoch()}:${current?.id}`;
    if (key === identity) return;
    identity = key;
    sequence++;
    setData(undefined);
    setPending(undefined);
    setConfirm(undefined);
    setSelected(undefined);
    setPage(0);
    setError('');
    if (current) void load();
  });
  createEffect(() => {
    if (data()?.own && props.connected) {
      if (props.active) void pulse();
      else
        void props.send({
          case: 'tournamentReady',
          value: { $typeName: 'game.v1.TournamentReady', ready: false },
        });
    }
  });
  createEffect(() => {
    if (props.active && props.connected) untrack(() => void load());
  });
  const timer = window.setInterval(() => {
    if (props.active && document.visibilityState === 'visible') {
      void load();
      void pulse();
    }
  }, 5000);
  onCleanup(() => {
    disposed = true;
    sequence++;
    clearInterval(timer);
    if (props.connected && data()?.own)
      void props.send({
        case: 'tournamentReady',
        value: { $typeName: 'game.v1.TournamentReady', ready: false },
      });
  });
  function select(id?: string) {
    sequence++;
    setSelected(id);
    setError('');
    void load();
  }
  async function mutate(mutation: Mutation) {
    if (loading() || !user()) return;
    setPending(mutation);
    setLoading(true);
    setError('');
    const epoch = accountEpoch();
    try {
      await api<string>('/api/tournaments', mutation);
      if (disposed || epoch !== accountEpoch()) return;
      setPending(undefined);
      setSelected(mutation.id);
      if (mutation.action === 'create') setShowCreate(false);
      await load();
      await pulse();
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
  function create(event: SubmitEvent) {
    event.preventDefault();
    const form = new FormData(event.currentTarget as HTMLFormElement);
    const terms: Terms = {
      teams: clanMode() ? allowed().length : Number(form.get('teams')),
      team_size: clanMode() ? Number(form.get('team_size')) : 1,
      fee: Number(form.get('fee')),
      max_games: Number(form.get('max_games')),
      min_level: Number(form.get('min_level')),
      max_level: Number(form.get('max_level')),
      tiers: tiers(),
      only_clan: form.get('only_clan') ? (data()?.own_clan ?? null) : null,
      allowed_clans: clanMode() ? allowed() : [],
    };
    void mutate({
      action: 'create',
      id: crypto.randomUUID(),
      name: String(form.get('name')),
      terms,
    });
  }
  const teamName = (id: string | null, p = profile()) =>
    id
      ? (data()?.clans.find((c) => c.id === id)?.name ??
        p?.entries.find((e) => e.team === id)?.login ??
        t('tourney-team'))
      : t('tourney-bye');
  const points = (pair: Pair, side: 0 | 1) => pair.lanes.reduce((n, l) => n + l.points[side], 0);
  const canJoin = (p: Profile) =>
    p.summary.status === 'registering' &&
    !data()?.own &&
    Boolean(props.avatar) &&
    user()?.email_verified;
  return (
    <section class="tournaments" data-testid="tournaments">
      <h2>{t('tourney-title')}</h2>
      <p>
        {t('tourney-help', {
          first: data()?.first_fight_s ?? 20,
          next: data()?.next_fight_s ?? 15,
          grace: data()?.ready_grace_s ?? 90,
        })}
      </p>
      <Show when={!user()?.email_verified}>
        <p>{t('tourney-verify')}</p>
      </Show>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <Show when={pending()}>
        {(mutation) => (
          <p>
            <button type="button" disabled={loading()} onClick={() => void mutate(mutation())}>
              {t('tourney-retry')}
            </button>
          </p>
        )}
      </Show>
      <div class="tourney-actions">
        <button type="button" disabled={loading()} onClick={() => void load()}>
          {t('refresh')}
        </button>
        <Show when={data()?.own}>
          <button type="button" onClick={() => select()}>
            {t('tourney-own')}
          </button>
        </Show>
        <button
          type="button"
          disabled={!user()?.email_verified || loading() || !!pending()}
          aria-expanded={showCreate()}
          onClick={() => setShowCreate((v) => !v)}
        >
          {t('tourney-create')}
        </button>
      </div>
      <Show when={showCreate()}>
        <form class="tourney-create" onSubmit={create}>
          <fieldset disabled={loading() || !!pending()}>
            <legend>{t('tourney-create')}</legend>
            <label>
              {t('tourney-name')}
              <input name="name" required minLength={3} maxLength={60} />
            </label>
            <label>
              <input
                type="checkbox"
                checked={clanMode()}
                onChange={(e) => setClanMode(e.currentTarget.checked)}
              />
              {t('tourney-clan-mode')}
            </label>
            <Show
              when={clanMode()}
              fallback={
                <>
                  <label>
                    {t('tourney-players')}
                    <input name="teams" type="number" min="2" max="64" value="8" required />
                  </label>
                  <Show when={data()?.own_clan}>
                    <label>
                      <input name="only_clan" type="checkbox" />
                      {t('tourney-only-clan')}
                    </label>
                  </Show>
                </>
              }
            >
              <label>
                {t('tourney-team-size')}
                <input name="team_size" type="number" min="2" max="16" value="4" required />
              </label>
              <fieldset class="tourney-clans">
                <legend>{t('tourney-allowed-clans')}</legend>
                <For each={data()?.clans}>
                  {(clan) => (
                    <label>
                      <input
                        type="checkbox"
                        checked={allowed().includes(clan.id)}
                        onChange={(e) =>
                          setAllowed((a) =>
                            e.currentTarget.checked
                              ? [...a, clan.id]
                              : a.filter((id) => id !== clan.id),
                          )
                        }
                      />
                      {clan.name}
                    </label>
                  )}
                </For>
              </fieldset>
            </Show>
            <label>
              {t('tourney-fee')}
              <input name="fee" type="number" min="0" max="1000000" value="0" required />
            </label>
            <label>
              {t('tourney-games')}
              <input name="max_games" type="number" min="1" max="9" value="3" required />
            </label>
            <label>
              {t('tourney-min-level')}
              <input name="min_level" type="number" min="1" max="20" value="1" required />
            </label>
            <label>
              {t('tourney-max-level')}
              <input name="max_level" type="number" min="1" max="20" value="20" required />
            </label>
            <fieldset>
              <legend>{t('tourney-tiers')}</legend>
              <For each={[0, 1, 2, 3]}>
                {(tier) => (
                  <label>
                    <input
                      type="checkbox"
                      checked={tiers().includes(tier)}
                      onChange={(e) =>
                        setTiers((a) =>
                          e.currentTarget.checked ? [...a, tier] : a.filter((t) => t !== tier),
                        )
                      }
                    />
                    {t(`tourney-tier-${tier}`)}
                  </label>
                )}
              </For>
            </fieldset>
            <p>{t('tourney-terms')}</p>
            <button type="submit">{t('tourney-publish')}</button>
          </fieldset>
        </form>
      </Show>
      <ul class="tourney-catalog">
        <For each={data()?.catalog}>
          {(item) => (
            <li>
              <button type="button" onClick={() => select(item.id)}>
                {item.name}
              </button>{' '}
              · {t(`tourney-status-${item.status}`)} · {item.enrolled} · {item.balance} D
            </li>
          )}
        </For>
      </ul>
      <div class="tourney-actions">
        <button
          type="button"
          disabled={page() === 0}
          onClick={() => {
            setPage((p) => p - 1);
            void load();
          }}
        >
          {t('tourney-previous')}
        </button>
        <span>{page() + 1}</span>
        <button
          type="button"
          disabled={!data()?.has_more}
          onClick={() => {
            setPage((p) => p + 1);
            void load();
          }}
        >
          {t('tourney-next')}
        </button>
      </div>
      <Show when={profile()}>
        {(p) => (
          <article class="tourney-profile">
            <h3>{p().summary.name}</h3>
            <Show when={p().reward}>
              {(reward) => (
                <div class="tourney-prize">
                  <p>{t('tourney-system-prize')}</p>
                  <Show when={reward().kind === 'ducats'}>
                    <p>
                      {t('tourney-prize-ducats', {
                        amount: (reward() as { amount: number }).amount,
                      })}
                    </p>
                  </Show>
                  <Show when={reward().kind === 'avatar'}>
                    <p>
                      {t('tourney-prize-avatar', {
                        name: t(
                          fluentKey(
                            avatars.get((reward() as { template: number }).template)?.key ?? '',
                          ),
                        ),
                      })}
                    </p>
                  </Show>
                  <Show when={reward().kind === 'card'}>
                    <TournamentCard id={(reward() as { card: number }).card} />
                  </Show>
                  <Show when={reward().kind === 'booster'}>
                    <p>
                      {t('tourney-prize-booster', {
                        name: (reward() as { booster: string }).booster,
                      })}
                    </p>
                  </Show>
                  <p>{t('tourney-prize-policy')}</p>
                </div>
              )}
            </Show>
            <p>
              {t(`tourney-status-${p().summary.status}`)} · {t('tourney-pot')}:{' '}
              {p().summary.balance} D
            </p>
            <p>
              {t('tourney-conditions', {
                players: p().terms.teams * p().terms.team_size,
                min: p().terms.min_level,
                max: p().terms.max_level,
                games: p().terms.max_games,
                fee: p().terms.fee,
              })}
            </p>
            <p>
              {t('tourney-eligible-tiers')}:{' '}
              {p()
                .terms.tiers.map((tier) => t(`tourney-tier-${tier}`))
                .join(', ')}
            </p>
            <Show when={p().terms.only_clan}>
              <p>
                {t('tourney-only-clan')}: {teamName(p().terms.only_clan)}
              </p>
            </Show>
            <Show when={p().terms.team_size > 1}>
              <p>
                {t('tourney-clan-roster', { size: p().terms.team_size })}{' '}
                {p()
                  .terms.allowed_clans.map((id) => teamName(id))
                  .join(', ')}
              </p>
            </Show>
            <p>{t('tourney-pin')}</p>
            <Show when={canJoin(p())}>
              <button
                type="button"
                disabled={loading() || !!pending()}
                onClick={() => {
                  const avatar = props.avatar;
                  if (avatar)
                    void mutate({
                      action: 'join',
                      id: p().summary.id,
                      request: crypto.randomUUID(),
                      avatar,
                    });
                }}
              >
                {t('tourney-join', { fee: p().terms.fee })}
              </button>
            </Show>
            <Show when={ownEntry() && !ownEntry()?.departed}>
              <p>{t('tourney-ready')}</p>
              <button
                type="button"
                disabled={loading() || !!pending()}
                onClick={() =>
                  setConfirm({ action: 'leave', id: p().summary.id, request: crypto.randomUUID() })
                }
              >
                {t('tourney-leave')}
              </button>
            </Show>
            <Show
              when={
                !p().reward &&
                p().summary.creator === user()?.id &&
                p().summary.status === 'registering'
              }
            >
              <button
                type="button"
                disabled={loading() || !!pending()}
                onClick={() =>
                  setConfirm({ action: 'cancel', id: p().summary.id, request: crypto.randomUUID() })
                }
              >
                {t('tourney-cancel')}
              </button>
            </Show>
            <Show when={p().champion}>
              <p role="status">
                {t('tourney-champion')}: {teamName(p().champion)}
              </p>
            </Show>
            <details>
              <summary>{t('tourney-roster')}</summary>
              <ul>
                <For each={p().entries}>
                  {(entry) => (
                    <li>
                      {entry.login} · {t('lobby-level-label')} {entry.level} ·{' '}
                      {t(`tourney-tier-${entry.tier}`)} ·{' '}
                      {t(`tourney-entry-${entry.departed ? 'departed' : entry.status}`)}
                      {entry.ready ? ` · ${t('tourney-is-ready')}` : ''}
                    </li>
                  )}
                </For>
              </ul>
            </details>
            <For each={p().rounds}>
              {(round, index) => (
                <section>
                  <h4>{t('tourney-round', { round: index() + 1 })}</h4>
                  <ul>
                    <For each={round.pairs}>
                      {(pair) => (
                        <li>
                          {teamName(pair.teams[0])} — {teamName(pair.teams[1])}: {points(pair, 0)}:
                          {points(pair, 1)}{' '}
                          <Show when={pair.resolved}>
                            {' '}
                            · {t('tourney-advances')}: {teamName(pair.winner)}
                          </Show>
                        </li>
                      )}
                    </For>
                  </ul>
                </section>
              )}
            </For>
          </article>
        )}
      </Show>
      <Show when={confirm()}>
        {(mutation) => (
          <Modal title={t('tourney-confirm')} close={() => setConfirm(undefined)}>
            <p>
              {t(
                mutation().action === 'cancel'
                  ? 'tourney-cancel-help'
                  : profile()?.summary.status === 'registering'
                    ? 'tourney-refund-help'
                    : 'tourney-forfeit-help',
              )}
            </p>
            <button
              type="button"
              onClick={() => {
                const action = mutation();
                setConfirm(undefined);
                void mutate(action);
              }}
            >
              {t('tourney-confirm')}
            </button>
          </Modal>
        )}
      </Show>
    </section>
  );
}
