import { A, useBeforeLeave, useSearchParams } from '@solidjs/router';
import {
  batch,
  createEffect,
  createSignal,
  For,
  Index,
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
  isPlayableAvatar,
  portrait,
} from '../../app/content';
import { readPreference, writePreference } from '../../app/preferences';
import { setDuelActive, speed } from '../../app/session';
import { Modal } from '../../components/Modal';
import type {
  DuelEvent,
  DuelMove,
  DuelSide,
  MovePreview,
  TrainingReply,
  TrainingView,
} from '../../generated/proto/game/v1/training_pb';
import { hasTranslation, t } from '../../i18n';
import { TrainingClient } from '../../worker/client';
import {
  create,
  DuelMoveSchema,
  TrainingForfeitSchema,
  TrainingSetupSchema,
} from '../../worker/wire';
import { advanceAnimation } from './animation';
import styles from './Training.module.css';

const profiles = ['novice', 'apprentice', 'tactician', 'archmage'];
const cardKeys = ['q', 'w', 'e', 'r', 't'];
const attackEvents = ['hit', 'block', 'miss'];
type Fx = { tone: 'damage' | 'heal' | 'block' | 'miss'; text: string; ms: number; delay: number };
type RoundRecord = { round: number; events: DuelEvent[] };
// Bookkeeping events that would only clutter the chronicle.
const quietEvents = ['reveal', 'round', 'draw', 'end'];
const name = (id: number) => t(fluentKey(avatars.get(id)?.key ?? ''));
const cardName = (id: number) => t(fluentKey(cards.get(id)?.key ?? ''));
const [pictures, setPictures] = createSignal(new Map<string, string>());
const cardPicture = (key: string) => pictures().get(key);
export const errorText = (code: string) =>
  t(hasTranslation(`error-${code}`) ? `error-${code}` : 'error-generic');

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
  const [playIndex, setPlayIndex] = createSignal(0);
  const [activeEvent, setActiveEvent] = createSignal<DuelEvent>();
  const [animating, setAnimating] = createSignal(false);
  const [animationView, setAnimationView] = createSignal<TrainingView>();
  const [fx, setFx] = createSignal<(Fx | undefined)[]>([]);
  const [strikePath, setStrikePath] = createSignal<Record<string, string>>({});
  // While the newest round animates, only its events up to the current one count.
  const shown = (list: DuelEvent[], newest: boolean) =>
    newest && animating() ? list.slice(0, playIndex() + 1) : list;
  const playedCard = (side: number) =>
    shown(events(), true).findLast((e) => e.kind === 'cast' && e.side === side)?.cardId ?? 0;
  const dealtTotal = (side: number) =>
    history().reduce((sum, record, i) => sum + dealtBy(shown(record.events, i === 0), side), 0);
  const lastSlot = (side: number) => {
    const event = shown(events(), true).findLast(
      (e) => attackEvents.includes(e.kind) && e.side === side,
    );
    return event ? event.slot + 1 : '–';
  };
  const strikeResult = (event: DuelEvent) =>
    event.kind === 'hit' ? `−${event.amount}` : t(event.kind === 'block' ? 'fx-block' : 'fx-miss');
  const slotRow = (group: 'attacks' | 'defenses') => (
    <fieldset class={styles.slots} aria-label={t(group === 'attacks' ? 'attack' : 'defense')}>
      <For each={draft()[group]}>
        {(kind, i) => (
          <button
            type="button"
            disabled={locked() || demo()}
            aria-label={t('slot', {
              group: t(group === 'attacks' ? 'attack' : 'defense'),
              number: i() + 1,
              kind: kindName(kind),
            })}
            class={styles.slot}
            data-kind={kind}
            onClick={() => toggle(group, i())}
          >
            <kbd>{i() + 1 + (group === 'attacks' ? 0 : 3)}</kbd>
            <img src={icon(kind === 0 ? 'attack-phys' : 'attack-mag')} alt="" />
          </button>
        )}
      </For>
    </fieldset>
  );
  const strike = () => {
    const event = activeEvent();
    return event && attackEvents.includes(event.kind) ? event : undefined;
  };
  let lastRoundStart: TrainingView | undefined;
  const [seconds, setSeconds] = createSignal(0);
  let client: TrainingClient;
  let previewGeneration = 0;
  let animationGeneration = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let finishAnimation: (() => void) | undefined;
  let leave: (() => void) | undefined;
  let disposed = false;
  let hold: ReturnType<typeof setTimeout> | undefined;
  let held = false;
  let arena: HTMLDivElement | undefined;
  let fighters: HTMLDivElement | undefined;
  const portraits: HTMLDivElement[] = [];
  let running: Animation[] = [];
  const locked = () => busy() || animating() || !!view()?.outcome;
  const kindName = (kind: number) => t(kind === 0 ? 'physical' : 'magical');

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
    skip();
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
  function cardFocus(index: number, card: Card) {
    const option = castOptions()[index];
    return option
      ? option.focusCost - option.changes * content.rules.duel.change_cost
      : card.cost.focus;
  }
  function toggle(group: 'attacks' | 'defenses', slot: number) {
    const next = create(DuelMoveSchema, {
      ...draft(),
      [group]: draft()[group].map((kind, index) => (index === slot ? 1 - kind : kind)),
    });
    void updateDraft(next);
  }
  function cycle(index: number) {
    if (pendingPreview() || demo()) return;
    const selected = draft().action !== 0 && draft().handIndex === index;
    const action = selected ? (draft().action + 1) % 3 : castOptions()[index]?.error ? 2 : 1;
    void updateDraft(create(DuelMoveSchema, { ...draft(), action, handIndex: index }));
  }
  function skip() {
    animationGeneration++;
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
  async function animate(list: DuelEvent[]) {
    skip();
    const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (speed() === 'instant' || reduced || list.length === 0) return;
    const generation = animationGeneration;
    setAnimating(true);
    setAnimationView(lastRoundStart ? structuredClone(lastRoundStart) : undefined);
    const factor = speed() === 'fast' ? 2 : 1;
    const delay = Math.min(500, 4500 / list.length) / factor;
    for (const [index, event] of list.entries()) {
      if (disposed || generation !== animationGeneration) return;
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
      if (disposed || generation !== animationGeneration) return;
      setAnimationView((current) => current && advanceAnimation(current, event));
      await wait(ms - impact);
    }
    if (disposed || generation !== animationGeneration) return;
    setActiveEvent(undefined);
    await wait(1200 / factor);
    if (generation === animationGeneration && !disposed) {
      setAnimating(false);
      setActiveEvent(undefined);
      setAnimationView(undefined);
      setFx([]);
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
      await animate(reply.events);
      apply(reply);
    } catch (e) {
      if (!disposed) setError(e instanceof Error ? e.message : 'generic');
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  async function forfeit() {
    setConfirmLeave(false);
    skip();
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
  function eventText(event: DuelEvent) {
    const detailKey = `stat-${event.detail}`;
    const schoolKey = `school-${event.detail.toLowerCase()}-name`;
    return t(`event-${event.kind}`, {
      name: t(event.side === 0 ? 'you' : 'opponent'),
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
  function keydown(event: KeyboardEvent) {
    if (
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.repeat ||
      inspect() ||
      showChronicle() ||
      confirmLeave()
    )
      return;
    const target = event.target as HTMLElement | null;
    if (target?.matches('input,select,textarea,[contenteditable=true]')) return;
    const key = event.key.toLowerCase();
    if (key === ' ' && animating()) {
      event.preventDefault();
      skip();
      return;
    }
    if (key === 'p' && events().length && !busy() && !animating()) {
      event.preventDefault();
      void animate(events());
      return;
    }
    if (locked()) return;
    if (/^[1-6]$/.test(key) && !demo()) {
      event.preventDefault();
      const slot = Number(key) - 1;
      toggle(slot < 3 ? 'attacks' : 'defenses', slot % 3);
    }
    const cardIndex = cardKeys.indexOf(key);
    if (cardIndex >= 0 && view()?.hand[cardIndex] !== undefined) {
      event.preventDefault();
      if (event.shiftKey) setInspect(cards.get(view()?.hand[cardIndex] ?? 0));
      else cycle(cardIndex);
    }
    // Preserve native Enter behavior on focused controls.
    if (key === 'enter' && !target?.closest('button,a')) {
      event.preventDefault();
      void submit();
    }
  }
  onMount(() => {
    if (!pictures().size) {
      void import('../../app/cardArt').then((module) => setPictures(module.cardPictures));
    }
    client = new TrainingClient();
    void start();
    window.addEventListener('keydown', keydown);
    const clock = setInterval(() => {
      if (!locked() && !document.hidden) setSeconds((s) => s + 1);
    }, 1000);
    onCleanup(() => clearInterval(clock));
  });
  onCleanup(() => {
    disposed = true;
    skip();
    clearTimeout(hold);
    window.removeEventListener('keydown', keydown);
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
              when={animating()}
              fallback={
                <section class={styles.table} aria-labelledby="round-title">
                  <TitleBar left={demo() ? 'AI' : t('you')} right={t(profile())}>
                    <p class={styles.boardTitle}>{t('duel-title')}</p>
                  </TitleBar>
                  <div class={styles.tableGrid}>
                    <Show when={state().me}>
                      {(me) => (
                        <>
                          <SideCard
                            side={me()}
                            avatar={avatarId()}
                            owner={0}
                            initiative={state().initiative}
                            enchanted={state().enchantments}
                            label={t('you')}
                          />
                          <HealthGauge side={me()} />
                        </>
                      )}
                    </Show>
                    <div class={styles.command}>
                      <div class={styles.roundTop}>
                        <span class={styles.focusOrb} title={t('stat-Focus')}>
                          {state().me?.focus}
                          <small>K</small>
                        </span>
                        <div class={styles.roundTitle}>
                          <h1 id="round-title">
                            {t('round', { round: state().round, total: state().totalRounds })}
                          </h1>
                          <small>
                            {t('initiative', {
                              name: t(state().initiative === 0 ? 'you' : 'opponent'),
                            })}
                          </small>
                        </div>
                        <span class={styles.focusOrb} title={t('stat-Focus')}>
                          {state().opponent?.focus}
                          <small>K</small>
                        </span>
                      </div>
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
                      <Show when={!state().outcome}>
                        <div class={styles.sequence}>
                          <span class={styles.rowLabel}>{t('attack')}</span>
                          {slotRow('attacks')}
                          <OpponentRow
                            label={t('opponent-prev-defenses')}
                            kinds={history().length ? state().opponent?.defenses : undefined}
                          />
                          <small class={styles.opponentCaption}>{t('opponent-prev')}</small>
                          <OpponentRow
                            label={t('opponent-prev-attacks')}
                            kinds={history().length ? state().opponent?.attacks : undefined}
                          />
                          {slotRow('defenses')}
                          <span class={styles.rowLabel}>{t('defense')}</span>
                        </div>
                        <div class={styles.timerBox}>
                          <strong title={t('elapsed', { seconds: seconds() })}>{seconds()}</strong>
                          <span>
                            {t('focus-cost', {
                              cost: preview()?.focusCost ?? 0,
                              after: preview()?.focusAfter ?? 0,
                            })}
                          </span>
                          <small>
                            {t('stamina-cost', {
                              spell: preview()?.staminaCost ?? 0,
                              attacks: preview()?.maxAttackStamina ?? 0,
                            })}
                          </small>
                        </div>
                      </Show>
                    </div>
                    <Show when={state().opponent}>
                      {(opponent) => (
                        <>
                          <HealthGauge side={opponent()} />
                          <SideCard
                            side={opponent()}
                            avatar={state().opponentAvatarId}
                            owner={1}
                            initiative={state().initiative}
                            enchanted={state().enchantments}
                            label={t('opponent')}
                          />
                        </>
                      )}
                    </Show>
                  </div>
                  <Show when={!state().outcome}>
                    <section class={styles.hand} aria-labelledby="hand-title">
                      <div class={styles.sectionHeading}>
                        <h2 id="hand-title">{t('hand')}</h2>
                        <span>{state().me?.deckLen} / 20</span>
                      </div>
                      <div class={styles.cards}>
                        <For each={state().hand}>
                          {(id, index) => (
                            <Show when={cards.get(id)}>
                              {(card) => (
                                <article
                                  class={styles.card}
                                  data-selected={
                                    draft().action !== 0 && draft().handIndex === index()
                                  }
                                  data-action={draft().handIndex === index() ? draft().action : 0}
                                  data-rarity={card().rarity}
                                >
                                  <button
                                    type="button"
                                    class={styles.cardMain}
                                    disabled={locked() || demo() || pendingPreview()}
                                    onClick={() => {
                                      if (!held) cycle(index());
                                      held = false;
                                    }}
                                    onContextMenu={(event) => {
                                      event.preventDefault();
                                      setInspect(card());
                                    }}
                                    onPointerDown={() => {
                                      held = false;
                                      hold = setTimeout(() => {
                                        held = true;
                                        setInspect(card());
                                      }, 600);
                                    }}
                                    onPointerUp={() => clearTimeout(hold)}
                                    onPointerCancel={() => clearTimeout(hold)}
                                    onPointerLeave={() => clearTimeout(hold)}
                                    aria-label={`${cardName(id)}. ${cardFocus(index(), card())} K, ${castOptions()[index()]?.staminaCost ?? card().cost.stamina} W. ${cardDescription(card())}. ${castOptions()[index()]?.error ? errorText(castOptions()[index()]?.error ?? '') : t('cast')}`}
                                  >
                                    <CardFace
                                      card={card()}
                                      focus={cardFocus(index(), card())}
                                      stamina={
                                        castOptions()[index()]?.staminaCost ?? card().cost.stamina
                                      }
                                      hotkey={cardKeys[index()]?.toUpperCase()}
                                    />
                                  </button>
                                  <div class={styles.cardActions}>
                                    <button
                                      type="button"
                                      class="quiet"
                                      aria-pressed={
                                        draft().action === 1 && draft().handIndex === index()
                                      }
                                      disabled={
                                        locked() ||
                                        demo() ||
                                        pendingPreview() ||
                                        !!castOptions()[index()]?.error
                                      }
                                      onClick={() =>
                                        void updateDraft(
                                          create(DuelMoveSchema, {
                                            ...draft(),
                                            action:
                                              draft().action === 1 && draft().handIndex === index()
                                                ? 0
                                                : 1,
                                            handIndex: index(),
                                          }),
                                        )
                                      }
                                    >
                                      {t('cast')}
                                    </button>
                                    <button
                                      type="button"
                                      class="quiet"
                                      aria-pressed={
                                        draft().action === 2 && draft().handIndex === index()
                                      }
                                      disabled={locked() || demo() || pendingPreview()}
                                      onClick={() =>
                                        void updateDraft(
                                          create(DuelMoveSchema, {
                                            ...draft(),
                                            action:
                                              draft().action === 2 && draft().handIndex === index()
                                                ? 0
                                                : 2,
                                            handIndex: index(),
                                          }),
                                        )
                                      }
                                    >
                                      {t('discard')}
                                    </button>
                                    <button
                                      type="button"
                                      class="quiet"
                                      aria-label={`${t('inspect')}: ${cardName(id)}`}
                                      onClick={() => setInspect(card())}
                                    >
                                      ⓘ
                                    </button>
                                  </div>
                                  <Show when={castOptions()[index()]?.error}>
                                    <small class={styles.cardUnavailable}>
                                      {errorText(castOptions()[index()]?.error ?? '')}
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
                      {preview()?.error
                        ? errorText(preview()?.error ?? '')
                        : draft().action
                          ? `${t(draft().action === 1 ? 'cast' : 'discard')}: ${cardName(state().hand[draft().handIndex] ?? 0)}`
                          : t('choose-sequence')}
                    </p>
                    <div class={styles.commandButtons}>
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
                        onClick={() => void animate(events())}
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
                    </div>
                  </div>
                </section>
              }
            >
              <section class={styles.board} aria-labelledby="board-title">
                <TitleBar left={demo() ? 'AI' : t('you')} right={t(profile())}>
                  <h1 id="board-title" class={styles.boardTitle}>
                    {t('duel-title')} ·{' '}
                    {t('round', { round: state().round, total: state().totalRounds })}
                  </h1>
                </TitleBar>
                <div class={styles.fighters} ref={fighters}>
                  <Show when={strike()} keyed>
                    {(event) => (
                      <span
                        class={styles.strike}
                        data-kind={event.attackKind}
                        style={strikePath()}
                        aria-hidden="true"
                      >
                        <img
                          src={icon(event.attackKind === 0 ? 'attack-phys' : 'attack-mag')}
                          alt=""
                        />
                        <b>{event.slot + 1}</b>
                      </span>
                    )}
                  </Show>
                  <Show when={state().me}>
                    {(me) => (
                      <>
                        <Combatant
                          side={animationView()?.me ?? me()}
                          avatar={avatarId()}
                          owner={0}
                          initiative={state().initiative}
                          label={t('you')}
                          played={playedCard(0)}
                          fx={fx()[0]}
                          portraitRef={(element) => {
                            portraits[0] = element;
                          }}
                        />
                        <StatColumn
                          side={animationView()?.me ?? me()}
                          base={lastRoundStart?.me}
                          owner={0}
                          label={t('you')}
                          dealt={dealtTotal(0)}
                        />
                      </>
                    )}
                  </Show>
                  <Show when={state().opponent}>
                    {(opponent) => (
                      <>
                        <StatColumn
                          side={animationView()?.opponent ?? opponent()}
                          base={lastRoundStart?.opponent}
                          owner={1}
                          label={t('opponent')}
                          dealt={dealtTotal(1)}
                        />
                        <Combatant
                          side={animationView()?.opponent ?? opponent()}
                          avatar={state().opponentAvatarId}
                          owner={1}
                          initiative={state().initiative}
                          label={t('opponent')}
                          played={playedCard(1)}
                          fx={fx()[1]}
                          portraitRef={(element) => {
                            portraits[1] = element;
                          }}
                        />
                      </>
                    )}
                  </Show>
                  <div class={styles.exchange} aria-hidden="true">
                    <span class={styles.slotOrb}>{lastSlot(0)}</span>
                    <Show
                      when={strike()}
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
                <div class={styles.arena} ref={arena} aria-live="polite" role="status">
                  <Show
                    when={activeEvent()}
                    fallback={<span class={styles.arenaIdle}>{t('round-over')}</span>}
                  >
                    {(event) => (
                      <>
                        <Show when={strike()} keyed>
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
                        <span class={styles.arenaText}>{eventText(event())}</span>
                      </>
                    )}
                  </Show>
                </div>
                <div class={styles.commandBar}>
                  <p class={styles.validation}>{t('simulation')}</p>
                  <div class={styles.commandButtons}>
                    <button type="button" class="secondary" onClick={skip}>
                      {t('skip')}
                    </button>
                  </div>
                </div>
              </section>
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
        <Modal title={t('chronicle')} wide close={() => setShowChronicle(false)}>
          <Chronicle history={history()} text={eventText} />
          <Show when={events().length}>
            <details class={styles.log}>
              <summary>{t('log')}</summary>
              <ol>
                <For each={events()}>{(event) => <li>{eventText(event)}</li>}</For>
              </ol>
            </details>
          </Show>
          <p class={styles.shortcuts}>{t('shortcuts')}</p>
        </Modal>
      </Show>
      <Show when={inspect()}>
        {(card) => (
          <Modal title={cardName(card().id)} close={() => setInspect(undefined)}>
            <CardDetails card={card()} />
            <button type="button" class="secondary" onClick={() => setInspect(undefined)}>
              {t('close')}
            </button>
          </Modal>
        )}
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
