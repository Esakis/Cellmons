// Icons of the online screens (art/icons/online) stay out of the offline package, unlike content.ts icons.
const symbols = import.meta.glob<string>('../assets/icons/online/*.svg', {
  eager: true,
  query: '?raw',
  import: 'default',
});
export const onlineIcon = (key: string) => {
  const svg = symbols[`../assets/icons/online/${key}.svg`];
  return svg && `data:image/svg+xml,${encodeURIComponent(svg)}`;
};
