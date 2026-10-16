import overlays from '../generated/path-overlays.json';
// Reuse the base portrait; variant overlays are the same SVG geometry as the review files.
export function decoratePath(svg: string, key: string) {
  return svg.replace('</svg>', `${(overlays as Record<string, string>)[key] ?? ''}</svg>`);
}
