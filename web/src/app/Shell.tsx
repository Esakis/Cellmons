import { A } from '@solidjs/router';
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
import { duelActive, theme } from './session';

export function Shell(props: ParentProps) {
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
  return (
    <>
      <a class="skip-link" href="#main">
        {t('training')}
      </a>
      <header class={styles.header}>
        <A class={styles.brand} href="/">
          <span class={styles.brandIcon} aria-hidden="true">
            ✦
          </span>
          <span>
            Spellgambit <small>{t('brand-subtitle')}</small>
          </span>
        </A>
        <nav aria-label={t('app-title')}>
          <A href="/" end>
            {t('training')}
          </A>
          <A href="/settings">{t('settings')}</A>
          <A href="/account">{t('auth-account')}</A>
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
      <main id="main">
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
      <footer class={styles.footer}>
        <span>Spellgambit: Arcane Class</span>
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
      <Show when={updateReady() && !duelActive()}>
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
