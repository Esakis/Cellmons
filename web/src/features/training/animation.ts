import type { DuelEvent, TrainingView } from '../../generated/proto/game/v1/training_pb';

// Replay public events without changing the authoritative round snapshot.
export function advanceAnimation(current: TrainingView, event: DuelEvent): TrainingView {
  if (!current.me || !current.opponent) return current;
  const next = structuredClone(current);
  const sides = [next.me, next.opponent];
  const side = sides[event.side];
  if (!side) return current;
  switch (event.kind) {
    case 'reveal':
      if (next.me) next.me.focus -= event.oldValue;
      if (next.opponent) next.opponent.focus -= event.newValue;
      for (const [index, move] of event.moves.entries()) {
        const actor = sides[index];
        if (actor) {
          actor.attacks = move.attacks;
          actor.defenses = move.defenses;
        }
      }
      break;
    case 'cast':
    case 'countered':
      side.focus -= event.oldValue;
      side.stamina -= event.newValue;
      break;
    case 'hit': {
      const defender = sides[1 - event.side];
      // R-ATK-07: negative HP must survive until healing later in the same round.
      if (defender) defender.hp = Math.max(-2147483648, defender.hp - event.amount);
      side.stamina = Math.max(0, side.stamina - event.newValue);
      break;
    }
    case 'damage':
      side.hp = Math.max(-2147483648, side.hp - event.amount);
      break;
    case 'heal':
      side.hp += event.amount;
      break;
    case 'resource':
      if (event.detail === 'Stamina') side.stamina = event.newValue;
      if (event.detail === 'Focus') side.focus = event.newValue;
      break;
    case 'stat': {
      const index = ['PhysAtk', 'PhysDef', 'MagAtk', 'MagDef', 'HpMax', 'StaminaMax'].indexOf(
        event.detail,
      );
      if (index >= 0) side.stats[index] = event.newValue;
      break;
    }
  }
  return next;
}
