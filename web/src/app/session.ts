import { createSignal } from 'solid-js';
import { readPreference, writePreference } from './preferences';

export const [duelActive, setDuelActive] = createSignal(false);
export const [speed, setSpeedSignal] = createSignal(readPreference('speed', 'normal'));
export const [theme, setThemeSignal] = createSignal(readPreference('theme', 'dark'));
export function setTheme(value: string) {
  if (!['dark', 'light', 'system'].includes(value)) return;
  setThemeSignal(value);
  writePreference('theme', value);
}
export function setSpeed(value: string) {
  if (!['normal', 'fast', 'instant'].includes(value)) return;
  setSpeedSignal(value);
  writePreference('speed', value);
}
