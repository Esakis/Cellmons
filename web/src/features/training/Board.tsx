import {
  createSignal,
  For,
  Index,
  type JSX,
  onCleanup,
  onMount,
  type ParentProps,
  Show,
} from 'solid-js';
import {
  avatars,
  type Card,
  cardArgs,
  cards,
  content,
  fluentKey,
  icon,
  portrait,
} from '../../app/content';
import { speed } from '../../app/session';
import { Modal } from '../../components/Modal';
import type {
  DuelEvent,
  DuelMove,
  DuelSide,
  MovePreview,
  TrainingView,
} from '../../generated/proto/game/v1/training_pb';
import { hasTranslation, t } from '../../i18n';
import { create, DuelMoveSchema } from '../../worker/wire';
import { advanceAnimation } from './animation';
import styles from './Training.module.css';

// The duel board shared by offline training and online duels: character panels, the decision
// table, the round simulation and the chronicle. Each screen supplies its own data and actions.
const cardKeys = ['q', 'w', 'e', 'r', 't'];
const attackEvents = ['hit', 'block', 'miss'];
type Fx = { tone: 'damage' | 'heal' | 'block' | 'miss'; text: string; ms: number; delay: number };
export type RoundRecord = { round: number; events: DuelEvent[] };
type Group = 'attacks' | 'defenses';
// Bookkeeping events that would only clutter the chronicle.
export const quietEvents = ['reveal', 'round', 'draw', 'end'];
const name = (id: number) => t(fluentKey(avatars.get(id)?.key ?? ''));
export const cardName = (id: number) => t(fluentKey(cards.get(id)?.key ?? ''));
const [pictures, setPictures] = createSignal(new Map<string, string>());
const cardPicture = (key: string) => pictures().get(key);
// Card illustrations arrive on demand; until then (or without them) cards show school icons.
export function loadPictures() {
  if (pictures().size) return;
  void import('../../app/cardArt')
    .then((module) => setPictures(module.cardPictures))
    .catch(() => {});
}
export const errorText = (code: string) =>
  t(hasTranslation(`error-${code}`) ? `error-${code}` : 'error-generic');
const kindName = (kind: number) => t(kind === 0 ? 'physical' : 'magical');
// Screens around the board use its window, result plate and colours.
export const boardStyles = styles;

// Damage dealt by `side`: its hits plus spell damage taken by the other side.
function dealtBy(list: DuelEvent[], side: number) {
  return list.reduce(
    (sum, e) =>
      (e.kind === 'hit' && e.side === side) || (e.kind === 'damage' && e.side !== side)
        ? sum + e.amount
        : sum,
    0,
  );
}

const hpPercent = (side: DuelSide) =>
  `${Math.min(100, (Math.max(0, side.hp) / Math.max(1, side.hpStart)) * 100)}%`;
const lowHp = (side: DuelSide) => side.hp * 10 < side.hpStart * 3;

// One event as a sentence; `names` are both sides as the viewer sees them.
export function eventText(event: DuelEvent, names: string[]) {
  const detailKey = `stat-${event.detail}`;
  const schoolKey = `school-${event.detail.toLowerCase()}-name`;
  return t(`event-${event.kind}`, {
    name: names[event.side] ?? '',
    card: event.cardId ? cardName(event.cardId) : '',
    amount: event.amount,
    slot: event.slot + 1,
    kind: kindName(event.attackKind),
    old: event.oldValue,
    new: event.newValue,
    detail: hasTranslation(detailKey)
      ? t(detailKey)
      : hasTranslation(schoolKey)
        ? t(schoolKey)
        : event.detail,
  });
}

function toggled(move: DuelMove, group: Group, slot: number) {
  return create(DuelMoveSchema, {
    ...move,
    [group]: move[group].map((kind, index) => (index === slot ? 1 - kind : kind)),
  });
}
// Tapping a card steps through cast, discard and no action; an uncastable card starts at discard.
function cycled(move: DuelMove, index: number, castOptions: MovePreview[]) {
  const selected = move.action !== 0 && move.handIndex === index;
  const action = selected ? (move.action + 1) % 3 : castOptions[index]?.error ? 2 : 1;
  return create(DuelMoveSchema, { ...move, action, handIndex: index });
}

// Title plate on the window frame with a name plate for each side.
function TitleBar(props: ParentProps<{ left: string; right: string }>) {
  return (
    <div class={styles.titleBar}>
      <span class={styles.namePlate}>{props.left}</span>
      {props.children}
      <span class={styles.namePlate}>{props.right}</span>
    </div>
  );
}

// Strip with the character name; the badge in the corner marks initiative.
function NameStrip(props: { avatar: number; owner: number; initiative: number; label: string }) {
  return (
    <>
      <h2 class={styles.nameStrip}>{name(props.avatar)}</h2>
      <Show when={props.initiative === props.owner}>
        <span
          class={styles.initiativeBadge}
          role="img"
          aria-label={t('initiative', { name: props.label })}
          title={t('initiative', { name: props.label })}
        >
          ◆
        </span>
      </Show>
    </>
  );
}

// Vertical health gauge between a character panel and the centre.
function HealthGauge(props: { side: DuelSide }) {
  return (
    <div
      class={styles.gauge}
      title={`${t('hp')}: ${Math.max(0, props.side.hp)} / ${props.side.hpStart}`}
      aria-hidden="true"
    >
      <span style={{ height: hpPercent(props.side) }} data-low={lowHp(props.side)} />
    </div>
  );
}

