export type Member = { user: string; login: string; role: string; title: string; rating: number };
export type Summary = {
  id: string;
  name: string;
  status: string;
  revision: number;
  members: number;
  score: number;
  rank: number | null;
};
export type Profile = Summary & {
  description: string;
  motd: string;
  www: string;
  email: string;
  founded_ms: number | null;
  ducats: string;
  roster: Member[];
  applicants: { user: string; login: string; at_ms: number }[];
};
export type Snapshot = {
  content_version: string;
  account_level?: number;
  own: string | null;
  own_role: string | null;
  clans: Summary[];
  has_more: boolean;
  profile: Profile | null;
  applications: { clan: string; name: string }[];
  limits: { min_members: number; max_members: number; max_applications: number };
};
type Versioned = { clan: string; revision: number };
export type Mutation =
  | { action: 'create'; id: string; name: string }
  | { action: 'apply' | 'withdraw'; clan: string }
  | (Versioned & { action: 'accept' | 'reject' | 'kick' | 'transfer'; user: string })
  | (Versioned & { action: 'role'; user: string; role: 'vice' | 'scribe' | 'member' })
  | (Versioned & { action: 'title'; user: string; title: string })
  | (Versioned & { action: 'edit'; description: string; motd: string; www: string; email: string })
  | (Versioned & { action: 'leave' | 'disband' });
