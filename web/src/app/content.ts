import { createSignal } from 'solid-js';
import type cardData from '../generated/cards.json';
import raw from '../generated/content.json';
import type pathData from '../generated/paths.json';

export type Card = (typeof cardData.cards)[number];
export type Path = (typeof pathData.paths)[number];
export type Avatar = (typeof raw.avatars)[number] | Path;
let paths: Path[] = [];
let cardCatalog: Card[] = [];
export const content = {
  ...raw,
  get cards() {
    return cardCatalog;
  },
  get paths() {
    return paths;
  },
};
export const cards = new Map<number, Card>();
export function registerCards(data: typeof cardData) {
  if (data.version !== content.version) throw new Error('CONTENT_MISMATCH');
  cardCatalog = data.cards;
  for (const card of cardCatalog) cards.set(card.id, card);
}
export const avatars = new Map<number, Avatar>(
  content.avatars.map((avatar) => [avatar.id, avatar]),
);
export function registerPaths(data: typeof pathData) {
  if (data.version !== content.version) throw new Error('CONTENT_MISMATCH');
  paths = data.paths;
  for (const path of paths) avatars.set(path.id, path);
}
let pathCatalog: Promise<unknown> | undefined;
export function loadPaths() {
  if (!pathCatalog) {
    pathCatalog = import('./pathCatalog').catch((error) => {
      pathCatalog = undefined;
      throw error;
    });
  }
  return pathCatalog;
}
// Portraits are inlined together: shared SVG markup compresses far better than six files.
const pictures = import.meta.glob<string>(
  ['../assets/avatars/*.svg', '!../assets/avatars/*_path_*.svg'],
  {
    eager: true,
    query: '?raw',
    import: 'default',
  },
);
const [pathDecorator, setPathDecorator] = createSignal<(svg: string, key: string) => string>();
let loadingPaths: Promise<unknown> | undefined;
const symbols = import.meta.glob<string>('../assets/icons/*.svg', {
  eager: true,
  query: '?raw',
  import: 'default',
});
// Painted portraits (art/paintings) are separate AVIF files; path variants share the base painting.
const paintings = import.meta.glob<string>('../assets/paintings/avatars/*.avif', {
  eager: true,
  query: '?url',
  import: 'default',
});
export function portrait(key: string) {
  const base = key.replace('avatar.', '').replace(/_path_[01]$/, '');
  const painting = paintings[`../assets/paintings/avatars/${base}.avif`];
  if (painting) return painting;
  const decorate = pathDecorator();
  const isPath = /_path_[01]$/.test(key);
  if (isPath && !decorate && !loadingPaths) {
    loadingPaths = import('./pathArt')
      .then((m) => setPathDecorator(() => m.decoratePath))
      .catch(() => {
        loadingPaths = undefined;
      });
  }
  let svg =
    pictures[`../assets/avatars/${key.replace('avatar.', '')}.svg`] ??
    pictures[`../assets/avatars/${key.replace('avatar.', '').replace(/_path_[01]$/, '')}.svg`];
  if (isPath && svg && decorate) svg = decorate(svg, key);
  return svg && `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
export const icon = (key: string) => {
  const svg = symbols[`../assets/icons/${key}.svg`];
  return svg && `data:image/svg+xml,${encodeURIComponent(svg)}`;
};
// Character stats in protocol order (AF, OF, AM, OM, max HP, max stamina): icon and name.
export const statIcons = [
  ['stat-phys-atk', 'stat-PhysAtk'],
  ['stat-phys-def', 'stat-PhysDef'],
  ['stat-mag-atk', 'stat-MagAtk'],
  ['stat-mag-def', 'stat-MagDef'],
  ['stat-hp', 'stat-HpMax'],
  ['stat-stamina', 'stat-StaminaMax'],
] as const;
export const fluentKey = (key: string, field = 'name') => `${key.replace('.', '-')}-${field}`;

export function cardArgs(card: Card): Record<string, number> {
  const args: Record<string, number> = {};
  for (const [key, value] of Object.entries(card.params)) {
    if (typeof value === 'number') args[key] = value;
  }
  return args;
}