// Card face shared by the hand and the played card: name with focus cost, illustration,
// school requirements with stamina cost and the description.
function CardFace(props: { card: Card; focus: number; stamina: number; hotkey?: string }) {
  return (
    <>
      <span class={styles.cardTop}>
        <Show when={props.hotkey}>
          <kbd>{props.hotkey}</kbd>
        </Show>
        <strong>{cardName(props.card.id)}</strong>
        <b class={styles.focusBadge}>
          {props.focus}
          <small>K</small>
        </b>
      </span>
      <span class={styles.cardArt} data-picture={!!cardPicture(props.card.key)} aria-hidden="true">
        <img
          src={
            cardPicture(props.card.key) ??
            icon(`school-${Object.keys(props.card.requires)[0] ?? 'arcane'}`)
          }
          alt=""
        />
      </span>
      <span class={styles.cardStrip}>
        <Requirements card={props.card} />
        <b class={styles.staminaBadge}>
          {props.stamina}
          <small>W</small>
        </b>
      </span>
      <span class={styles.cardText}>{cardDescription(props.card)}</span>
    </>
  );
}

// One side of the board: framed portrait, health, schools, choices and the card it played.
function Combatant(props: {
  side: DuelSide;
  avatar: number;
  owner: number;
  initiative: number;
  label: string;
  played: number;
  fx?: Fx;
  portraitRef: (element: HTMLDivElement) => void;
}) {
  return (
    <section class={styles.combatant} aria-label={props.label} data-owner={props.owner}>
      <NameStrip
        avatar={props.avatar}
        owner={props.owner}
        initiative={props.initiative}
        label={props.label}
      />
      <div class={styles.portraitFrame} ref={props.portraitRef}>
        <img
          class={styles.portrait}
          src={portrait(avatars.get(props.avatar)?.key ?? '')}
          alt=""
          width="132"
          height="132"
        />
        <Show when={props.fx} keyed>
          {(fx) => (
            <span
              class={styles.popup}
              data-tone={fx.tone}
              aria-hidden="true"
              style={{ 'animation-duration': `${fx.ms}ms`, 'animation-delay': `${fx.delay}ms` }}
            >
              {fx.text}
            </span>
          )}
        </Show>
      </div>
      <div class={styles.healthLabel}>
        <span>{t('hp')}</span>
        <strong>
          {props.side.hp} / {props.side.hpStart}
        </strong>
      </div>
      <meter
        class={styles.srOnly}
        min="0"
        max={props.side.hpStart}
        value={Math.max(0, props.side.hp)}
        aria-label={t('hp')}
      />
      <div class={styles.health} aria-hidden="true">
        <span class={styles.healthGhost} style={{ width: hpPercent(props.side) }} />
        <span
          class={styles.healthFill}
          style={{ width: hpPercent(props.side) }}
          data-low={lowHp(props.side)}
        />
      </div>
      <PlayedCard id={props.played} />
    </section>
  );
}

// Decision screen: a character card with its schools, four stats and enchantments.
function SideCard(props: {
  side: DuelSide;
  avatar: number;
  owner: number;
  initiative: number;
  enchanted: TrainingView['enchantments'];
  label: string;
}) {
  const pills = () => [
    {
      icon: 'stat-phys-atk',
      label: `${t('stat-PhysAtk')} / ${t('stat-PhysDef')}`,
      value: `${props.side.stats[0]}/${props.side.stats[1]}`,
    },
    {
      icon: 'stat-mag-atk',
      label: `${t('stat-MagAtk')} / ${t('stat-MagDef')}`,
      value: `${props.side.stats[2]}/${props.side.stats[3]}`,
    },
    { icon: 'stat-hp', label: t('hp'), value: `${Math.max(0, props.side.hp)}` },
    { icon: 'stat-stamina', label: t('stat-Stamina'), value: `${props.side.stamina}` },
  ];
  return (
    <section class={styles.sideCard} aria-label={props.label} data-owner={props.owner}>
      <NameStrip
        avatar={props.avatar}
        owner={props.owner}
        initiative={props.initiative}
        label={props.label}
      />
      <div class={styles.portraitFrame}>
        <img
          class={styles.sidePortrait}
          src={portrait(avatars.get(props.avatar)?.key ?? '')}
          alt=""
          width="190"
          height="190"
        />
      </div>
      <div class={styles.schools}>
        <For each={content.schools}>
          {(school, i) => (
            <Show when={(props.side.schools[i()] ?? 0) > 0}>
              <span>
                <img class={styles.panelIcon} src={icon(`school-${school.key}`)} alt="" />
                {t(`school-${school.key}-name`)} {props.side.schools[i()]}
              </span>
            </Show>
          )}
        </For>
      </div>
      <dl class={styles.pills}>
        <Index each={pills()}>
          {(pill) => (
            <div class={styles.pill} title={pill().label}>
              <dt>
                <img
                  class={`${styles.panelIcon} ${styles.medalIcon}`}
                  src={icon(pill().icon)}
                  alt={pill().label}
                />
              </dt>
              <dd>{pill().value}</dd>
            </div>
          )}
        </Index>
      </dl>
      <meter
        class={styles.srOnly}
        min="0"
        max={props.side.hpStart}
        value={Math.max(0, props.side.hp)}
        aria-label={t('hp')}
      />
      <section class={styles.enchantments} aria-label={t('enchantments')}>
        <For
          each={props.enchanted.filter((e) => e.side === props.owner)}
          fallback={<small>{t('no-enchantments')}</small>}
        >
          {(e) => <span title={cardDescription(cards.get(e.cardId))}>✦ {cardName(e.cardId)}</span>}
        </For>
      </section>
    </section>
  );
}

