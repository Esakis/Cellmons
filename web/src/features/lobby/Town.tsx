import { A, useNavigate } from '@solidjs/router';
import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import { t } from '../../i18n';
import { canEnter, type Journey, requiredLevel } from './journey';

// The town comes in several shapes: town.json/town.avif (square) and optional variants such as
// town-portrait (phones) and town-wide (wide screens), each with its own building coordinates.
// The view picks the variant whose proportions best match the space it has, then fits it whole.
type Layout = {
  size: number[];
  places: { id: string; at: number[]; shape: number[][] }[];
};
const layouts = import.meta.glob<Layout>('../../generated/town*.json', {
  eager: true,
  import: 'default',
});
const paintings = import.meta.glob<string>('../../assets/paintings/scenes/town*.avif', {
  eager: true,
  query: '?url',
  import: 'default',
});
const variants = Object.entries(layouts)
  .map(([path, layout]) => {
    const name = path.slice(path.lastIndexOf('/') + 1, -'.json'.length);
    const src = Object.entries(paintings).find(([p]) => p.endsWith(`/${name}.avif`))?.[1];
    const [width = 1, height = 1] = layout.size;
    return src ? { name, src, width, height, places: layout.places } : undefined;
  })
  .filter((v): v is NonNullable<typeof v> => v !== undefined);
type Variant = (typeof variants)[number];
const square = variants.find((v) => v.name === 'town') ?? (variants[0] as Variant);
/** The variant closest in shape to the available box (compared on a log scale). */
export function pickVariant<T extends { width: number; height: number }>(
  options: T[],
  box: { width: number; height: number },
): T | undefined {
  if (!(box.width > 0 && box.height > 0)) return undefined;
  const target = Math.log(box.width / box.height);
  let best: T | undefined;
  for (const option of options) {
    const distance = Math.abs(Math.log(option.width / option.height) - target);
    if (!best || distance < Math.abs(Math.log(best.width / best.height) - target)) best = option;
  }
  return best;
}
// A building's box on the canvas and its outline inside the box, both in per cent.
function outline(shape: number[][]) {
  const xs = shape.map(([x = 0]) => x);
  const ys = shape.map(([, y = 0]) => y);
  const [left, top] = [Math.min(...xs), Math.min(...ys)];
  const [w, h] = [Math.max(...xs) - left, Math.max(...ys) - top];
  const corners = shape.map(
    ([x = 0, y = 0]) =>
      `${(((x - left) / w) * 100).toFixed(2)}% ${(((y - top) / h) * 100).toFixed(2)}%`,
  );
  return {
    left: `${left}%`,
    top: `${top}%`,
    width: `${w}%`,
    height: `${h}%`,
    'clip-path': `polygon(${corners.join(', ')})`,
  };
}

