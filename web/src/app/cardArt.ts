// Card illustrations in one lazily loaded module: their shared SVG markup compresses
// several times better together than as separate files (offline budget).
const sources = import.meta.glob<string>('../assets/cards/*.svg', {
  eager: true,
  query: '?raw',
  import: 'default',
});
// Painted illustrations (art/paintings) are separate AVIF files; they replace the SVG drawings.
const paintings = import.meta.glob<string>('../assets/paintings/cards/*.avif', {
  eager: true,
  query: '?url',
  import: 'default',
});
const cardKey = (path: string) =>
  `card.${path.slice(path.lastIndexOf('/') + 1, path.lastIndexOf('.'))}`;
export const cardPictures = new Map([
  ...Object.entries(sources).map(([path, svg]): [string, string] => [
    cardKey(path),
    `data:image/svg+xml,${encodeURIComponent(svg)}`,
  ]),
  ...Object.entries(paintings).map(([path, url]): [string, string] => [cardKey(path), url]),
]);