function OpponentRow(props: { label: string; kinds?: number[] }) {
  return (
    <div class={styles.opponentRow} title={props.label}>
      <span class={styles.srOnly}>{props.label}</span>
      <For each={props.kinds ?? [-1, -1, -1]}>
        {(kind) => (
          <span class={styles.ghostSlot} data-kind={kind}>
            <Show when={kind >= 0} fallback="·">
              <img
                class={`${styles.panelIcon} ${styles.ghostIcon}`}
                src={icon(kind === 0 ? 'attack-phys' : 'attack-mag')}
                alt={t(kind === 0 ? 'physical' : 'magical')}
              />
            </Show>
          </span>
        )}
      </For>
    </div>
  );
}

function PlayedCard(props: { id: number }) {
  return (
    <div class={styles.played}>
      <span class={styles.playedLabel}>{t('played-card')}</span>
      <Show
        when={cards.get(props.id)}
        fallback={<p class={styles.playedEmpty}>{t('no-card-played')}</p>}
      >
        {(card) => (
          <article class={styles.playedCard} data-rarity={card().rarity}>
            <CardFace card={card()} focus={card().cost.focus} stamina={card().cost.stamina} />
          </article>
        )}
      </Show>
    </div>
  );
}

// The centre of the board: one narrow column of numbers per side.
function statValues(side: DuelSide) {
  return [
    `${side.stats[0]}/${side.stats[1]}`,
    `${side.stats[2]}/${side.stats[3]}`,
    `${Math.max(0, side.hp)}`,
    `${side.stamina}`,
    `${side.focus}`,
  ];
}
// How a value such as "4/3" changed since the round start: every number grew, fell or both.
function trend(value: string, before: string) {
  if (value === before) return undefined;
  const now = value.split('/').map(Number);
  const then = before.split('/').map(Number);
  if (now.every((n, i) => n >= (then[i] ?? n))) return 'up';
  if (now.every((n, i) => n <= (then[i] ?? n))) return 'down';
  return 'mixed';
}

function StatColumn(props: {
  side: DuelSide;
  base?: DuelSide;
  owner: number;
  label: string;
  dealt: number;
}) {
  const labels = () => [
    `${t('stat-PhysAtk')} / ${t('stat-PhysDef')}`,
    `${t('stat-MagAtk')} / ${t('stat-MagDef')}`,
    t('hp'),
    t('stat-Stamina'),
    t('stat-Focus'),
  ];
  const statIcons = ['stat-phys-atk', 'stat-mag-atk', 'stat-hp', 'stat-stamina', 'stat-focus'];
  const rows = () => {
    const values = statValues(props.side);
    const before = props.base ? statValues(props.base) : values;
    return values.map((value, i) => ({
      icon: statIcons[i] ?? '',
      label: labels()[i] ?? '',
      value,
      trend: trend(value, before[i] ?? value),
    }));
  };
  return (
    <dl class={styles.statColumn} data-owner={props.owner} aria-label={props.label}>
      <Index each={rows()}>
        {(row) => (
          <div class={styles.statRow} title={row().label} data-trend={row().trend}>
            <dt class={styles.medal}>
              <img
                class={`${styles.panelIcon} ${styles.medalIcon}`}
                src={icon(row().icon)}
                alt={row().label}
              />
            </dt>
            <dd>{row().value}</dd>
          </div>
        )}
      </Index>
      <div class={styles.statDealt} title={t('chronicle-total')}>
        <dt>{t('dealt-short')}</dt>
        <dd>{props.dealt}</dd>
      </div>
    </dl>
  );
}

