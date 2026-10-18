import { create } from '@bufbuild/protobuf';
import { expect, it } from 'vitest';
import { DuelEventSchema, TrainingViewSchema } from '../../generated/proto/game/v1/training_pb';
import { advanceAnimation } from './animation';

it('preserves overkill damage before subsequent healing for attacks and spells', () => {
  const start = create(TrainingViewSchema, {
    me: { hp: 10, stamina: 5 },
    opponent: { hp: 10, stamina: 5 },
  });
  for (const kind of ['hit', 'damage']) {
    for (const side of [0, 1]) {
      const target = kind === 'hit' ? 1 - side : side;
      const damaged = advanceAnimation(
        start,
        create(DuelEventSchema, {
          kind,
          side,
          amount: 30,
          newValue: 5,
        }),
      );
      const healed = advanceAnimation(
        damaged,
        create(DuelEventSchema, {
          kind: 'heal',
          side: target,
          amount: 5,
        }),
      );
      const actor = target === 0 ? healed.me : healed.opponent;
      expect(actor?.hp).toBe(-15);
      expect(start.me?.hp).toBe(10);
      expect(start.opponent?.hp).toBe(10);
    }
  }
});
