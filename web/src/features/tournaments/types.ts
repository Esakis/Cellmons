export type Terms = {
  teams: number;
  team_size: number;
  fee: number;
  max_games: number;
  min_level: number;
  max_level: number;
  tiers: number[];
  only_clan: string | null;
  allowed_clans: string[];
};
export type Mutation =
  | { action: 'create'; id: string; name: string; terms: Terms }
  | { action: 'join'; id: string; request: string; avatar: string }
  | { action: 'leave' | 'cancel'; id: string; request: string };
export type Summary = {
  id: string;
  name: string;
  status: string;
  creator: string;
  enrolled: number;
  revision: number;
  balance: string;
};
export type Entry = {
  user: string;
  avatar: string;
  team: string;
  login: string;
  template_id: number;
  level: number;
  tier: number;
  status: string;
  departed: boolean;
  ready: boolean;
};
export type Pair = {
  teams: [string | null, string | null];
  lanes: { users: [string, string]; points: [number, number]; fought: number; active: boolean }[];
  resolved: boolean;
  winner: string | null;
};
export type Profile = {
  summary: Summary;
  terms: Terms;
  entries: Entry[];
  rounds: { pairs: Pair[] }[];
  champion: string | null;
  reward: Reward | null;
};
export type Reward =
  | { kind: 'ducats'; amount: number }
  | { kind: 'avatar'; template: number }
  | { kind: 'card'; card: number }
  | { kind: 'booster'; booster: string };
export type Snapshot = {
  content_version: string;
  own: string | null;
  own_clan: string | null;
  catalog: Summary[];
  profile: Profile | null;
  has_more: boolean;
  clans: { id: string; name: string }[];
  first_fight_s: number;
  next_fight_s: number;
  ready_grace_s: number;
};