// Every round so far, newest first: each attack slot with its result, then spells and effects.
// `playing` is the index of the event being animated in the newest round; later events stay hidden.
function Chronicle(props: {
  history: RoundRecord[];
  playing?: number;
  text: (event: DuelEvent) => string;
}) {
  return (
    <section class={styles.chronicle} aria-label={t('chronicle')}>
      <For
        each={props.history}
        fallback={<p class={styles.chronicleEmpty}>{t('chronicle-empty')}</p>}
      >
        {(record, i) => {
          const visible = (index: number) =>
            i() > 0 || props.playing === undefined || index <= props.playing;
          const attack = (side: number, slot: number) => {
            const index = record.events.findIndex(
              (e) => attackEvents.includes(e.kind) && e.side === side && e.slot === slot,
            );
            const event = record.events[index];
            return event && visible(index) ? { event, index } : undefined;
          };
          const dealt = (side: number) =>
            dealtBy(
              record.events.filter((_, index) => visible(index)),
              side,
            );
          const others = () =>
            record.events.filter(
              (e, index) =>
                visible(index) &&
                !quietEvents.includes(e.kind) &&
                !attackEvents.includes(e.kind) &&
                (e.kind !== 'resource' || e.cardId !== 0),
            );
          return (
            <article class={styles.chronicleRound}>
              <table>
                <caption>{t('chronicle-round', { round: record.round })}</caption>
                <thead>
                  <tr>
                    <td />
                    <th scope="col">{t('chronicle-yours')}</th>
                    <th scope="col">{t('chronicle-theirs')}</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={[0, 1, 2]}>
                    {(slot) => (
                      <tr>
                        <th scope="row">{t('chronicle-attack', { slot: slot + 1 })}</th>
                        <For each={[0, 1]}>
                          {(side) => (
                            <td
                              data-result={attack(side, slot)?.event.kind}
                              data-active={i() === 0 && attack(side, slot)?.index === props.playing}
                            >
                              <Show when={attack(side, slot)} fallback="…">
                                {(cell) => (
                                  <>
                                    <img
                                      class={styles.chronicleIcon}
                                      src={icon(
                                        cell().event.attackKind === 0
                                          ? 'attack-phys'
                                          : 'attack-mag',
                                      )}
                                      alt={t(
                                        cell().event.attackKind === 0 ? 'physical' : 'magical',
                                      )}
                                    />
                                    {cell().event.kind === 'hit'
                                      ? `−${cell().event.amount}`
                                      : t(cell().event.kind === 'block' ? 'fx-block' : 'fx-miss')}
                                  </>
                                )}
                              </Show>
                            </td>
                          )}
                        </For>
                      </tr>
                    )}
                  </For>
                </tbody>
                <tfoot>
                  <tr>
                    <th scope="row">{t('chronicle-total')}</th>
                    <td>{dealt(0)}</td>
                    <td>{dealt(1)}</td>
                  </tr>
                </tfoot>
              </table>
              <Show when={others().length}>
                <ul class={styles.chronicleEffects} aria-label={t('chronicle-other')}>
                  <For each={others()}>{(event) => <li>{props.text(event)}</li>}</For>
                </ul>
              </Show>
            </article>
          );
        }}
      </For>
    </section>
  );
}

function cardDescription(card?: Card) {
  return card ? t(fluentKey(card.key, 'text'), cardArgs(card)) : '';
}
function Requirements(props: { card: Card }) {
  return (
    <div class={styles.requirements}>
      <For each={Object.entries(props.card.requires)}>
        {([school, level]) => (
          <Show when={level}>
            <span>
              <img src={icon(`school-${school}`)} alt={t(`school-${school}-name`)} />
              {level}
            </span>
          </Show>
        )}
      </For>
    </div>
  );
}
function CardDetails(props: { card: Card }) {
  return (
    <div class={styles.cardDetails}>
      <Show when={cardPicture(props.card.key)}>
        {(src) => <img class={styles.detailsPicture} src={src()} alt="" />}
      </Show>
      <p class={styles.cardCost}>
        {props.card.cost.focus} K · {props.card.cost.stamina} W
      </p>
      <Requirements card={props.card} />
      <p>{cardDescription(props.card)}</p>
    </div>
  );
}

export function CardDialog(props: { card: Card; close: () => void }) {
  return (
    <Modal title={cardName(props.card.id)} close={props.close}>
      <CardDetails card={props.card} />
      <button type="button" class="secondary" onClick={props.close}>
        {t('close')}
      </button>
    </Modal>
  );
}

export function ChronicleDialog(props: {
  history: RoundRecord[];
  events: DuelEvent[];
  text: (event: DuelEvent) => string;
  close: () => void;
}) {
  return (
    <Modal title={t('chronicle')} wide close={props.close}>
      <Chronicle history={props.history} text={props.text} />
      <Show when={props.events.length}>
        <details class={styles.log}>
          <summary>{t('log')}</summary>
          <ol>
            <For each={props.events}>{(event) => <li>{props.text(event)}</li>}</For>
          </ol>
        </details>
      </Show>
      <p class={styles.shortcuts}>{t('shortcuts')}</p>
    </Modal>
  );
}

export type Simulation = ReturnType<typeof createSimulation>;

