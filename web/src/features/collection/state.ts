import { accountEpoch } from '../../app/account';
import { avatars, cards, content, playableAvatars } from '../../app/content';

export type AvatarState = {
  id: string;
  slot: number;
  template_id: number;
  path_id?: number | null;
  specialization?: string | null;
  gift_id?: number | null;
  is_shade?: boolean;
  shade_school?: string | null;
  level: number;
  xp: number;
  tier: number;
  active: boolean;
  revision: number;
  unspent: number;
  spent: number[];
  stats: number[];
  schools: number[];
  deck: number[];
  deck_revision: number;
};
export type Snapshot = {
  content_version: string;
  recruitment?: RecruitmentState;
  templates: number[];
  avatars: AvatarState[];
  collection: { card_id: number; qty: number }[];
  presets: { id: string; name: string; cards: number[]; revision: number }[];
};
export type RecruitmentState = {
  level: number;
  available: number[];
  milestones: {
    level: number;
    claimed: boolean;
    template_id: number | null;
    avatar_id: string | null;
  }[];
};
export type Mutation =
  | { action: 'recruit'; level: number; template_id: number | null }
  | {
      action: 'reincarnate';
      id: string;
      revision: number;
      choice:
        | { kind: 'path'; id: number }
        | { kind: 'specialization'; name: string }
        | { kind: 'gift'; card_id: number };
      sacrifice?: string;
    }
  | { action: 'shade'; id: string; revision: number; new_id: string; school: string }
  | { action: 'create'; id: string; template_id: number }
  | { action: 'delete'; id: string; revision: number }
  | { action: 'activate'; id: string }
  | { action: 'spend'; id: string; revision: number; points: number[] }
  | { action: 'deck'; id: string; revision: number; cards: number[] }
  | { action: 'preset_save'; id: string; revision: number | null; name: string; cards: number[] }
  | { action: 'preset_delete'; id: string; revision: number };

