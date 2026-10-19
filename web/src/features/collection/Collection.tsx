import { A, useBeforeLeave } from '@solidjs/router';
import { createEffect, createMemo, createSignal, For, lazy, onCleanup, Show } from 'solid-js';
import {
  ApiError,
  account,
  accountEpoch,
  api,
  enterGuest,
  restoreAccount,
} from '../../app/account';
import {
  avatars,
  cardArgs,
  cards,
  content,
  fluentKey,
  icon,
  loadPaths,
  portrait,
  statIcons,
} from '../../app/content';
import { setTrainingPlayer } from '../../app/trainingPlayer';
import { BackToMap } from '../../components/BackToMap';
import { Modal } from '../../components/Modal';
import { Scene } from '../../components/Scene';
import { addTranslations, hasTranslation, t } from '../../i18n';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import { progressionDescription } from './progressionDescription';
import { guestMutation, guestState, type Mutation, type Snapshot } from './state';
import './collection.css';

addTranslations({ pl, en });
const Progression = lazy(() =>
  import('./Progression').then((module) => ({ default: module.Progression })),
);
const schools = ['combat', 'divine', 'element', 'arcane', 'necro', 'tech'];
// The collection's painting (art/paintings/scenes) and the card illustrations, which load with
// their own module as in the duel (app/cardArt.ts).
const [painting] = Object.values(
  import.meta.glob<string>('../../assets/paintings/scenes/collection.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);
const [pictures, setPictures] = createSignal(new Map<string, string>());

export function Collection() {
  if (!pictures().size) {
    void import('../../app/cardArt').then((module) => setPictures(module.cardPictures));
  }
  const [data, setData] = createSignal<Snapshot>();
  const [selected, setSelected] = createSignal('');
  const [draft, setDraft] = createSignal<number[]>([]);
  const [points, setPoints] = createSignal([0, 0, 0, 0, 0, 0]);
  const [template, setTemplate] = createSignal(0);
  const [query, setQuery] = createSignal('');
  const [visibleCount, setVisibleCount] = createSignal(24);
  const [rarity, setRarity] = createSignal('');
  const [castable, setCastable] = createSignal(false);
  const [ownedOnly, setOwnedOnly] = createSignal(false);
  const [presetName, setPresetName] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [notice, setNotice] = createSignal('');
  const [confirm, setConfirm] = createSignal<Mutation>();
  const own = () => data()?.avatars.find((a) => a.id === selected());
  const guest = () => account()?.kind === 'guest';
  const changed = () => JSON.stringify(draft()) !== JSON.stringify(own()?.deck ?? []);
  const spent = () => points().reduce((a, b) => a + b, 0);
  let key = '';
  let deckKey = '';
  let disposed = false;
  function failure(reason: unknown) {
    const code =
      reason instanceof ApiError
        ? reason.code
        : reason instanceof Error
          ? reason.message
          : 'INTERNAL';
    setError(
      t(
        hasTranslation(`collection-error-${code}`)
          ? `collection-error-${code}`
          : 'collection-error-default',
      ),
    );
  }
  async function load() {
    const epoch = accountEpoch();
    setLoading(true);
    setError('');
    try {
      if (!guest()) {
        await loadPaths();
        if (disposed || epoch !== accountEpoch()) return;
      }
      const snapshot = guest() ? guestState() : await api<Snapshot>('/api/game');
      if (snapshot.content_version !== content.version) throw new ApiError('CONTENT_MISMATCH');
      if (!disposed && epoch === accountEpoch()) setData(snapshot);
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) failure(reason);
    } finally {
      if (!disposed && epoch === accountEpoch()) setLoading(false);
    }
  }
  createEffect(() => {
    const current = account();
    const next = `${current?.kind}:${accountEpoch()}`;
    if (next === key) return;
    key = next;
    setData(undefined);
    if (current) void load();
    else if (navigator.onLine) void restoreAccount();
  });
  createEffect(() => {
    const rows = data()?.avatars;
    if (rows && !rows.some((a) => a.id === selected()))
      setSelected(rows.find((a) => a.active)?.id ?? rows[0]?.id ?? '');
    const avatar = own();
    const next = `${avatar?.id}:${avatar?.deck_revision}`;
    if (next !== deckKey) {
      deckKey = next;
      setDraft([...(avatar?.deck ?? [])]);
      setPoints([0, 0, 0, 0, 0, 0]);
    }
  });
  onCleanup(() => {
    disposed = true;
  });
  useBeforeLeave((event) => {
    if (changed() && !window.confirm(t('collection-unsaved'))) event.preventDefault();
  });
  async function mutate(mutation: Mutation) {
    const epoch = accountEpoch();
    setLoading(true);
    setError('');
    setNotice('');
    try {
      const snapshot = guest()
        ? guestMutation(mutation)
        : await api<Snapshot>('/api/game', mutation);
      if (snapshot.content_version !== content.version) throw new ApiError('CONTENT_MISMATCH');
      if (!disposed && epoch === accountEpoch()) {
        setData(snapshot);
        if (mutation.action === 'spend') setPoints([0, 0, 0, 0, 0, 0]);
        if (mutation.action === 'create') setSelected(mutation.id);
        if (mutation.action === 'shade') setSelected(mutation.new_id);
        if (mutation.action === 'reincarnate') setPoints([0, 0, 0, 0, 0, 0]);
        setNotice(t('collection-saved'));
      }
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) {
        failure(reason);
        if (!guest()) await load();
      }
    } finally {
      if (!disposed && epoch === accountEpoch()) setLoading(false);
    }
  }
  const visible = createMemo(() =>
    content.cards.filter((card) => {
      if (
        query() &&
        !t(fluentKey(card.key)).toLocaleLowerCase().includes(query().toLocaleLowerCase())
      )
        return false;
      if (rarity() && card.rarity !== rarity()) return false;
      if (ownedOnly() && !data()?.collection.some((s) => s.card_id === card.id && s.qty > 0))
        return false;
      if (
        castable() &&
        Object.entries(card.requires).some(
          ([school, level]) => (own()?.schools[schools.indexOf(school)] ?? 0) < level,
        )
      )
        return false;
      return true;
    }),
  );
  createEffect(() => {
    query();
    rarity();
    castable();
    ownedOnly();
    setVisibleCount(24);
  });
  function select(id: string) {
    if (changed() && !window.confirm(t('collection-unsaved'))) return;
    setSelected(id);
    setPoints([0, 0, 0, 0, 0, 0]);
  }
  function add(id: number) {
    const copies = draft().filter((value) => value === id).length;
    if (
      draft().length < content.rules.deck.size &&
      copies <
        Math.min(
          content.rules.deck.max_copies,
          data()?.collection.find((s) => s.card_id === id)?.qty ?? 0,
        )
    )
      setDraft((rows) => [...rows, id]);
  }
  return (
    <section class="collection-page">
      <Scene src={painting}>
        <h1 class="plaque">{t('collection-title')}</h1>
        <BackToMap />
      </Scene>
      <Show
        when={account()}
        fallback={
          <p>
            {t('collection-login')} <A href="/account">{t('auth-account')}</A>{' '}
            <button type="button" onClick={enterGuest}>
              {t('auth-guest')}
            </button>
          </p>
        }
      >
        <Show when={guest()}>
          <p role="status">{t('collection-guest')}</p>
        </Show>
        <Show when={error()}>
          <p class="collection-error" role="alert">
            {error()}
          </p>
        </Show>
        <Show when={notice()}>
          <p role="status">{notice()}</p>
        </Show>
        <Show when={data()}>
          {(snapshot) => (
            <>
              <fieldset disabled={loading()} class="collection-create">
                <legend>{t('collection-create')}</legend>
                <label>
                  {t('collection-template')}{' '}
                  <select
                    aria-label={t('collection-template')}
                    value={template() || snapshot().templates[0]}
                    onChange={(e) => setTemplate(Number(e.currentTarget.value))}
                  >
                    <For each={snapshot().templates}>
                      {(id) => (
                        <option value={id}>{t(fluentKey(avatars.get(id)?.key ?? ''))}</option>
                      )}
                    </For>
                  </select>
                </label>
                <button
                  type="button"
                  disabled={snapshot().avatars.length >= 10}
                  onClick={() =>
                    void mutate({
                      action: 'create',
                      id: crypto.randomUUID(),
                      template_id: template() || snapshot().templates[0] || 1,
                    })
                  }
                >
                  {t('collection-create-button')}
                </button>
                <span>{snapshot().avatars.length} / 10</span>
              </fieldset>
              <div class="collection-layout">
                <aside class="collection-avatars" aria-label={t('collection-avatars')}>
                  <For each={snapshot().avatars}>
                    {(avatar) => (
                      <button
                        type="button"
                        aria-pressed={selected() === avatar.id}
                        onClick={() => select(avatar.id)}
                      >
                        <Show when={avatars.get(avatar.path_id ?? avatar.template_id)}>
                          {(definition) => (
                            <img width="64" height="64" alt="" src={portrait(definition().key)} />
                          )}
                        </Show>
                        <span>
                          {t(
                            fluentKey(avatars.get(avatar.path_id ?? avatar.template_id)?.key ?? ''),
                          )}
                          <small>
                            {t('collection-level', { level: avatar.level })} · {avatar.xp} PD{' '}
                            {avatar.active ? '★' : ''}
                          </small>
                        </span>
                      </button>
                    )}
                  </For>
                </aside>
                <div class="collection-editor">
                  <Show when={own()}>
                    {(avatar) => (
                      <>
                        <div class="collection-heading">
                          <h2>
                            {t(
                              fluentKey(
                                avatars.get(avatar().path_id ?? avatar().template_id)?.key ?? '',
                              ),
                            )}
                          </h2>
                          <button
                            type="button"
                            disabled={loading() || avatar().active}
                            onClick={() => void mutate({ action: 'activate', id: avatar().id })}
                          >
                            {t(avatar().active ? 'collection-active' : 'collection-activate')}
                          </button>
                          <button
                            type="button"
                            disabled={loading() || avatar().active}
                            onClick={() =>
                              setConfirm({
                                action: 'delete',
                                id: avatar().id,
                                revision: avatar().revision,
                              })
                            }
                          >
                            {t('collection-delete')}
                          </button>
                        </div>
                        <fieldset disabled={loading()}>
                          <legend>{t('collection-points', { count: avatar().unspent })}</legend>
                          <div class="collection-points">
                            <For each={statIcons}>
                              {([symbol, stat], index) => (
                                <label>
                                  <img class="icon" src={icon(symbol)} alt="" />
                                  {t(stat)}: {avatar().stats[index()]}
                                  <input
                                    aria-label={t('collection-point-stat', {
                                      stat: t(stat),
                                    })}
                                    type="number"
                                    min="0"
                                    max={avatar().unspent}
                                    value={points()[index()]}
                                    onInput={(e) =>
                                      setPoints((rows) =>
                                        rows.map((v, i) =>
                                          i === index()
                                            ? Math.max(
                                                0,
                                                Math.floor(Number(e.currentTarget.value) || 0),
                                              )
                                            : v,
                                        ),
                                      )
                                    }
                                  />
                                </label>
                              )}
                            </For>
                          </div>
                          <button
                            type="button"
                            disabled={!spent() || spent() > avatar().unspent}
                            onClick={() =>
                              setConfirm({
                                action: 'spend',
                                id: avatar().id,
                                revision: avatar().revision,
                                points: [...points()],
                              })
                            }
                          >
                            {t('collection-spend')}
                          </button>
                        </fieldset>
                        <Show when={!guest()}>
                          <Progression
                            avatar={avatar()}
                            avatars={snapshot().avatars}
                            disabled={loading()}
                            confirm={setConfirm}
                          />
                        </Show>
                        <section aria-label={t('collection-deck')}>
                          <div class="collection-heading">
                            <h2>
                              {t('collection-deck')} ({draft().length}/{content.rules.deck.size})
                            </h2>
                            <A
                              href={`/duel?avatar=${avatar().path_id ?? avatar().template_id}&bot=novice&custom=1`}
                              onClick={() =>
                                setTrainingPlayer({
                                  avatar: avatar().path_id ?? avatar().template_id,
                                  specialization: avatar().specialization ?? undefined,
                                  gift_id: avatar().gift_id ?? undefined,
                                  level: avatar().level,
                                  spent: avatar().spent,
                                  deck: draft(),
                                })
                              }
                            >
                              {t('collection-test-deck')}
                            </A>
                            <button
                              type="button"
                              class="primary"
                              disabled={loading() || !changed()}
                              onClick={() =>
                                void mutate({
                                  action: 'deck',
                                  id: avatar().id,
                                  revision: avatar().deck_revision,
                                  cards: draft(),
                                })
                              }
                            >
                              {t('collection-save-deck')}
                            </button>
                            <button type="button" onClick={() => setDraft([])}>
                              {t('collection-clear')}
                            </button>
                          </div>
                          <p>{t('collection-fill')}</p>
                          <div class="collection-deck">
                            <For each={[...new Set(draft())]}>
                              {(id) => (
                                <button
                                  type="button"
                                  disabled={loading()}
                                  aria-label={t('collection-remove-card', {
                                    card: t(fluentKey(cards.get(id)?.key ?? '')),
                                  })}
                                  onClick={() =>
                                    setDraft((rows) => {
                                      const result = [...rows];
                                      result.splice(result.indexOf(id), 1);
                                      return result;
                                    })
                                  }
                                >
                                  {t(fluentKey(cards.get(id)?.key ?? ''))} ×
                                  {draft().filter((value) => value === id).length} −
                                </button>
                              )}
                            </For>
                          </div>
                          <div class="collection-presets">
                            <label>
                              {t('collection-preset-name')}{' '}
                              <input
                                maxLength={40}
                                value={presetName()}
                                onInput={(e) => setPresetName(e.currentTarget.value)}
                              />
                            </label>
                            <button
                              type="button"
                              disabled={
                                loading() ||
                                !presetName().trim() ||
                                snapshot().presets.length >= content.rules.deck.saved_presets_max
                              }
                              onClick={() =>
                                void mutate({
                                  action: 'preset_save',
                                  id: crypto.randomUUID(),
                                  revision: null,
                                  name: presetName(),
                                  cards: draft(),
                                })
                              }
                            >
                              {t('collection-save-preset')}
                            </button>
                            <For each={snapshot().presets}>
                              {(preset) => (
                                <div>
                                  <button type="button" onClick={() => setDraft([...preset.cards])}>
                                    {preset.name}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={t('collection-delete-preset', {
                                      name: preset.name,
                                    })}
                                    disabled={loading()}
                                    onClick={() =>
                                      void mutate({
                                        action: 'preset_delete',
                                        id: preset.id,
                                        revision: preset.revision,
                                      })
                                    }
                                  >
                                    ×
                                  </button>
                                </div>
                              )}
                            </For>
                          </div>
                        </section>
                      </>
                    )}
                  </Show>
                  <section aria-label={t('collection-library')}>
                    <h2>{t('collection-library')}</h2>
                    <div class="collection-filters">
                      <label>
                        {t('collection-search')}{' '}
                        <input
                          type="search"
                          value={query()}
                          onInput={(e) => setQuery(e.currentTarget.value)}
                        />
                      </label>
                      <select
                        aria-label={t('collection-rarity')}
                        value={rarity()}
                        onChange={(e) => setRarity(e.currentTarget.value)}
                      >
                        <option value="">{t('collection-all')}</option>
                        <For each={['bronze', 'silver', 'gold']}>
                          {(value) => (
                            <option value={value}>{t(`collection-rarity-${value}`)}</option>
                          )}
                        </For>
                      </select>
                      <label>
                        <input
                          type="checkbox"
                          checked={ownedOnly()}
                          onChange={(e) => setOwnedOnly(e.currentTarget.checked)}
                        />
                        {t('collection-owned')}
                      </label>
                      <label>
                        <input
                          type="checkbox"
                          checked={castable()}
                          onChange={(e) => setCastable(e.currentTarget.checked)}
                        />
                        {t('collection-castable')}
                      </label>
                    </div>
                    <div class="collection-cards">
                      <For each={visible().slice(0, visibleCount())}>
                        {(card) => {
                          const quantity = () =>
                            snapshot().collection.find((s) => s.card_id === card.id)?.qty ?? 0;
                          const copies = () => draft().filter((id) => id === card.id).length;
                          return (
                            <article>
                              <Show when={pictures().get(card.key)}>
                                {(src) => (
                                  <img
                                    class="collection-art"
                                    src={src()}
                                    alt=""
                                    width="512"
                                    height="320"
                                    loading="lazy"
                                  />
                                )}
                              </Show>
                              <h3>{t(fluentKey(card.key))}</h3>
                              <p>{t(fluentKey(card.key, 'text'), cardArgs(card))}</p>
                              <p>
                                K {card.cost.focus} · W {card.cost.stamina} ·{' '}
                                {t(`collection-rarity-${card.rarity}`)}
                              </p>
                              <p>
                                {t('collection-quantity', { count: quantity(), deck: copies() })}
                              </p>
                              <button
                                type="button"
                                disabled={
                                  loading() ||
                                  !own() ||
                                  draft().length >= content.rules.deck.size ||
                                  copies() >= Math.min(quantity(), content.rules.deck.max_copies)
                                }
                                aria-label={t('collection-add-card', {
                                  card: t(fluentKey(card.key)),
                                })}
                                onClick={() => add(card.id)}
                              >
                                {t('collection-add')}
                              </button>
                            </article>
                          );
                        }}
                      </For>
                    </div>
                    <p>
                      {t('collection-showing', {
                        shown: Math.min(visibleCount(), visible().length),
                        total: visible().length,
                      })}
                    </p>
                    <Show when={visibleCount() < visible().length}>
                      <button type="button" onClick={() => setVisibleCount((n) => n + 24)}>
                        {t('collection-more')}
                      </button>
                    </Show>
                  </section>
                </div>
              </div>
            </>
          )}
        </Show>
      </Show>
      <Show when={confirm()}>
        {(mutation) => (
          <Modal title={t('collection-confirm')} close={() => setConfirm(undefined)}>
            <p>
              {t(
                mutation().action === 'delete'
                  ? 'collection-delete-warning'
                  : mutation().action === 'reincarnate'
                    ? 'collection-reincarnate-warning'
                    : mutation().action === 'shade'
                      ? 'collection-shade-warning'
                      : 'collection-spend-warning',
              )}
            </p>
            <p>{progressionDescription(mutation(), data()?.avatars)}</p>
            <button
              type="button"
              onClick={() => {
                const value = confirm();
                setConfirm(undefined);
                if (value) void mutate(value);
              }}
            >
              {t('collection-confirm')}
            </button>
          </Modal>
        )}
      </Show>
    </section>
  );
}
