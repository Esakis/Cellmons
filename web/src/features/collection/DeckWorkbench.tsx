import { createMemo, createSignal, For, Show } from 'solid-js';
import { type Card, cardArgs, cards, content, fluentKey, icon } from '../../app/content';
import { Modal } from '../../components/Modal';
import { t } from '../../i18n';

const schools = ['combat', 'divine', 'element', 'arcane', 'necro', 'tech'];
export function DeckWorkbench(props: {
  draft: number[];
  change: (cards: number[]) => void;
  collection: { card_id: number; qty: number }[];
  schools: number[];
  pictures: Map<string, string>;
  disabled: boolean;
  mastery?: { card_id: number; xp: number }[];
}) {
  const [pane, setPane] = createSignal('library');
  const [query, setQuery] = createSignal('');
  const [school, setSchool] = createSignal('');
  const [rarity, setRarity] = createSignal('');
  const [ownedOnly, setOwnedOnly] = createSignal(true);
  const [castable, setCastable] = createSignal(true);
  const [preview, setPreview] = createSignal<Card>();
  const [hand, setHand] = createSignal<number[]>();
  const [limit, setLimit] = createSignal(24);
  const [undo, setUndo] = createSignal<number[]>();
  const quantity = (id: number) => props.collection.find((stack) => stack.card_id === id)?.qty ?? 0;
  const masteryXP = (id: number) => props.mastery?.find((c) => c.card_id === id)?.xp ?? 0;
  const masteryLevel = (id: number) =>
    [0, 15, 45, 100, 200].filter((n) => n <= masteryXP(id)).length;
  const copies = (id: number) => props.draft.filter((value) => value === id).length;
  const usable = (card: Card) =>
    Object.entries(card.requires).every(
      ([name, level]) => (props.schools[schools.indexOf(name)] ?? 0) >= level,
    );
  const canAdd = (id: number) =>
    !props.disabled &&
    props.draft.length < content.rules.deck.size &&
    copies(id) < Math.min(quantity(id), content.rules.deck.max_copies);
  const visible = createMemo(() =>
    content.cards.filter(
      (card) =>
        (!query() ||
          t(fluentKey(card.key))
            .toLocaleLowerCase()
            .includes(query().trim().toLocaleLowerCase())) &&
        (!school() || school() in card.requires) &&
        (!rarity() || rarity() === card.rarity) &&
        (!ownedOnly() || quantity(card.id) > 0) &&
        (!castable() || usable(card)),
    ),
  );
  function change(next: number[]) {
    setUndo([...props.draft]);
    props.change(next);
    setHand(undefined);
  }
  function add(id: number) {
    if (canAdd(id)) change([...props.draft, id]);
  }
  function remove(id: number) {
    if (props.disabled || !copies(id)) return;
    const next = [...props.draft];
    next.splice(next.indexOf(id), 1);
    change(next);
  }
  function draw() {
    const shuffled = [...props.draft];
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const left = shuffled[i];
      const right = shuffled[j];
      if (left !== undefined && right !== undefined) [shuffled[i], shuffled[j]] = [right, left];
    }
    setHand(shuffled.slice(0, content.rules.duel.hand_size));
  }
  function Counter(card: Card) {
    return (
      <div class="deck-counter">
        <button
          type="button"
          disabled={props.disabled || !copies(card.id)}
          aria-label={t('collection-remove-card', { card: t(fluentKey(card.key)) })}
          onClick={() => remove(card.id)}
        >
          −
        </button>
        <span title={t('collection-quantity', { count: quantity(card.id), deck: copies(card.id) })}>
          {copies(card.id)} / {Math.min(quantity(card.id), content.rules.deck.max_copies)}
        </span>
        <button
          type="button"
          disabled={!canAdd(card.id)}
          aria-label={t('collection-add-card', { card: t(fluentKey(card.key)) })}
          onClick={() => add(card.id)}
        >
          +
        </button>
      </div>
    );
  }
  // Focus and stamina costs as two coins on the picture's corner, like the cards on the duel board.
  function Cost(card: Card) {
    return (
      <span
        class="deck-cost"
        role="img"
        aria-label={`${t('focus')} ${card.cost.focus}, ${t('stamina')} ${card.cost.stamina}`}
      >
        <b class="deck-cost-focus">{card.cost.focus}K</b>
        <b class="deck-cost-stamina">{card.cost.stamina}W</b>
      </span>
    );
  }
  function Tile(card: Card) {
    return (
      <article class="deck-card" classList={{ 'deck-card-unusable': !usable(card) }}>
        <button
          type="button"
          class="deck-preview"
          onClick={() => setPreview(card)}
          aria-label={t('collection-preview', { card: t(fluentKey(card.key)) })}
        >
          <img src={props.pictures.get(card.key)} alt="" width="160" height="100" loading="lazy" />
          <strong>{t(fluentKey(card.key))}</strong>
        </button>
        {Cost(card)}
        {/* Every card starts at mastery 1/5; the tile shows only progress beyond it. */}
        <Show when={props.mastery && masteryLevel(card.id) > 1}>
          <small class="mastery-badge">
            {t('collection-mastery', { level: masteryLevel(card.id) })}
          </small>
        </Show>
        <Show when={!usable(card)}>
          <small class="deck-warning">{t('collection-requirements-missing')}</small>
        </Show>
        {Counter(card)}
      </article>
    );
  }
  return (
    <div class="deck-workbench" data-pane={pane()}>
      <div class="deck-toolbar">
        <nav class="deck-tabs" aria-label={t('collection-deck')}>
          <button
            type="button"
            aria-pressed={pane() === 'library'}
            onClick={() => setPane('library')}
          >
            {t('collection-available')}
          </button>
          <button type="button" aria-pressed={pane() === 'deck'} onClick={() => setPane('deck')}>
            {t('collection-deck')} {props.draft.length}/{content.rules.deck.size}
          </button>
        </nav>
        <button
          type="button"
          disabled={props.disabled || !undo()}
          onClick={() => {
            const previous = undo();
            if (previous) props.change(previous);
            setUndo(undefined);
            setHand(undefined);
          }}
        >
          {t('collection-undo')}
        </button>
        <button type="button" disabled={!props.draft.length} onClick={draw}>
          {t('collection-draw-hand')}
        </button>
      </div>
      <p class="deck-guidance" role="status">
        {t(
          props.draft.length === content.rules.deck.size
            ? 'collection-deck-full'
            : 'collection-deck-help',
        )}
      </p>
      <div class="deck-columns">
        <section class="deck-current" aria-label={t('collection-current-deck')}>
          <h3>
            {t('collection-current-deck')} · {props.draft.length}/{content.rules.deck.size}
          </h3>
          <div class="collection-deck">
            <For each={[...new Set(props.draft)]} fallback={<p>{t('collection-deck-empty')}</p>}>
              {(id) => <Show when={cards.get(id)}>{(card) => Tile(card())}</Show>}
            </For>
          </div>
        </section>
        <section class="deck-library" aria-label={t('collection-library')}>
          <h3>{t('collection-available')}</h3>
          <div class="collection-filters">
            <input
              type="search"
              aria-label={t('collection-search')}
              placeholder={t('collection-search')}
              value={query()}
              onInput={(e) => {
                setQuery(e.currentTarget.value);
                setLimit(24);
              }}
            />
            <select
              aria-label={t('collection-school')}
              value={school()}
              onChange={(e) => {
                setSchool(e.currentTarget.value);
                setLimit(24);
              }}
            >
              <option value="">{t('collection-all-schools')}</option>
              <For each={schools}>
                {(name) => <option value={name}>{t(`school-${name}-name`)}</option>}
              </For>
            </select>
            {/* Search and school stay at hand; the rarer filters wait behind one line. */}
            <details class="collection-more-filters">
              <summary>{t('collection-more-filters')}</summary>
              <select
                aria-label={t('collection-rarity')}
                value={rarity()}
                onChange={(e) => {
                  setRarity(e.currentTarget.value);
                  setLimit(24);
                }}
              >
                <option value="">{t('collection-all')}</option>
                <For each={['bronze', 'silver', 'gold']}>
                  {(name) => <option value={name}>{t(`collection-rarity-${name}`)}</option>}
                </For>
              </select>
              <label>
                <input
                  type="checkbox"
                  checked={ownedOnly()}
                  onChange={(e) => {
                    setOwnedOnly(e.currentTarget.checked);
                    setLimit(24);
                  }}
                />
                {t('collection-owned')}
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={castable()}
                  onChange={(e) => {
                    setCastable(e.currentTarget.checked);
                    setLimit(24);
                  }}
                />
                {t('collection-castable')}
              </label>
            </details>
          </div>
          <div class="collection-cards">
            <For each={visible().slice(0, limit())} fallback={<p>{t('collection-no-cards')}</p>}>
              {Tile}
            </For>
          </div>
          <p>
            {t('collection-showing', {
              shown: Math.min(limit(), visible().length),
              total: visible().length,
            })}
          </p>
          <Show when={visible().length > limit()}>
            <button type="button" onClick={() => setLimit(limit() + 24)}>
              {t('collection-more')}
            </button>
          </Show>
        </section>
      </div>
      <Show when={preview()}>
        {(card) => (
          <Modal title={t(fluentKey(card().key))} close={() => setPreview(undefined)}>
            <img
              class="deck-detail-art"
              src={props.pictures.get(card().key)}
              alt=""
              width="480"
              height="300"
            />
            <p>{t(fluentKey(card().key, 'text'), cardArgs(card()))}</p>
            <Show when={props.mastery}>
              <p>
                {t('collection-mastery-detail', {
                  level: masteryLevel(card().id),
                  xp: masteryXP(card().id),
                  target: [0, 15, 45, 100, 200][masteryLevel(card().id)] ?? 200,
                })}
              </p>
            </Show>
            <p>
              {t('focus')} {card().cost.focus} · {t('stamina')} {card().cost.stamina} ·{' '}
              {t(`collection-rarity-${card().rarity}`)}
            </p>
            <ul class="deck-requirements">
              <For each={Object.entries(card().requires)}>
                {([name, level]) => (
                  <li>
                    <img class="icon" src={icon(`school-${name}`)} alt="" />
                    {t(`school-${name}-name`)} {level}
                  </li>
                )}
              </For>
            </ul>
            <Show when={!usable(card())}>
              <p class="deck-warning">{t('collection-requirements-missing')}</p>
            </Show>
            {Counter(card())}
          </Modal>
        )}
      </Show>
      <Show when={hand()}>
        {(drawn) => (
          <Modal title={t('collection-draw-hand')} close={() => setHand(undefined)} wide>
            <p>{t('collection-hand-help')}</p>
            <div class="deck-sample">
              <For each={drawn()}>
                {(id) => (
                  <Show when={cards.get(id)}>
                    {(card) => (
                      <article>
                        <img src={props.pictures.get(card().key)} alt="" width="160" height="100" />
                        <strong>{t(fluentKey(card().key))}</strong>
                        {Cost(card())}
                      </article>
                    )}
                  </Show>
                )}
              </For>
            </div>
            <button type="button" onClick={draw}>
              {t('collection-draw-again')}
            </button>
          </Modal>
        )}
      </Show>
    </div>
  );
}