let guest: Snapshot | undefined;
let epoch = -1;
function freshAvatar(id: string, template: number, slot: number): AvatarState {
  const definition = avatars.get(template);
  if (!definition) throw new Error('NOT_FOUND');
  return {
    id,
    slot,
    template_id: template,
    level: 1,
    xp: 0,
    tier: 0,
    active: slot === 0,
    revision: 0,
    unspent: 0,
    spent: [0, 0, 0, 0, 0, 0],
    stats: [
      definition.base.phys_atk,
      definition.base.phys_def,
      definition.base.mag_atk,
      definition.base.mag_def,
      definition.base.hp,
      definition.base.stamina,
    ],
    schools: ['combat', 'divine', 'element', 'arcane', 'necro', 'tech'].map(
      (s) => (definition.schools as Record<string, number | undefined>)[s] ?? 0,
    ),
    deck: Object.entries(
      content.starters.find((s) => s.avatar === definition.key)?.deck ?? {},
    ).flatMap(([key, count]) => {
      const card = content.cards.find((c) => c.key === key);
      return card ? Array<number>(count).fill(card.id) : [];
    }),
    deck_revision: 0,
  };
}
export function guestState(): Snapshot {
  if (!guest || epoch !== accountEpoch()) {
    epoch = accountEpoch();
    guest = {
      content_version: content.version,
      templates: playableAvatars.map((a) => a.id),
      avatars: [freshAvatar(crypto.randomUUID(), content.avatars[0]?.id ?? 1, 0)],
      collection: content.cards.map((c) => ({ card_id: c.id, qty: 4 })),
      presets: [],
    };
  }
  return structuredClone(guest);
}
export function guestMutation(mutation: Mutation): Snapshot {
  const next = guestState();
  const find = (id: string) => {
    const avatar = next.avatars.find((a) => a.id === id);
    if (!avatar) throw new Error('NOT_FOUND');
    return avatar;
  };
  const version = (actual: number, expected: number | null) => {
    if (actual !== expected) throw new Error('CONFLICT');
  };
  const deck = (ids: number[]) => {
    if (
      ids.length > content.rules.deck.size ||
      ids.some(
        (id) =>
          !cards.has(id) ||
          ids.filter((v) => v === id).length >
            Math.min(
              content.rules.deck.max_copies,
              next.collection.find((s) => s.card_id === id)?.qty ?? 0,
            ),
      )
    )
      throw new Error('INVALID_DECK');
    return [...ids];
  };
  switch (mutation.action) {
    case 'recruit':
      throw new Error('NOT_READY');
    case 'reincarnate':
    case 'shade':
      // Training grants no XP; the local guest collection never reaches the threshold.
      throw new Error('NOT_READY');
    case 'create': {
      if (!next.templates.includes(mutation.template_id)) throw new Error('NOT_FOUND');
      const existing = next.avatars.find((a) => a.id === mutation.id);
      if (existing) {
        if (existing.template_id !== mutation.template_id) throw new Error('CONFLICT');
        break;
      }
      const slot = Array.from({ length: 10 }, (_, i) => i).find((i) =>
        next.avatars.every((a) => a.slot !== i),
      );
      if (slot === undefined) throw new Error('SLOTS_FULL');
      const created = freshAvatar(mutation.id, mutation.template_id, slot);
      created.active = next.avatars.length === 0;
      next.avatars.push(created);
      break;
    }
    case 'activate':
      find(mutation.id);
      for (const avatar of next.avatars) {
        const active = avatar.id === mutation.id;
        if (active !== avatar.active) avatar.revision++;
        avatar.active = active;
      }
      break;
    case 'delete': {
      const avatar = find(mutation.id);
      version(avatar.revision, mutation.revision);
      if (avatar.active) throw new Error('ACTIVE_AVATAR');
      next.avatars = next.avatars.filter((a) => a.id !== mutation.id);
      break;
    }
    case 'spend': {
      const avatar = find(mutation.id);
      version(avatar.revision, mutation.revision);
      const sum = mutation.points.reduce((a, b) => a + b, 0);
      if (
        mutation.points.length !== 6 ||
        mutation.points.some((v) => !Number.isInteger(v) || v < 0) ||
        sum <= 0 ||
        sum > avatar.unspent
      )
        throw new Error('INVALID_POINTS');
      avatar.unspent -= sum;
      for (let i = 0; i < 6; i++) {
        avatar.spent[i] = (avatar.spent[i] ?? 0) + (mutation.points[i] ?? 0);
        avatar.stats[i] = (avatar.stats[i] ?? 0) + (mutation.points[i] ?? 0) * (i >= 4 ? 7 : 1);
      }
      avatar.revision++;
      break;
    }
    case 'deck': {
      const avatar = find(mutation.id);
      version(avatar.deck_revision, mutation.revision);
      avatar.deck = deck(mutation.cards);
      avatar.deck_revision++;
      break;
    }
    case 'preset_save': {
      const existing = next.presets.find((p) => p.id === mutation.id);
      const name = mutation.name.trim();
      if (
        !name ||
        name.length > 40 ||
        next.presets.some((p) => p.id !== mutation.id && p.name === name)
      )
        throw new Error('CONFLICT');
      if (existing) {
        version(existing.revision, mutation.revision);
        existing.name = name;
        existing.cards = deck(mutation.cards);
        existing.revision++;
      } else {
        if (next.presets.length >= content.rules.deck.saved_presets_max)
          throw new Error('PRESETS_FULL');
        next.presets.push({ id: mutation.id, name, cards: deck(mutation.cards), revision: 0 });
      }
      break;
    }
    case 'preset_delete': {
      const preset = next.presets.find((p) => p.id === mutation.id);
      if (!preset) throw new Error('NOT_FOUND');
      version(preset.revision, mutation.revision);
      next.presets = next.presets.filter((p) => p.id !== mutation.id);
      break;
    }
  }
  guest = next;
  return structuredClone(next);
}

import '../../app/cardCatalog';
