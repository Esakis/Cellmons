// Card illustrations in one lazily loaded module: their shared SVG markup compresses
// several times better together than as separate files (offline budget).
const sources = import.meta.glob<string>('../assets/cards/*.svg', {
  eager: true,
  query: '?raw',
  import: 'default',
});
export const cardPictures = new Map(
  Object.entries(sources).map(([path, svg]) => [
    `card.${path.slice(path.lastIndexOf('/') + 1, -'.svg'.length)}`,
    `data:image/svg+xml,${encodeURIComponent(svg)}`,
  ]),
);
