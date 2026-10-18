import { createSignal } from 'solid-js';
import { readPreference, writePreference } from './preferences';

export const [duelActive, setDuelActive] = createSignal(false);
export const [activityBusy, setActivityBusy] = createSignal(false);
export const [accountRightsBusy, setAccountRightsBusy] = createSignal(false);
export const [onlineDuelActive, setOnlineDuelActive] = createSignal(false);
export const [sessionKnown, setSessionKnown] = createSignal(false);
export const [appForeground, setAppForeground] = createSignal(true);
// Painting of the place the player is in (lobby room); replaces the view's backdrop while set.
export const [placeBackdrop, setPlaceBackdrop] = createSignal<string>();
export const updatesBlocked = () =>
  duelActive() || activityBusy() || accountRightsBusy() || !sessionKnown();
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
