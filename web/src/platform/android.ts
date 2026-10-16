import { createEffect, createRoot, createSignal, untrack } from 'solid-js';
import { account, restoreAccount } from '../app/account';
import { content } from '../app/content';
import { readPreference, writePreference } from '../app/preferences';
import {
  appForeground,
  duelActive,
  onlineDuelActive,
  setAppForeground,
  updatesBlocked,
} from '../app/session';
import type { DuelSnapshot } from '../generated/proto/game/v1/online_duel_pb';
import type { DuelEvent } from '../generated/proto/game/v1/training_pb';
import { t } from '../i18n';
import { type Metadata, native } from './bridge';
import './translations';
export const [haptics, setHaptics] = createSignal(readPreference('haptics', 'on') === 'on');
export const [awake, setAwake] = createSignal(readPreference('awake', 'on') === 'on');
export const [reminders, setReminders] = createSignal(readPreference('reminders', 'off') === 'on');
export const [metadata, setMetadata] = createSignal<Metadata>();
export const [platformError, setPlatformError] = createSignal('');
let initialized = false;
const [pendingBundle, setPendingBundle] = createSignal(0);
export const [nativeUpdateRequired, setNativeUpdateRequired] = createSignal(false);
async function checkBundle(force = false) {
  if (account()?.kind !== 'account' && !force) return;
  try {
    const result = await native.checkBundle({ force });
    setPendingBundle(result.pending);
    setNativeUpdateRequired(result.needsNative);
  } catch {
    /* Keep the last verified offline bundle. */
  }
}
let hapticRound = '';
export function duelFeedback(snapshot: DuelSnapshot) {
  const round = `${snapshot.duelId}:${snapshot.view?.round}:${Boolean(snapshot.result)}`;
  if (round !== hapticRound && snapshot.events.length) haptic(snapshot.events);
  hapticRound = round;
}
export function option(key: 'haptics' | 'awake' | 'reminders', enabled: boolean) {
  writePreference(key, enabled ? 'on' : 'off');
  ({ haptics: setHaptics, awake: setAwake, reminders: setReminders })[key](enabled);
}
export async function refreshMetadata() {
  try {
    setMetadata(await native.metadata());
  } catch {
    setPlatformError(t('android-permission-error'));
  }
}
export async function permission() {
  try {
    const { allowed } = await native.enableReminders();
    option('reminders', allowed);
    await refreshMetadata();
    return allowed;
  } catch {
    setPlatformError(t('android-permission-error'));
    return false;
  }
}
export function haptic(events: readonly DuelEvent[]) {
  if (!haptics()) return;
  const kind = events.some((e) => e.kind === 'end')
    ? 'end'
    : events.some((e) => e.kind === 'hit' && e.oldValue >= 4)
      ? 'combo'
      : events.some((e) => e.kind === 'hit')
        ? 'hit'
        : undefined;
  if (kind) void native.haptic({ kind }).catch(() => {});
}
async function checkEngine() {
  const { TrainingClient } = await import('../worker/client');
  const client = new TrainingClient();
  try {
    const value = await client.request({
      case: 'start',
      value: {
        $typeName: 'game.v1.TrainingSetup',
        avatarId: 1,
        profileId: 'novice',
        seed: new Uint8Array(32),
        rotation: 0,
        autoplay: false,
        contentVersion: content.version,
      },
    });
    if (!value.view?.me || value.view.me.hp <= 0) throw new Error('ENGINE_FAILED');
  } finally {
    client.dispose();
  }
}
export async function initialize() {
  if (initialized) return;
  initialized = true;
  window.addEventListener('game-version-mismatch', () => void checkBundle(true));
  window.addEventListener('game-feedback', (event) =>
    haptic((event as CustomEvent<DuelEvent[]>).detail),
  );
  window.addEventListener('game-duel-feedback', (event) =>
    duelFeedback((event as CustomEvent<DuelSnapshot>).detail),
  );
  await native.addListener('pause', () => {
    setAppForeground(false);
    window.dispatchEvent(new Event('offline'));
  });
  await native.addListener('resume', () => {
    setAppForeground(true);
    window.dispatchEvent(new Event('offline'));
    window.dispatchEvent(new Event('online'));
    void refreshMetadata();
  });
  await native.addListener('network', (value) => {
    window.dispatchEvent(new Event('offline'));
    if (value.connected) window.dispatchEvent(new Event('online'));
  });
  await native.addListener('back', () => {
    const dialog = document.querySelector<HTMLDialogElement>('dialog[open]');
    if (dialog) {
      const cancel = new Event('cancel', { cancelable: true });
      if (dialog.dispatchEvent(cancel)) dialog.close();
      return;
    }
    if (duelActive()) {
      if (
        window.confirm(
          t(onlineDuelActive() ? 'android-background-warning' : 'android-background-training'),
        )
      )
        void native.minimize();
    } else if (location.hash && location.hash !== '#/' && location.hash !== '#') {
      if (history.length > 1) history.back();
      else location.hash = '/';
    } else void native.minimize();
  });
  createRoot(() => {
    createEffect(() => {
      if (account()?.kind === 'account') void checkBundle();
    });
    createEffect(() => {
      if (
        pendingBundle() > 0 &&
        appForeground() &&
        !updatesBlocked() &&
        account()?.kind === 'account'
      ) {
        setPendingBundle(0);
        void native.applyBundle({ idle: true }).catch(() => {});
      }
    });
    let identity = '';
    createEffect(() => {
      const a = account(),
        key = `${a?.kind}:${a?.kind === 'account' ? a.profile.id : ''}`;
      if (identity === key) return;
      identity = key;
      untrack(() => {
        setHaptics(readPreference('haptics', 'on') === 'on');
        setAwake(readPreference('awake', 'on') === 'on');
        setReminders(readPreference('reminders', 'off') === 'on');
      });
    });
    createEffect(() => {
      // Guest preferences and training stay in RAM; configure controls hardware only.
      const a = account();
      void native
        .configure({
          duel: duelActive(),
          online: onlineDuelActive(),
          awake: awake(),
          haptics: haptics(),
          reminders: reminders() && a?.kind === 'account',
          title: t('android-reminder-title'),
          body: t('android-reminder-body'),
        })
        .catch(() => {});
    });
  });
  await refreshMetadata();
  try {
    await checkEngine();
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    await native.confirmBundle();
    window.setInterval(() => void checkBundle(), 6 * 60 * 60 * 1000);
  } catch {
    setPlatformError(t('android-worker-error'));
  }
  await restoreAccount();
}
