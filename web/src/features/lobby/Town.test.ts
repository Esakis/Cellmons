import { expect, it } from 'vitest';
import { pickVariant } from './Town';

const square = { name: 'town', width: 1024, height: 1024 };
const portrait = { name: 'town-portrait', width: 864, height: 1536 };
const wide = { name: 'town-wide', width: 1536, height: 864 };

it('picks the town painting closest to the shape of the available space', () => {
  const all = [square, portrait, wide];
  // A phone in portrait leaves a tall box under the header.
  expect(pickVariant(all, { width: 380, height: 640 })).toBe(portrait);
  // A desktop browser leaves a wide box.
  expect(pickVariant(all, { width: 1400, height: 760 })).toBe(wide);
  expect(pickVariant(all, { width: 700, height: 680 })).toBe(square);
  // Without variants the square town stays.
  expect(pickVariant([square], { width: 380, height: 640 })).toBe(square);
  // Not measured yet.
  expect(pickVariant(all, { width: 0, height: 0 })).toBeUndefined();
});
