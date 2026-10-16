import { createEffect, createMemo, createSignal, For, onCleanup, Show } from 'solid-js';
import { accountEpoch, api } from '../../app/account';
import { cards, content, fluentKey } from '../../app/content';
import { Modal } from '../../components/Modal';
import type { TradeSnapshot } from '../../generated/proto/game/v1/trade_pb';
import { t } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';
import type { Snapshot } from '../collection/state';
import type { Wallet } from '../store/state';

const name = (id: number) => {
  const card = cards.get(id);
  return card ? t(fluentKey(card.key)) : `#${id}`;
};
export function Trade(props: {
  snapshot: TradeSnapshot;
  user: string;
  connected: boolean;
  now: number;
  send: (body: ClientBody) => Promise<ServerBody | undefined>;
  close: () => void;
}) {
  const [owned, setOwned] = createSignal<Snapshot>();
  const [wallet, setWallet] = createSignal<Wallet>();
  const [quantities, setQuantities] = createSignal<Record<number, number>>({});
  const [cash, setCash] = createSignal('0');
  const [pending, setPending] = createSignal(false);
  const [error, setError] = createSignal('');
  const [confirm, setConfirm] = createSignal<number>();
  let disposed = false;
  let loadedTrade = '';
  let draftKey = '';
  onCleanup(() => {
    disposed = true;
  });
  const side = () => props.snapshot.users.indexOf(props.user);
  const active = () => props.snapshot.status === 'offering';
  const selected = createMemo(() =>
    Object.entries(quantities())
      .filter(([, qty]) => qty > 0)
      .map(([id, qty]) => ({ $typeName: 'game.v1.TradeCard' as const, cardId: Number(id), qty }))
      .sort((a, b) => a.cardId - b.cardId),
  );
  const dirty = () => {
    const offer = props.snapshot.offers[side()];
    return (
      !offer ||
      !/^\d+$/.test(cash()) ||
      BigInt(cash()) !== offer.ducats ||
      JSON.stringify(selected().map((c) => [c.cardId, c.qty])) !==
        JSON.stringify(offer.cards.map((c) => [c.cardId, c.qty]))
    );
  };
  async function load() {
    const epoch = accountEpoch();
    try {
      const [collection, funds] = await Promise.all([
        api<Snapshot>('/api/game'),
        api<Wallet>('/api/economy'),
      ]);
      if (collection.content_version !== content.version) throw new Error('CONTENT_MISMATCH');
      if (!disposed && epoch === accountEpoch()) {
        setOwned(collection);
        setWallet(funds);
      }
    } catch {
      if (!disposed && epoch === accountEpoch()) setError(t('trade-load-error'));
    }
  }
  createEffect(() => {
    const snapshot = props.snapshot;
    const key = `${snapshot.tradeId}:${props.user}:${snapshot.version}:${snapshot.status}`;
    if (key === draftKey) return;
    draftKey = key;
    setConfirm(undefined);
    const own = snapshot.offers[side()];
    setQuantities(Object.fromEntries(own?.cards.map((c) => [c.cardId, c.qty]) ?? []));
    setCash(own?.ducats.toString() ?? '0');
    if (snapshot.status === 'offering' && loadedTrade !== snapshot.tradeId) {
      loadedTrade = snapshot.tradeId;
      void load();
    }
  });
  async function send(body: ClientBody) {
    if (pending()) return;
    setPending(true);
    setError('');
    setConfirm(undefined);
    try {
      if (!(await props.send(body)) && !disposed) {
        setError(t('trade-operation-error'));
        await props.send({
          case: 'tradeSnapshot',
          value: { $typeName: 'game.v1.TradeTarget', tradeId: props.snapshot.tradeId },
        });
      }
    } finally {
      if (!disposed) setPending(false);
    }
  }
  return (
    <section class="lobby-panel trade-panel" data-testid="trade">
      <h2>{t('trade-title')}</h2>
      <p>
        {props.snapshot.logins.join(' ↔ ')} · {t(`trade-status-${props.snapshot.status}`)}
      </p>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <Show when={props.snapshot.status === 'invited'}>
        <p>{t('trade-invite-help')}</p>
        <Show when={side() === 1}>
          <button
            type="button"
            disabled={!props.connected || pending()}
            onClick={() =>
              void send({
                case: 'tradeRespond',
                value: {
                  $typeName: 'game.v1.TradeRespond',
                  tradeId: props.snapshot.tradeId,
                  accept: true,
                },
              })
            }
          >
            {t('trade-join')}
          </button>{' '}
        </Show>
      </Show>
      <Show when={active() || props.snapshot.status === 'completed'}>
        <p>{t('trade-version', { value: props.snapshot.version })}</p>
        <div class="trade-offers">
          <For each={props.snapshot.offers}>
            {(offer, index) => (
              <article>
                <h3>{props.snapshot.logins[index()]}</h3>
                <p>
                  {offer.ducats.toString()} {t('store-ducats')}
                </p>
                <ul>
                  <For each={offer.cards}>
                    {(row) => (
                      <li>
                        {row.qty} × {name(row.cardId)}
                      </li>
                    )}
                  </For>
                </ul>
                <strong>
                  {t(props.snapshot.accepted[index()] ? 'trade-accepted' : 'trade-not-accepted')}
                </strong>
              </article>
            )}
          </For>
        </div>
      </Show>
      <Show when={active()}>
        <p>{t('trade-help')}</p>
        <button
          type="button"
          class="secondary"
          disabled={pending() || !props.connected}
          onClick={() => void load()}
        >
          {t('refresh')}
        </button>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (!/^\d+$/.test(cash())) return;
            void send({
              case: 'tradeUpdate',
              value: {
                $typeName: 'game.v1.TradeUpdate',
                tradeId: props.snapshot.tradeId,
                version: props.snapshot.version,
                offer: {
                  $typeName: 'game.v1.TradeOffer',
                  cards: selected(),
                  ducats: BigInt(cash()),
                },
              },
            });
          }}
        >
          <label>
            {t('trade-cash', { value: wallet()?.balance ?? '—' })}
            <input
              aria-label={t('trade-cash-label')}
              type="text"
              inputmode="numeric"
              pattern="[0-9]{1,19}"
              value={cash()}
              disabled={pending() || !props.connected}
              onInput={(event) => setCash(event.currentTarget.value)}
            />
          </label>
          <div class="trade-stacks">
            <For each={owned()?.collection.filter((c) => c.qty > 0)}>
              {(row) => (
                <label>
                  {name(row.card_id)} (×{row.qty})
                  <input
                    aria-label={`${t('trade-quantity')}: ${name(row.card_id)}`}
                    type="number"
                    min="0"
                    max={row.qty}
                    step="1"
                    value={quantities()[row.card_id] ?? 0}
                    disabled={pending() || !props.connected}
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
          <button
            type="submit"
            disabled={
              !owned() ||
              !props.connected ||
              pending() ||
              !dirty() ||
              !/^\d{1,19}$/.test(cash()) ||
              BigInt(cash()) > 9223372036854775807n
            }
          >
            {t('trade-send-offer')}
          </button>
        </form>
        <button
          type="button"
          disabled={!props.connected || pending() || dirty() || !!props.snapshot.accepted[side()]}
          onClick={() => setConfirm(props.snapshot.version)}
        >
          {t('trade-accept-offers')}
        </button>
      </Show>
      <Show
        when={active() || props.snapshot.status === 'invited'}
        fallback={
          <button type="button" onClick={props.close}>
            {t('close')}
          </button>
        }
      >
        <p>
          {t('trade-time', {
            value: Math.max(0, Math.ceil((Number(props.snapshot.deadlineMs) - props.now) / 1000)),
          })}
        </p>
        <button
          type="button"
          class="secondary"
          disabled={!props.connected || pending()}
          onClick={() =>
            void send({
              case: 'tradeCancel',
              value: { $typeName: 'game.v1.TradeTarget', tradeId: props.snapshot.tradeId },
            })
          }
        >
          {t('trade-cancel')}
        </button>
      </Show>
      <Show when={confirm() !== undefined}>
        <Modal title={t('trade-accept-offers')} close={() => setConfirm(undefined)}>
          <p>{t('trade-final-warning', { value: confirm() ?? 0 })}</p>
          <button
            type="button"
            disabled={confirm() !== props.snapshot.version || !props.connected || pending()}
            onClick={() =>
              void send({
                case: 'tradeConfirm',
                value: {
                  $typeName: 'game.v1.TradeConfirm',
                  tradeId: props.snapshot.tradeId,
                  version: confirm() ?? props.snapshot.version,
                },
              })
            }
          >
            {t('trade-confirm')}
          </button>
        </Modal>
      </Show>
    </section>
  );
}

import '../../app/cardCatalog';
