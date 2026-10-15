import { accountEpoch } from '../../app/account';

export type Stack = { card_id: number; qty: number };
export type Slot = { rarity: string; count: number; pool: number[] };
export type Inventory = {
  content_version: string;
  offers: { id: string; price: string; quote: string; available: boolean; slots: Slot[] }[];
  boosters: {
    id: string;
    kind: string;
    status: string;
    price: string;
    contents: number[] | null;
    slots: Slot[];
  }[];
  junkyard_points: string;
  junkyard_rewards: string;
  junkyard_available: boolean;
};
export type Wallet = {
  balance: string;
  entries: { id: string; delta: string; reason: string; at_ms: number }[];
};
export type Operation =
  | { action: 'buy'; kind: string; quote: string }
  | { action: 'open'; id: string }
  | { action: 'discard'; cards: Stack[] };
export type Request = { key: string; operation: Operation };
export type Receipt = {
  key: string;
  action: string;
  booster: string | null;
  cards: number[];
  discarded: Stack[];
  points: string;
  unlocked: number[];
  ducats: string;
};
// Keep the same key after an uncertain network result, including route changes.
// Account switches always discard it; it never enters browser storage.
let pending: Request | undefined;
let epoch = -1;
export function pendingRequest() {
  if (epoch !== accountEpoch()) {
    pending = undefined;
    epoch = accountEpoch();
  }
  return pending;
}
export function startRequest(operation: Operation): Request {
  if (pendingRequest()) throw new Error('PENDING');
  pending = { key: crypto.randomUUID(), operation };
  return pending;
}
export function finishRequest(key: string) {
  if (pendingRequest()?.key === key) pending = undefined;
}
