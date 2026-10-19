import { A, useLocation } from '@solidjs/router';
import {
  createEffect,
  createSignal,
  ErrorBoundary,
  onCleanup,
  type ParentProps,
  Show,
} from 'solid-js';
import { language, setLanguage, t } from '../i18n';
import { offlineStatus, updateApp, updateReady } from '../pwa';
import styles from './App.module.css';
import { icon } from './content';
import { duelActive, placeBackdrop, theme, updatesBlocked } from './session';

// Each view has its own dimmed painting behind the content (art/paintings/backdrops).
const backdrops = import.meta.glob<string>('../assets/paintings/backdrops/*.avif', {
  eager: true,
  query: '?url',
  import: 'default',
});
const backdropViews = ['training', 'duel', 'lobby', 'collection', 'store'];

function backdrop(path: string) {
  const view = path.split('/')[1] || 'training';
  const name = backdropViews.includes(view) ? view : 'default';
  return backdrops[`../assets/paintings/backdrops/${name}.avif`];
}

export function Shell(props: ParentProps) {
  const route = useLocation();
  const lightQuery =
    typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: light)') : undefined;
  const [systemLight, setSystemLight] = createSignal(lightQuery?.matches ?? false);
  const follow = (event: MediaQueryListEvent) => setSystemLight(event.matches);
  lightQuery?.addEventListener('change', follow);
  onCleanup(() => lightQuery?.removeEventListener('change', follow));
  createEffect(() => {
    document.documentElement.lang = language();
  });
  createEffect(() => {
    const light = theme() === 'light' || (theme() === 'system' && systemLight());
    document.documentElement.dataset.theme = light ? 'light' : 'dark';
  });
  createEffect(() => {
    const url = placeBackdrop() ?? backdrop(route.pathname);
    document.documentElement.style.setProperty('--backdrop', url ? `url("${url}")` : 'none');
    document.documentElement.toggleAttribute('data-place', Boolean(placeBackdrop()));
  });
  return (
    <>
      <button
        type="button"
        class="skip-link"
        onClick={() => {
          const main = document.getElementById('main');
          main?.focus();
          main?.scrollIntoView();
        }}
      >
        {t('skip-to-content')}
      </button>
      <header class={styles.header}>
        <A class={styles.brand} href="/">
          <span class={styles.brandIcon} aria-hidden="true">
            ✦
          </span>
          <span>
            Spellgambit <small>{t('brand-subtitle')}</small>
          </span>
        </A>
        {/* On phones this is the icon bar along the bottom edge, like in mobile games. */}
        <nav aria-label={t('app-title')}>
          <A href="/" end>
            <img src={icon('nav-training')} alt="" width="24" height="24" />
            {t('training')}
          </A>
          <A href="/lobby">
            <img src={icon('nav-lobby')} alt="" width="24" height="24" />
            {t('lobby')}
          </A>
          <A href="/collection">
            <img src={icon('nav-collection')} alt="" width="24" height="24" />
            {t('collection')}
          </A>
          <A href="/store">
            <img src={icon('nav-store')} alt="" width="24" height="24" />
            {t('store')}
          </A>
          <A href="/account">
            <img src={icon('nav-account')} alt="" width="24" height="24" />
            {t('auth-account')}
          </A>
          <A href="/settings">
            <img src={icon('nav-settings')} alt="" width="24" height="24" />
            {t('settings')}
          </A>
        </nav>
        <select
          class={styles.language}
          aria-label={t('language')}
          value={language()}
          onChange={(event) => setLanguage(event.currentTarget.value)}
        >
          <option value="pl">PL</option>
          <option value="en">EN</option>
        </select>
      </header>
      <main id="main" tabIndex={-1}>
        <ErrorBoundary
          fallback={
            <section class="settings-page">
              <h1>{t('error-title')}</h1>
              <p>{t('error-generic')}</p>
              <button
                type="button"
                class="secondary"
                onClick={() => location.assign(new URL('./', location.href).href)}
              >
                {t('back')}
              </button>
            </section>
          }
        >
          {props.children}
        </ErrorBoundary>
      </main>
      <Show when={!duelActive()}>
        <footer class={styles.footer}>
          <span>Spellgambit: Arcane Class</span>
          <A href="/legal">{t('legal')}</A>
          <span role="status" data-testid="offline-status">
            {t(
              offlineStatus() === 'ready'
                ? 'offline'
                : offlineStatus() === 'failed'
                  ? 'offline-failed'
                  : offlineStatus() === 'local'
                    ? 'offline-local'
                    : 'caching',
            )}
          </span>
        </footer>
      </Show>
      <Show when={updateReady() && !updatesBlocked()}>
        <div class={styles.update} role="status">
          {t('update')}{' '}
          <button type="button" onClick={updateApp}>
            {t('refresh')}
          </button>
        </div>
      </Show>
    </>
  );
}