// Plays the public events of a round one by one on a copy of the view from the round start.
export function createSimulation() {
  const [playIndex, setPlayIndex] = createSignal(0);
  const [activeEvent, setActiveEvent] = createSignal<DuelEvent>();
  const [animating, setAnimating] = createSignal(false);
  const [animationView, setAnimationView] = createSignal<TrainingView>();
  const [fx, setFx] = createSignal<(Fx | undefined)[]>([]);
  const [strikePath, setStrikePath] = createSignal<Record<string, string>>({});
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let finishAnimation: (() => void) | undefined;
  let disposed = false;
  let arena: HTMLDivElement | undefined;
  let fighters: HTMLDivElement | undefined;
  const portraits: HTMLDivElement[] = [];
  let running: Animation[] = [];
  function skip() {
    generation++;
    clearTimeout(timer);
    for (const animation of running) animation.cancel();
    running = [];
    finishAnimation?.();
    finishAnimation = undefined;
    setAnimating(false);
    setActiveEvent(undefined);
    setAnimationView(undefined);
    setFx([]);
  }
  function wait(ms: number) {
    return new Promise<void>((resolve) => {
      finishAnimation = resolve;
      timer = setTimeout(resolve, ms);
    });
  }
  function motion(index: number, keyframes: Keyframe[], options: KeyframeAnimationOptions) {
    const animation = portraits[index]?.animate(keyframes, options);
    if (animation) running.push(animation);
  }
  // Attack: the attacker lunges, the strike flies across and the defender reacts on impact.
  function play(event: DuelEvent, ms: number) {
    const effects: (Fx | undefined)[] = [];
    const attack = attackEvents.includes(event.kind);
    const target = attack ? 1 - event.side : event.side;
    const impact = attack ? Math.round(ms / 2) : 0;
    const popup = (tone: Fx['tone'], text: string) => {
      effects[target] = { tone, text, ms: ms - impact, delay: impact };
    };
    if (event.kind === 'hit' || event.kind === 'damage') popup('damage', `−${event.amount}`);
    if (event.kind === 'heal') popup('heal', `+${event.amount}`);
    if (event.kind === 'block') popup('block', t('fx-block'));
    if (event.kind === 'miss') popup('miss', t('fx-miss'));
    setFx(effects);
    running = running.filter((animation) => animation.playState === 'running');
    if (attack) {
      // The strike flies from the centre of one portrait to the other.
      const box = fighters?.getBoundingClientRect();
      const [from, to] = [event.side, target].map((i) => portraits[i]?.getBoundingClientRect());
      if (box && from && to) {
        const centre = (rect: DOMRect, axis: 'x' | 'y') =>
          `${axis === 'x' ? rect.left - box.left + rect.width / 2 : rect.top - box.top + rect.height / 2}px`;
        setStrikePath({
          '--x1': centre(from, 'x'),
          '--y1': centre(from, 'y'),
          '--x2': centre(to, 'x'),
          '--y2': centre(to, 'y'),
          'animation-duration': `${ms}ms`,
        });
      }
      const direction = event.side === 0 ? 1 : -1;
      motion(
        event.side,
        [
          { transform: 'none' },
          { transform: `translateX(${direction * 26}px) scale(1.08)`, offset: 0.4 },
          { transform: 'none' },
        ],
        { duration: impact * 1.4, easing: 'ease-in-out' },
      );
    }
    if (event.kind === 'hit' || event.kind === 'damage') {
      motion(
        target,
        [
          { transform: 'none', filter: 'none' },
          { transform: 'translateX(-7px) rotate(-3deg)', filter: 'var(--hurt-glow)' },
          { transform: 'translateX(6px) rotate(2deg)', filter: 'var(--hurt-glow)' },
          { transform: 'translateX(-4px)', filter: 'var(--hurt-glow)' },
          { transform: 'none', filter: 'none' },
        ],
        { duration: Math.min(420, ms - impact), delay: impact },
      );
    }
    if (event.kind === 'block') {
      motion(target, [{ transform: 'none' }, { transform: 'scale(0.94)' }, { transform: 'none' }], {
        duration: Math.min(300, ms - impact),
        delay: impact,
      });
    }
    if (event.kind === 'heal') {
      motion(target, [{ filter: 'none' }, { filter: 'var(--heal-glow)' }, { filter: 'none' }], {
        duration: ms,
      });
    }
    return impact;
  }
  // Resolves when the round has been shown or skipped; `start` is the view before the round.
  async function animate(list: DuelEvent[], start?: TrainingView) {
    skip();
    const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (speed() === 'instant' || reduced || list.length === 0) return;
    const current = generation;
    setAnimating(true);
    setAnimationView(start ? structuredClone(start) : undefined);
    const factor = speed() === 'fast' ? 2 : 1;
    const delay = Math.min(500, 4500 / list.length) / factor;
    for (const [index, event] of list.entries()) {
      if (disposed || current !== generation) return;
      setPlayIndex(index);
      const ms = attackEvents.includes(event.kind) ? 900 / factor : delay;
      const impact = play(event, ms);
      setActiveEvent(event);
      const textAnimation = arena?.animate(
        [
          { opacity: 0.4, transform: 'translateY(4px)' },
          { opacity: 1, transform: 'translateY(0)' },
        ],
        { duration: Math.min(ms, 240), easing: 'ease-out' },
      );
      if (textAnimation) running.push(textAnimation);
      // Health drops at the moment of impact, not when the attacker starts moving.
      await wait(impact);
      if (disposed || current !== generation) return;
      setAnimationView((view) => view && advanceAnimation(view, event));
      await wait(ms - impact);
    }
    if (disposed || current !== generation) return;
    setActiveEvent(undefined);
    await wait(1200 / factor);
    if (current === generation && !disposed) {
      setAnimating(false);
      setActiveEvent(undefined);
      setAnimationView(undefined);
      setFx([]);
    }
  }
  onCleanup(() => {
    disposed = true;
    skip();
  });
  return {
    animating,
    activeEvent,
    animationView,
    fx,
    strikePath,
    skip,
    animate,
    // While the newest round animates, only its events up to the current one count.
    shown: (list: DuelEvent[], newest: boolean) =>
      newest && animating() ? list.slice(0, playIndex() + 1) : list,
    strike: () => {
      const event = activeEvent();
      return event && attackEvents.includes(event.kind) ? event : undefined;
    },
    setArena: (element: HTMLDivElement) => {
      arena = element;
    },
    setFighters: (element: HTMLDivElement) => {
      fighters = element;
    },
    setPortrait: (index: number, element: HTMLDivElement) => {
      portraits[index] = element;
    },
  };
}

