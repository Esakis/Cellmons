import { A } from '@solidjs/router';
import { createEffect, createMemo, createSignal, For, onCleanup, Show } from 'solid-js';
import { ApiError, account, accountEpoch, api, restoreAccount } from '../../app/account';
import { avatars, cards, content, fluentKey } from '../../app/content';
import { onlineIcon } from '../../app/onlineIcons';
import { BackToMap } from '../../components/BackToMap';
import { Modal } from '../../components/Modal';
import { Scene } from '../../components/Scene';
import { addTranslations, hasTranslation, language, t } from '../../i18n';
import type { Snapshot } from '../collection/state';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import {
  finishRequest,
  type Inventory,
  type Operation,
  pendingRequest,
  type Receipt,
  type Request,
  startRequest,
  type Wallet,
} from './state';
import '../online.css';

addTranslations({ pl, en });
// The shop's painting (art/paintings/scenes), the same as in the lobby's shop room.
const [painting] = Object.values(
  import.meta.glob<string>('../../assets/paintings/scenes/shop.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);
const cardName = (id: number) => {
  const card = cards.get(id);
  return card ? t(fluentKey(card.key)) : `#${id}`;
};
const junkyard = content.rules.junkyard;
const rarityPoints = {
  bronze: junkyard.rarity_points[0] ?? 0,
  silver: junkyard.rarity_points[1] ?? 0,
  gold: junkyard.rarity_points[2] ?? 0,
};
export function Store() {
  const [inventory, setInventory] = createSignal<Inventory>();
  const [wallet, setWallet] = createSignal<Wallet>();
  const [collection, setCollection] = createSignal<Snapshot>();
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [receipt, setReceipt] = createSignal<Receipt>();
  const [pending, setPending] = createSignal(pendingRequest());
  const [confirm, setConfirm] = createSignal<Operation>();
  const [quantities, setQuantities] = createSignal<Record<number, number>>({});
  const [odds, setOdds] = createSignal<Inventory['offers'][number]>();
  let disposed = false;
  let identity = '';
  const ready = () => account()?.kind === 'account';
  const selected = createMemo(() =>
    Object.entries(quantities())
      .filter(([, qty]) => qty > 0)
      .map(([id, qty]) => ({ card_id: Number(id), qty })),
  );
  const points = createMemo(() =>
    selected().reduce((sum, row) => {
      const rarity = cards.get(row.card_id)?.rarity;
      const value =
        rarity === 'bronze' || rarity === 'silver' || rarity === 'gold' ? rarityPoints[rarity] : 0;
      return sum + row.qty * value;
    }, 0),
  );
  function fail(reason: unknown) {
    const code = reason instanceof ApiError ? reason.code : 'SERVER_UNAVAILABLE';
    setError(
      t(hasTranslation(`store-error-${code}`) ? `store-error-${code}` : 'store-error-default'),
    );
  }
  async function refresh() {
    const epoch = accountEpoch();
    const [inv, funds, owned] = await Promise.all([
      api<Inventory>('/api/store'),
      api<Wallet>('/api/economy'),
      api<Snapshot>('/api/game'),
    ]);
    if (inv.content_version !== content.version || owned.content_version !== content.version)
      throw new ApiError('CONTENT_MISMATCH');
    if (!disposed && epoch === accountEpoch()) {
      setInventory(inv);
      setWallet(funds);
      setCollection(owned);
    }
  }
  async function load() {
    const epoch = accountEpoch();
    setLoading(true);
    setError('');
    try {
      await refresh();
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) fail(reason);
    } finally {
      if (!disposed && epoch === accountEpoch()) setLoading(false);
    }
  }
  createEffect(() => {
    const current = account();
    const key = `${current?.kind}:${accountEpoch()}`;
    if (key === identity) return;
    identity = key;
    setInventory(undefined);
    setWallet(undefined);
    setCollection(undefined);
    setReceipt(undefined);
    setQuantities({});
    setConfirm(undefined);
    setPending(pendingRequest());
    setError('');
    setLoading(false);
    if (current?.kind === 'account') void load();
    else if (!current && navigator.onLine) void restoreAccount();
  });
  onCleanup(() => {
    disposed = true;
  });
  async function execute(request: Request) {
    if (loading() || !ready()) return;
    const epoch = accountEpoch();
    setLoading(true);
    setError('');
    setConfirm(undefined);
    setReceipt(undefined);
    try {
      const result = await api<Receipt>('/api/store', request);
      if (disposed || epoch !== accountEpoch()) return;
      finishRequest(request.key);
      setPending(undefined);
      setReceipt(result);
      setQuantities({});
      await refresh();
    } catch (reason) {
      if (disposed || epoch !== accountEpoch()) return;
      if (
        reason instanceof ApiError &&
        !['SERVER_UNAVAILABLE', 'INTERNAL', 'UNAVAILABLE', 'TIMEOUT'].includes(reason.code)
      ) {
        finishRequest(request.key);
        setPending(undefined);
        if (reason.code === 'PRICE_CHANGED') await refresh().catch(() => {});
      }
      fail(reason);
    } finally {
      if (!disposed && epoch === accountEpoch()) setLoading(false);
    }
  }
  function begin(operation: Operation) {
    if (loading() || pending()) return;
    const request = startRequest(operation);
    setPending(request);
    void execute(request);
  }
  return (
    <section class="store-page">
      <Scene src={painting}>
        <h1 class="plaque">{t('store')}</h1>
        <BackToMap />
      </Scene>
      <Show
        when={ready()}
        fallback={
          <p>
            {t('store-account')} <A href="/account">{t('auth-account')}</A>
          </p>
        }
      >
        <div class="store-toolbar">
          <strong data-testid="wallet-balance">
            <img class="icon" src={onlineIcon('ducats')} alt="" />
            {t('store-balance', { value: wallet()?.balance ?? '—' })}
          </strong>
          <button type="button" class="secondary" disabled={loading()} onClick={() => void load()}>
            {t('refresh')}
          </button>
        </div>
        <Show when={error()}>
          <p role="alert">{error()}</p>
        </Show>
        <Show when={pending()}>
          {(request) => (
            <aside role="status">
              <p>{t('store-pending')}</p>
              <button type="button" disabled={loading()} onClick={() => void execute(request())}>
                {t('store-retry')}
              </button>
            </aside>
          )}
        </Show>
        <Show when={receipt()}>
          {(result) => (
            <aside class="store-receipt" role="status">
              <p>
                {t('store-complete')} ({result().key})
              </p>
              <Show when={result().ducats !== '0'}>
                <p>{t('store-change', { value: result().ducats })}</p>
              </Show>
              <Show when={result().points !== '0'}>
                <p>{t('store-points-added', { value: result().points })}</p>
              </Show>
              <ul>
                <For each={result().cards}>{(id) => <li>{cardName(id)}</li>}</For>
                <For each={result().unlocked}>
                  {(id) => <li>{t(fluentKey(avatars.get(id)?.key ?? `avatar.${id}`))}</li>}
                </For>
              </ul>
            </aside>
          )}
        </Show>
        <h2>{t('store-packs')}</h2>
        <p>{t('store-final')}</p>
        <div class="store-grid">
          <For each={inventory()?.offers}>
            {(offer) => (
              <article>
                <h3>{t(`store-pack-${offer.id}`)}</h3>
                <p>
                  <img class="icon" src={onlineIcon('ducats')} alt="" />
                  {t('store-price', { value: offer.price })}
                </p>
                <p>
                  <For each={offer.slots}>
                    {(slot) => (
                      <span>
                        {slot.count} × {t(`store-rarity-${slot.rarity}`)}{' '}
                      </span>
                    )}
                  </For>
                </p>
                <button type="button" class="secondary" onClick={() => setOdds(offer)}>
                  {t('store-odds')}
                </button>{' '}
                <button
                  type="button"
                  disabled={
                    loading() ||
                    !!pending() ||
                    !offer.available ||
                    BigInt(wallet()?.balance ?? '0') < BigInt(offer.price)
                  }
                  onClick={() => setConfirm({ action: 'buy', kind: offer.id, quote: offer.quote })}
                >
                  {t('store-buy', { pack: t(`store-pack-${offer.id}`) })}
                </button>
              </article>
            )}
          </For>
        </div>
        <h2>{t('store-owned')}</h2>
        <div class="store-grid">
          <For each={inventory()?.boosters.filter((b) => b.status === 'unopened')}>
            {(booster) => (
              <article>
                <h3>{t(`store-pack-${booster.kind}`)}</h3>
                <small>{booster.id}</small>
                <p>{t('store-price', { value: booster.price })}</p>
                <button
                  type="button"
                  class="secondary"
                  onClick={() => setOdds({ ...booster, quote: '', available: true })}
                >
                  {t('store-odds')}
                </button>{' '}
                <button
                  type="button"
                  disabled={loading() || !!pending()}
                  onClick={() => begin({ action: 'open', id: booster.id })}
                >
                  {t('store-open')}
                </button>
              </article>
            )}
          </For>
        </div>
        <h2>{t('store-opened')}</h2>
        <For each={inventory()?.boosters.filter((b) => b.status === 'opened')}>
          {(booster) => (
            <details>
              <summary>
                {t(`store-pack-${booster.kind}`)} · {booster.id}
              </summary>
              <ul>
                <For each={booster.contents ?? []}>{(id) => <li>{cardName(id)}</li>}</For>
              </ul>
            </details>
          )}
        </For>
        <h2>{t('store-junkyard')}</h2>
        <p>
          {t('store-junkyard-rules', {
            bronze: rarityPoints.bronze,
            silver: rarityPoints.silver,
            gold: rarityPoints.gold,
            threshold: junkyard.reward_threshold,
            fallback: junkyard.fallback_ducats,
            characters: content.avatars.filter((a) => a.rarity === 'junkyard').length,
          })}
        </p>
        <p>{t('store-total-points', { value: inventory()?.junkyard_points ?? '0' })}</p>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            setConfirm({ action: 'discard', cards: selected() });
          }}
        >
          <div class="store-stacks">
            <For each={collection()?.collection.filter((row) => row.qty > 0)}>
              {(row) => (
                <label>
                  <span>
                    {cardName(row.card_id)} (×{row.qty})
                  </span>
                  <input
                    type="number"
                    inputmode="numeric"
                    min="0"
                    max={row.qty}
                    step="1"
                    value={quantities()[row.card_id] ?? 0}
                    disabled={loading() || !!pending()}
                    onInput={(event) => {
                      const qty = Number(event.currentTarget.value);
                      setQuantities((old) => ({
                        ...old,
                        [row.card_id]:
                          Number.isInteger(qty) && qty >= 0 && qty <= row.qty ? qty : 0,
                      }));
                    }}
                  />
                </label>
              )}
            </For>
          </div>
          <p>{t('store-selected-points', { value: points() })}</p>
          <button
            type="submit"
            disabled={
              !inventory()?.junkyard_available || !selected().length || loading() || !!pending()
            }
          >
            {t('store-discard')}
          </button>
        </form>
        <h2>{t('store-ledger')}</h2>
        <ol class="store-ledger">
          <For each={wallet()?.entries}>
            {(entry) => (
              <li>
                <time>{new Date(entry.at_ms).toLocaleString(language())}</time>{' '}
                <span>{t(`store-reason-${entry.reason}`)}</span> <strong>{entry.delta}</strong>
              </li>
            )}
          </For>
        </ol>
      </Show>
      <Show when={confirm()}>
        {(operation) => (
          <Modal
            title={t(operation().action === 'discard' ? 'store-discard' : 'store-confirm-buy')}
            close={() => setConfirm(undefined)}
          >
            <p>{t(operation().action === 'discard' ? 'store-discard-warning' : 'store-final')}</p>
            <Show
              when={operation().action === 'discard'}
              fallback={
                <p>
                  {t('store-price', {
                    value:
                      inventory()?.offers.find(
                        (o) =>
                          operation().action === 'buy' &&
                          o.id === (operation() as { kind: string }).kind,
                      )?.price ?? '',
                  })}
                </p>
              }
            >
              <ul>
                <For each={selected()}>
                  {(row) => (
                    <li>
                      {row.qty} × {cardName(row.card_id)}
                    </li>
                  )}
                </For>
              </ul>
              <p>{t('store-selected-points', { value: points() })}</p>
            </Show>
            <button type="button" onClick={() => begin(operation())}>
              {t('store-confirm')}
            </button>
          </Modal>
        )}
      </Show>
      <Show when={odds()}>
        {(offer) => (
          <Modal title={t('store-odds')} close={() => setOdds(undefined)} wide>
            <p>{t('store-odds-help')}</p>
            <For each={offer().slots}>
              {(slot) => (
                <section>
                  <h3>
                    {slot.count} × {t(`store-rarity-${slot.rarity}`)}
                  </h3>
                  <p>
                    {t('store-per-draw', {
                      value: (100 / slot.pool.length).toLocaleString(language(), {
                        maximumFractionDigits: 4,
                      }),
                      count: slot.pool.length,
                    })}
                  </p>
                  <ul>
                    <For each={slot.pool}>{(id) => <li>{cardName(id)}</li>}</For>
                  </ul>
                </section>
              )}
            </For>
          </Modal>
        )}
      </Show>
    </section>
  );
}

import '../../app/cardCatalog';
