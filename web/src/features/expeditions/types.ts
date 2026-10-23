export type Award = { xp: number; ducats: number; template: number | null; card: number | null };
export type Run = {
  id: string;
  avatar: string;
  difficulty: string;
  status: string;
  stage: number;
  stages: number;
  xp_accum: number;
  retreat_xp: number;
  active_duel: string | null;
  revision: number;
  choices: { choice: number; template_id: number; level: number; color: string; boss: boolean }[];
  award: Award | null;
};
export type Snapshot = {
  content_version: string;
  account_level?: number;
  run: Run | null;
  difficulties: {
    id: string;
    stages: number;
    fixed_ducats: number;
    random_ducats: number;
    rarity: string;
  }[];
  avatar_chance_bps: number;
  card_chance_bps: number;
  ducats_chance_bps: number;
};
export type Mutation =
  | { action: 'start'; id: string; avatar: string; difficulty: string }
  | { action: 'choose'; id: string; request: string; revision: number; choice: number }
  | { action: 'retreat'; id: string; request: string; revision: number };