export function Town(props: {
  connected: boolean;
  journey: Journey | undefined;
  /** The player's clan, or '' without one: the clan hall then leads to the clan list. */
  clan: string;
  /** A place where something waits for the player (a challenge), or ''. */
  alert: string;
  enter: (room: string) => void;
}) {
  const navigate = useNavigate();
  const [hot, setHot] = createSignal('');
  const [box, setBox] = createSignal({ width: 0, height: 0 });
  const town = createMemo(() => pickVariant(variants, box()) ?? square);
  const points = (shape: number[][]) =>
    shape
      .map(([x = 0, y = 0]) => `${(x * town().width) / 100},${(y * town().height) / 100}`)
      .join(' ');
  let section: HTMLElement | undefined;
  onMount(() => {
    if (!section || variants.length < 2 || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setBox({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(section);
    onCleanup(() => observer.disconnect());
  });
  const locked = (id: string) => !canEnter(props.journey, id);
  const waiting = (id: string) =>
    props.alert === id || (id === 'library' && (props.journey?.recruits ?? 0) > 0);
  // The library, the shop and the tower are pages with their own painting and way back;
  // the other buildings are lobby rooms.
  const link = (id: string) =>
    locked(id)
      ? ''
      : id === 'library'
        ? '/collection'
        : id === 'shop'
          ? '/store'
          : id === 'tower' || (id === 'clan' && !props.clan)
            ? '/clans'
            : '';
  function go(id: string) {
    if (locked(id)) return;
    if (link(id)) navigate(link(id));
    else if (props.connected) props.enter(id === 'clan' ? `clan:${props.clan}` : id);
  }
  const hint = (id: string) =>
    t(
      id === 'library' && waiting(id)
        ? 'lobby-recruit-ready'
        : id === 'clan' && !props.clan
          ? 'lobby-room-clan-join'
          : `lobby-room-${id}-hint`,
    );
  return (
    <section class="town" aria-labelledby="town-title" ref={section}>
      <h2 id="town-title" class="visually-hidden">
        {t('lobby-tab-map')}
      </h2>
      <div
        class="town-view"
        data-variant={town().name}
        style={{ '--town-w': town().width, '--town-h': town().height }}
      >
        <div class="town-canvas">
          <img src={town().src} alt="" width={town().width} height={town().height} />
          <svg
            viewBox={`0 0 ${town().width} ${town().height}`}
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <defs>
              <filter id="town-glow" x="-10%" y="-10%" width="120%" height="120%">
                <feGaussianBlur stdDeviation="5" result="blur" />
                <feMerge>
                  <feMergeNode in="blur" />
                  <feMergeNode in="blur" />
                  <feMergeNode in="SourceGraphic" />
                </feMerge>
              </filter>
            </defs>
            <For each={town().places}>
              {(p) => (
                <polygon
                  class="town-edge"
                  classList={{
                    hot: hot() === p.id,
                    alert: waiting(p.id),
                    locked: locked(p.id),
                  }}
                  points={points(p.shape)}
                  filter="url(#town-glow)"
                />
              )}
            </For>
          </svg>
          {/* The building itself, for the pointer (it brightens what is under it); keyboards and
              screen readers use its plaque. */}
          <For each={town().places}>
            {(p) => (
              <button
                type="button"
                class="town-shape"
                classList={{ hot: hot() === p.id, locked: locked(p.id) }}
                tabIndex={-1}
                aria-hidden="true"
                style={outline(p.shape)}
                disabled={locked(p.id) || (!link(p.id) && !props.connected)}
                onClick={() => go(p.id)}
                onPointerEnter={() => setHot(p.id)}
                onPointerLeave={() => setHot('')}
              />
            )}
          </For>
          <For each={town().places}>
            {(p) => (
              // The library and the clan list are pages; the other buildings are lobby rooms.
              <Dynamic
                component={link(p.id) ? A : 'button'}
                type={link(p.id) ? undefined : 'button'}
                href={link(p.id) || undefined}
                disabled={locked(p.id) || (!link(p.id) && !props.connected)}
                class="town-place"
                data-place={p.id}
                classList={{ hot: hot() === p.id, locked: locked(p.id) }}
                style={{ '--place-x': `${p.at[0]}%`, '--place-y': `${p.at[1]}%` }}
                aria-describedby={`town-hint-${p.id}`}
                title={hint(p.id)}
                onClick={() => link(p.id) || go(p.id)}
                onPointerEnter={() => setHot(p.id)}
                onPointerLeave={() => setHot('')}
                onFocus={() => setHot(p.id)}
                onBlur={() => setHot('')}
              >
                <span class="town-label">
                  {t(`lobby-room-${p.id}`)}
                  <Show when={locked(p.id)}>
                    <small class="town-lock">
                      {t('lobby-location-locked-short', {
                        level: requiredLevel(props.journey, p.id),
                      })}
                    </small>
                  </Show>
                  <small class="visually-hidden" id={`town-hint-${p.id}`} aria-hidden="true">
                    {hint(p.id)}
                  </small>
                </span>
                <Show when={waiting(p.id)}>
                  <b class="town-badge" aria-hidden="true">
                    !
                  </b>
                </Show>
              </Dynamic>
            )}
          </For>
        </div>
      </div>
    </section>
  );
}
