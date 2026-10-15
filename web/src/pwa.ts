import { createSignal } from 'solid-js';
import { duelActive } from './app/session';

export const [offlineStatus, setOfflineStatus] = createSignal<
  'loading' | 'ready' | 'failed' | 'local'
>('loading');
export const [updateReady, setUpdateReady] = createSignal(false);
let registration: ServiceWorkerRegistration | undefined;

export async function registerOffline() {
  if (import.meta.env.VITE_PLATFORM === 'android') {
    setOfflineStatus('ready');
    return;
  }
  if (!import.meta.env.PROD) {
    setOfflineStatus('local');
    return;
  }
  if (!('serviceWorker' in navigator)) {
    setOfflineStatus('failed');
    return;
  }
  try {
    registration = await navigator.serviceWorker.register('./sw.js');
    navigator.serviceWorker.addEventListener('message', (event) => {
      if (event.data === 'offline-ready') setOfflineStatus('ready');
      if (event.data === 'can-update') event.ports[0]?.postMessage(!duelActive());
    });
    registration.active?.postMessage('status');
    if (registration.waiting) setUpdateReady(true);
    const observeInstall = () => {
      const worker = registration?.installing;
      worker?.addEventListener('statechange', () => {
        if (worker.state === 'redundant' && !registration?.active) setOfflineStatus('failed');
        if (worker.state === 'installed' && navigator.serviceWorker.controller)
          setUpdateReady(true);
      });
    };
    registration.addEventListener('updatefound', observeInstall);
    observeInstall();
    await navigator.serviceWorker.ready;
    setOfflineStatus('ready');
  } catch {
    setOfflineStatus('failed');
  }
}

export function updateApp() {
  if (duelActive() || !registration?.waiting) return;
  navigator.serviceWorker.addEventListener(
    'controllerchange',
    () => {
      if (!duelActive()) location.reload();
    },
    {
      once: true,
    },
  );
  registration.waiting.postMessage('activate');
}