// Keys: 1–6 slots, Q–T cards (Shift for details), Enter confirm, P replay, Space skip.
export function useDuelKeys(keys: {
  ignore: () => boolean;
  animating: () => boolean;
  skip: () => void;
  canReplay: () => boolean;
  replay: () => void;
  locked: () => boolean;
  // Choices are made by a bot (training demo); only details and confirmation stay available.
  fixed: () => boolean;
  pending: () => boolean;
  view: () => TrainingView | undefined;
  draft: () => DuelMove;
  castOptions: () => MovePreview[];
  change: (move: DuelMove) => void;
  inspect: (card: Card) => void;
  submit: () => void;
}) {
  function keydown(event: KeyboardEvent) {
    if (event.altKey || event.ctrlKey || event.metaKey || event.repeat || keys.ignore()) return;
    const target = event.target as HTMLElement | null;
    if (target?.matches('input,select,textarea,[contenteditable=true]')) return;
    const key = event.key.toLowerCase();
    if (key === ' ' && keys.animating()) {
      event.preventDefault();
      keys.skip();
      return;
    }
    if (key === 'p' && keys.canReplay()) {
      event.preventDefault();
      keys.replay();
      return;
    }
    if (keys.locked()) return;
    if (/^[1-6]$/.test(key) && !keys.fixed()) {
      event.preventDefault();
      const slot = Number(key) - 1;
      keys.change(toggled(keys.draft(), slot < 3 ? 'attacks' : 'defenses', slot % 3));
    }
    const cardIndex = cardKeys.indexOf(key);
    const card = cards.get(keys.view()?.hand[cardIndex] ?? 0);
    if (cardIndex >= 0 && card) {
      event.preventDefault();
      if (event.shiftKey) keys.inspect(card);
      else if (!keys.pending() && !keys.fixed())
        keys.change(cycled(keys.draft(), cardIndex, keys.castOptions()));
    }
    // Preserve native Enter behavior on focused controls.
    if (key === 'enter' && !target?.closest('button,a')) {
      event.preventDefault();
      keys.submit();
    }
  }
  onMount(() => window.addEventListener('keydown', keydown));
  onCleanup(() => window.removeEventListener('keydown', keydown));
}

