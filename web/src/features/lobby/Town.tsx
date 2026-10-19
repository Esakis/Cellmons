import { A, useNavigate } from '@solidjs/router';
import { createSignal, For, type JSX, onMount, Show } from 'solid-js';
import { icon } from '../../app/content';
import { onlineIcon } from '../../app/onlineIcons';
import layout from '../../generated/town.json';
import { t } from '../../i18n';

// The lobby map is one painting of the town (art/paintings/scenes/town.avif). Its clickable
// buildings are already lit in the picture (art/paintings/town.json holds their outlines), so a
// gilt rim and a name plaque mark them at rest; hovering or focusing a building brightens it.
const [painting] = Object.values(
  import.meta.glob<string>('../../assets/paintings/scenes/town.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);
const [width, height] = layout.size as [number, number];
const icons: Record<string, () => string | undefined> = {
  tower: () => onlineIcon('tab-social'),
  library: () => icon('nav-collection'),
  castle: () => icon('nav-lobby'),
  portal: () => icon('attack-mag'),
  arena: () => icon('school-combat'),
  clan: () => icon('stat-phys-def'),
  training: () => icon('nav-training'),
  bazaar: () => onlineIcon('ducats'),
  shop: () => icon('nav-store'),
  expedition: () => onlineIcon('tab-map'),
};
/** The icon of a place (a lobby room or the library); clan rooms share one. */
export const placeIcon = (place: string) => icons[place]?.();
const points = (shape: number[][]) =>
  shape.map(([x = 0, y = 0]) => `${(x * width) / 100},${(y * height) / 100}`).join(' ');
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
  /** The player's clan, or '' without one: the clan hall then leads to the clan list. */
  clan: string;
  /** A place where something waits for the player (a challenge), or ''. */
  alert: string;
  enter: (room: string) => void;
}) {
  const navigate = useNavigate();
  const [hot, setHot] = createSignal('');
  let view: HTMLDivElement | undefined;
  // Phones pan the town sideways: start in the middle, at the castle, the portal and the arena.
  onMount(() => {
    if (view) view.scrollLeft = (view.scrollWidth - view.clientWidth) / 2;
  });
  const link = (id: string) =>
    id === 'library' ? '/collection' : id === 'clan' && !props.clan ? '/clans' : '';
  function go(id: string) {
    if (link(id)) navigate(link(id));
    else if (props.connected) props.enter(id === 'clan' ? `clan:${props.clan}` : id);
  }
  const hint = (id: string) =>
    t(id === 'clan' && !props.clan ? 'lobby-room-clan-join' : `lobby-room-${id}-hint`);
  return (
    <section class="town" aria-labelledby="town-title">
      <h2 id="town-title" class="visually-hidden">
        {t('lobby-tab-map')}
      </h2>
      <div class="town-view" ref={view}>
        <div class="town-canvas">
          <img src={painting} alt="" width={width} height={height} />
          <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" aria-hidden="true">
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
            <For each={layout.places}>
              {(p) => (
                <polygon
                  class="town-edge"
                  classList={{ hot: hot() === p.id, alert: props.alert === p.id }}
                  points={points(p.shape)}
                  filter="url(#town-glow)"
                />
              )}
            </For>
          </svg>
          {/* The building itself, for the pointer (it brightens what is under it); keyboards and
              screen readers use its plaque. */}
          <For each={layout.places}>
            {(p) => (
              <button
                type="button"
                class="town-shape"
                classList={{ hot: hot() === p.id }}
                tabIndex={-1}
                aria-hidden="true"
                style={outline(p.shape)}
                disabled={!link(p.id) && !props.connected}
                onClick={() => go(p.id)}
                onPointerEnter={() => setHot(p.id)}
                onPointerLeave={() => setHot('')}
              />
            )}
          </For>
          <For each={layout.places}>
            {(p) => {
              const content = (): JSX.Element => (
                <>
                  <span class="medal">
                    <img src={placeIcon(p.id)} alt="" width="20" height="20" />
                  </span>
                  <span>
                    {t(`lobby-room-${p.id}`)}
                    <small class="town-hint" id={`town-hint-${p.id}`} aria-hidden="true">
                      {hint(p.id)}
                    </small>
                  </span>
                  <Show when={props.alert === p.id}>
                    <b class="town-badge" aria-hidden="true">
                      !
                    </b>
                  </Show>
                </>
              );
              const events = {
                onPointerEnter: () => setHot(p.id),
                onPointerLeave: () => setHot(''),
                onFocus: () => setHot(p.id),
                onBlur: () => setHot(''),
              };
              return (
                <Show
                  when={link(p.id)}
                  fallback={
                    <button
                      type="button"
                      class="town-place plaque"
                      classList={{ hot: hot() === p.id }}
                      style={{ left: `${p.at[0]}%`, top: `${p.at[1]}%` }}
                      aria-describedby={`town-hint-${p.id}`}
                      disabled={!props.connected}
                      onClick={() => go(p.id)}
                      {...events}
                    >
                      {content()}
                    </button>
                  }
                >
                  {(href) => (
                    <A
                      class="town-place plaque"
                      classList={{ hot: hot() === p.id }}
                      style={{ left: `${p.at[0]}%`, top: `${p.at[1]}%` }}
                      aria-describedby={`town-hint-${p.id}`}
                      href={href()}
                      {...events}
                    >
                      {content()}
                    </A>
                  )}
                </Show>
              );
            }}
          </For>
        </div>
      </div>
    </section>
  );
}
