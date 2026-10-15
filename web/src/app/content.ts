import raw from '../generated/content.json';

export type Card = (typeof raw.cards)[number];
export const content = raw;
export const cards = new Map(content.cards.map((card) => [card.id, card]));
export const avatars = new Map(content.avatars.map((avatar) => [avatar.id, avatar]));
// Portraits are inlined together: shared SVG markup compresses far better than six files.
const pictures = import.meta.glob<string>('../assets/avatars/*.svg', {
  eager: true,
  query: '?raw',
  import: 'default',
});
const symbols = import.meta.glob<string>('../assets/icons/*.svg', {
  eager: true,
  query: '?url',
  import: 'default',
});
export function portrait(key: string) {
  const svg = pictures[`../assets/avatars/${key.replace('avatar.', '')}.svg`];
  return svg && `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
export const icon = (key: string) => symbols[`../assets/icons/${key}.svg`];
export const fluentKey = (key: string, field = 'name') => `${key.replace('.', '-')}-${field}`;

export function cardArgs(card: Card): Record<string, number> {
  const args: Record<string, number> = {};
  for (const [key, value] of Object.entries(card.params)) {
    if (typeof value === 'number') args[key] = value;
  }
  return args;
}