// Decision screen: both characters, the attack and defense trays with the opponent's previous
// round between them, the move cost, the hand and the command bar.
export function DecisionTable(props: {
  view: TrainingView;
  plates: [string, string];
  title: JSX.Element;
  heading: JSX.Element;
  labelledBy: string;
  avatars: [number, number];
  labels: [string, string];
  draft: DuelMove;
  preview?: MovePreview;
  castOptions: MovePreview[];
  // The opponent's previous round is known (after the first round).
  previous: boolean;
  locked: boolean;
  pending: boolean;
  clock: number;
  clockLabel: string;
  // Shown in the centre instead of the trays once the duel has ended.
  result: JSX.Element;
  message: JSX.Element;
  buttons: JSX.Element;
  onChange: (move: DuelMove) => void;
  onInspect: (card: Card) => void;
}) {
  let hold: ReturnType<typeof setTimeout> | undefined;
  let held = false;
  onCleanup(() => clearTimeout(hold));
  const focus = (index: number, card: Card) => {
    const option = props.castOptions[index];
    return option
      ? option.focusCost - option.changes * content.rules.duel.change_cost
      : card.cost.focus;
  };
  const stamina = (index: number, card: Card) =>
    props.castOptions[index]?.staminaCost ?? card.cost.stamina;
  const chosen = (index: number, action: number) =>
    props.draft.action === action && props.draft.handIndex === index;
  const choose = (index: number, action: number) =>
    props.onChange(
      create(DuelMoveSchema, {
        ...props.draft,
        action: chosen(index, action) ? 0 : action,
        handIndex: index,
      }),
    );
  const slotRow = (group: Group) => (
    <fieldset class={styles.slots} aria-label={t(group === 'attacks' ? 'attack' : 'defense')}>
      <For each={props.draft[group]}>
        {(kind, i) => (
          <button
            type="button"
            disabled={props.locked}
            aria-label={t('slot', {
              group: t(group === 'attacks' ? 'attack' : 'defense'),
              number: i() + 1,
              kind: kindName(kind),
            })}
            class={styles.slot}
            data-kind={kind}
            onClick={() => props.onChange(toggled(props.draft, group, i()))}
          >
            <kbd>{i() + 1 + (group === 'attacks' ? 0 : 3)}</kbd>
            <img src={icon(kind === 0 ? 'attack-phys' : 'attack-mag')} alt="" />
          </button>
        )}
      </For>
    </fieldset>
  );
  return (
    <section class={styles.table} aria-labelledby={props.labelledBy}>
      <TitleBar left={props.plates[0]} right={props.plates[1]}>
        {props.title}
      </TitleBar>
      <div class={styles.tableGrid}>
        <Show when={props.view.me}>
          {(me) => (
            <>
              <SideCard
                side={me()}
                avatar={props.avatars[0]}
                owner={0}
                initiative={props.view.initiative}
                enchanted={props.view.enchantments}
                label={props.labels[0]}
              />
              <HealthGauge side={me()} />
            </>
          )}
        </Show>
        <div class={styles.command}>
          <div class={styles.roundTop}>
            <span class={styles.focusOrb} title={t('stat-Focus')}>
              {props.view.me?.focus}
              <small>K</small>
            </span>
            <div class={styles.roundTitle}>
              {props.heading}
              <small>
                {t('initiative', { name: props.labels[props.view.initiative === 0 ? 0 : 1] })}
              </small>
            </div>
            <span class={styles.focusOrb} title={t('stat-Focus')}>
              {props.view.opponent?.focus}
              <small>K</small>
            </span>
          </div>
          <Show when={props.view.outcome}>{props.result}</Show>
          <Show when={!props.view.outcome}>
            <div class={styles.sequence}>
              <span class={styles.rowLabel}>{t('attack')}</span>
              {slotRow('attacks')}
              <OpponentRow
                label={t('opponent-prev-defenses')}
                kinds={props.previous ? props.view.opponent?.defenses : undefined}
              />
              <small class={styles.opponentCaption}>{t('opponent-prev')}</small>
              <OpponentRow
                label={t('opponent-prev-attacks')}
                kinds={props.previous ? props.view.opponent?.attacks : undefined}
              />
              {slotRow('defenses')}
              <span class={styles.rowLabel}>{t('defense')}</span>
            </div>
            <div class={styles.timerBox}>
              <strong title={props.clockLabel}>{props.clock}</strong>
              <span>
                {t('focus-cost', {
                  cost: props.preview?.focusCost ?? 0,
                  after: props.preview?.focusAfter ?? 0,
                })}
              </span>
              <small>
                {t('stamina-cost', {
                  spell: props.preview?.staminaCost ?? 0,
                  attacks: props.preview?.maxAttackStamina ?? 0,
                })}
              </small>
            </div>
          </Show>
        </div>
        <Show when={props.view.opponent}>
          {(opponent) => (
            <>
              <HealthGauge side={opponent()} />
              <SideCard
                side={opponent()}
                avatar={props.avatars[1]}
                owner={1}
                initiative={props.view.initiative}
                enchanted={props.view.enchantments}
                label={props.labels[1]}
              />
            </>
          )}
        </Show>
      </div>
      <Show when={!props.view.outcome}>
        <section class={styles.hand} aria-labelledby="hand-title">
          <div class={styles.sectionHeading}>
            <h2 id="hand-title">{t('hand')}</h2>
            <span>{props.view.me?.deckLen} / 20</span>
          </div>
          <div class={styles.cards}>
            <For each={props.view.hand}>
              {(id, index) => (
                <Show when={cards.get(id)}>
                  {(card) => (
                    <article
                      class={styles.card}
                      data-selected={props.draft.action !== 0 && props.draft.handIndex === index()}
                      data-action={props.draft.handIndex === index() ? props.draft.action : 0}
                      data-rarity={card().rarity}
                    >
                      <button
                        type="button"
                        class={styles.cardMain}
                        disabled={props.locked || props.pending}
                        onClick={() => {
                          if (!held)
                            props.onChange(cycled(props.draft, index(), props.castOptions));
                          held = false;
                        }}
                        onContextMenu={(event) => {
                          event.preventDefault();
                          props.onInspect(card());
                        }}
                        onPointerDown={() => {
                          held = false;
                          hold = setTimeout(() => {
                            held = true;
                            props.onInspect(card());
                          }, 600);
                        }}
                        onPointerUp={() => clearTimeout(hold)}
                        onPointerCancel={() => clearTimeout(hold)}
                        onPointerLeave={() => clearTimeout(hold)}
                        aria-label={`${cardName(id)}. ${focus(index(), card())} K, ${stamina(index(), card())} W. ${cardDescription(card())}. ${props.castOptions[index()]?.error ? errorText(props.castOptions[index()]?.error ?? '') : t('cast')}`}
                      >
                        <CardFace
                          card={card()}
                          focus={focus(index(), card())}
                          stamina={stamina(index(), card())}
                          hotkey={cardKeys[index()]?.toUpperCase()}
                        />
                      </button>
                      <div class={styles.cardActions}>
                        <button
                          type="button"
                          class="quiet"
                          aria-pressed={chosen(index(), 1)}
                          disabled={
                            props.locked || props.pending || !!props.castOptions[index()]?.error
                          }
                          onClick={() => choose(index(), 1)}
                        >
                          {t('cast')}
                        </button>
                        <button
                          type="button"
                          class="quiet"
                          aria-pressed={chosen(index(), 2)}
                          disabled={props.locked || props.pending}
                          onClick={() => choose(index(), 2)}
                        >
                          {t('discard')}
                        </button>
                        <button
                          type="button"
                          class="quiet"
                          aria-label={`${t('inspect')}: ${cardName(id)}`}
                          onClick={() => props.onInspect(card())}
                        >
                          ⓘ
                        </button>
                      </div>
                      <Show when={props.castOptions[index()]?.error}>
                        <small class={styles.cardUnavailable}>
                          {errorText(props.castOptions[index()]?.error ?? '')}
                        </small>
                      </Show>
                    </article>
                  )}
                </Show>
              )}
            </For>
          </div>
        </section>
      </Show>
      <div class={styles.commandBar}>
        <p class={styles.validation} aria-live="polite">
          {props.message}
        </p>
        <div class={styles.commandButtons}>{props.buttons}</div>
      </div>
    </section>
  );
}

