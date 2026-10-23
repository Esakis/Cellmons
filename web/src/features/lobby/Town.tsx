import { A, useNavigate } from '@solidjs/router';
import { createSignal, For, Show } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import { icon } from '../../app/content';
import { onlineIcon } from '../../app/onlineIcons';
import layout from '../../generated/town.json';
import { t } from '../../i18n';
import { canEnter, type Journey, requiredLevel } from './journey';

// The complete town and its building hit areas use the same coordinates on every screen.
// Resizing fits the painting to the available space; it never rearranges destinations.
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
  ranked: () => icon('school-combat'),
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
  journey: Journey | undefined;
  /** The player's clan, or '' without one: the clan hall then leads to the clan list. */
  clan: string;
  /** A place where something waits for the player (a challenge), or ''. */
  alert: string;
  enter: (room: string) => void;
}) {
  const navigate = useNavigate();
  const [hot, setHot] = createSignal('');
  const locked = (id: string) => !canEnter(props.journey, id);
  const waiting = (id: string) =>
    props.alert === id || (id === 'library' && (props.journey?.recruits ?? 0) > 0);
  const link = (id: string) =>
    locked(id)
      ? ''
      : id === 'library'
        ? '/collection'
        : id === 'clan' && !props.clan
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
    <section class="town" aria-labelledby="town-title">
      <h2 id="town-title" class="visually-hidden">
        {t('lobby-tab-map')}
      </h2>
      <div class="town-view">
        <div class="town-canvas" style={{ '--town-w': width, '--town-h': height }}>
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
          <For each={layout.places}>
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
          <For each={layout.places}>
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
