import type { Terms } from '../tournaments/types';
export type Gate = {
  role: string;
  enrolled: boolean;
  authorized: boolean;
  expires_ms: number | null;
};
export type Setup = { uri: string; secret: string; expires_ms: number };
export type Row = Record<string, unknown> & {
  id?: string;
  login?: string;
  name?: string;
  status?: string;
  role?: string;
  phrase?: string;
  enabled?: boolean;
  body?: string;
  description?: string;
  motd?: string;
  reviewed?: boolean;
};
export type Page = { rows: Row[]; has_more: boolean };
export type Reward =
  | { kind: 'ducats'; amount: number }
  | { kind: 'avatar'; template: number }
  | { kind: 'card'; card: number }
  | { kind: 'booster'; booster: string };
export type Operation =
  | { action: 'warn' | 'unban' | 'unmute'; user: string }
  | { action: 'ban'; user: string; seconds: number | null }
  | { action: 'mute'; user: string; seconds: number }
  | { action: 'role'; user: string; role: string }
  | { action: 'clan_edit'; clan: string; description: string; motd: string }
  | { action: 'filter'; id: string; phrase: string; enabled: boolean }
  | { action: 'announcement'; id: string; body: string; seconds: number; enabled: boolean }
  | { action: 'maintenance'; enabled: boolean; notice: string }
  | { action: 'resolve'; report: string; accepted: boolean }
  | { action: 'system_tournament'; id: string; name: string; terms: Terms; reward: Reward }
  | { action: 'system_cancel' | 'review_signal'; id: string };
export type Request = { request: string; reason: string; operation: Operation };
