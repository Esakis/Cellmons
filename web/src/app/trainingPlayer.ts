// Deliberately separate from account APIs and storage; this is only an offline worker input.
import { accountEpoch } from './account';
export type LocalPlayer = { avatar: number; level: number; spent: number[]; deck: number[] };
let player: LocalPlayer | undefined;
let epoch = -1;
export function setTrainingPlayer(value: LocalPlayer) {
  player = structuredClone(value);
  epoch = accountEpoch();
}
export function trainingPlayer(avatar: number) {
  return epoch === accountEpoch() && location.hash.includes('custom=1') && player?.avatar === avatar
    ? player
    : undefined;
}