// Simulation screen: both fighters with their stat columns, the strike in flight and the event.
export function SimulationBoard(props: {
  sim: Simulation;
  // The static parts (initiative); `start` is the round start for the stat trends.
  view: TrainingView;
  start?: TrainingView;
  plates: [string, string];
  title: JSX.Element;
  labelledBy: string;
  avatars: [number, number];
  labels: [string, string];
  // The newest round and all rounds, newest first.
  events: DuelEvent[];
  history: RoundRecord[];
  text: (event: DuelEvent) => string;
  message: JSX.Element;
}) {
  const playedCard = (side: number) =>
    props.sim.shown(props.events, true).findLast((e) => e.kind === 'cast' && e.side === side)
      ?.cardId ?? 0;
  const dealtTotal = (side: number) =>
    props.history.reduce(
      (sum, record, i) => sum + dealtBy(props.sim.shown(record.events, i === 0), side),
      0,
    );
  const lastSlot = (side: number) => {
    const event = props.sim
      .shown(props.events, true)
      .findLast((e) => attackEvents.includes(e.kind) && e.side === side);
    return event ? event.slot + 1 : '–';
  };
  const strikeResult = (event: DuelEvent) =>
    event.kind === 'hit' ? `−${event.amount}` : t(event.kind === 'block' ? 'fx-block' : 'fx-miss');
  return (
    <section class={styles.board} aria-labelledby={props.labelledBy}>
      <TitleBar left={props.plates[0]} right={props.plates[1]}>
        {props.title}
      </TitleBar>
      <div class={styles.fighters} ref={props.sim.setFighters}>
        <Show when={props.sim.strike()} keyed>
          {(event) => (
            <span
              class={styles.strike}
              data-kind={event.attackKind}
              style={props.sim.strikePath()}
              aria-hidden="true"
            >
              <img src={icon(event.attackKind === 0 ? 'attack-phys' : 'attack-mag')} alt="" />
              <b>{event.slot + 1}</b>
            </span>
          )}
        </Show>
        <Show when={props.view.me}>
          {(me) => (
            <>
              <Combatant
                side={props.sim.animationView()?.me ?? me()}
                avatar={props.avatars[0]}
                owner={0}
                initiative={props.view.initiative}
                label={props.labels[0]}
                played={playedCard(0)}
                fx={props.sim.fx()[0]}
                portraitRef={(element) => props.sim.setPortrait(0, element)}
              />
              <StatColumn
                side={props.sim.animationView()?.me ?? me()}
                base={props.start?.me}
                owner={0}
                label={props.labels[0]}
                dealt={dealtTotal(0)}
              />
            </>
          )}
        </Show>
        <Show when={props.view.opponent}>
          {(opponent) => (
            <>
              <StatColumn
                side={props.sim.animationView()?.opponent ?? opponent()}
                base={props.start?.opponent}
                owner={1}
                label={props.labels[1]}
                dealt={dealtTotal(1)}
              />
              <Combatant
                side={props.sim.animationView()?.opponent ?? opponent()}
                avatar={props.avatars[1]}
                owner={1}
                initiative={props.view.initiative}
                label={props.labels[1]}
                played={playedCard(1)}
                fx={props.sim.fx()[1]}
                portraitRef={(element) => props.sim.setPortrait(1, element)}
              />
            </>
          )}
        </Show>
        <div class={styles.exchange} aria-hidden="true">
          <span class={styles.slotOrb}>{lastSlot(0)}</span>
          <Show
            when={props.sim.strike()}
            keyed
            fallback={<span class={styles.exchangeIdle}>✦</span>}
          >
            {(event) => (
              <span class={styles.exchangeHit} data-result={event.kind}>
                <img
                  class={`${styles.panelIcon} ${styles.exchangeIcon}`}
                  src={icon(event.attackKind === 0 ? 'attack-phys' : 'attack-mag')}
                  alt=""
                />
                <b>{strikeResult(event)}</b>
              </span>
            )}
          </Show>
          <span class={styles.slotOrb}>{lastSlot(1)}</span>
        </div>
      </div>
      <div class={styles.arena} ref={props.sim.setArena} aria-live="polite" role="status">
        <Show
          when={props.sim.activeEvent()}
          fallback={<span class={styles.arenaIdle}>{t('round-over')}</span>}
        >
          {(event) => (
            <>
              <Show when={props.sim.strike()} keyed>
                {(hit) => (
                  <span class={styles.arenaStrike} data-result={hit.kind}>
                    <img
                      class={`${styles.panelIcon} ${styles.arenaIcon}`}
                      src={icon(hit.attackKind === 0 ? 'attack-phys' : 'attack-mag')}
                      alt=""
                    />
                    <b>{strikeResult(hit)}</b>
                  </span>
                )}
              </Show>
              <span class={styles.arenaText}>{props.text(event())}</span>
            </>
          )}
        </Show>
      </div>
      <div class={styles.commandBar}>
        <p class={styles.validation}>{props.message}</p>
        <div class={styles.commandButtons}>
          <button type="button" class="secondary" onClick={props.sim.skip}>
            {t('skip')}
          </button>
        </div>
      </div>
    </section>
  );
}
