import { lazy } from 'solid-js';
import { Scene } from '../components/Scene';
import { addTranslations, language, setLanguage, t } from '../i18n';
import styles from './App.module.css';
import { setSpeed, setTheme, speed, theme } from './session';
import en from './settings.en.ftl?raw';
import pl from './settings.pl.ftl?raw';

addTranslations({ pl, en });
// The settings' painting (art/paintings/scenes).
const [painting] = Object.values(
  import.meta.glob<string>('../assets/paintings/scenes/settings.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);

const NativeSettings =
  import.meta.env.VITE_PLATFORM === 'android'
    ? lazy(() => import('../platform/Settings').then((m) => ({ default: m.NativeSettings })))
    : null;

export function Settings() {
  return (
    <section class="settings-page">
      <Scene src={painting}>
        <h1 class="plaque">{t('settings')}</h1>
      </Scene>
      <label>
        {t('language')}
        <select value={language()} onChange={(event) => setLanguage(event.currentTarget.value)}>
          <option value="pl">Polski</option>
          <option value="en">English</option>
        </select>
      </label>
      <label>
        {t('theme')}
        <select value={theme()} onChange={(event) => setTheme(event.currentTarget.value)}>
          <option value="dark">{t('theme-dark')}</option>
          <option value="light">{t('theme-light')}</option>
          <option value="system">{t('theme-system')}</option>
        </select>
      </label>
      <label>
        {t('speed')}
        <select value={speed()} onChange={(event) => setSpeed(event.currentTarget.value)}>
          <option value="normal">{t('normal')}</option>
          <option value="fast">{t('fast')}</option>
          <option value="instant">{t('instant')}</option>
        </select>
      </label>
      <div class={styles.notes}>
        <p>{t('settings-note')}</p>
        {/* Attribution required by the CC BY 3.0 licence of the icons. */}
        <p>
          {t('credits-paintings')} {t('credits-icons')}{' '}
          <a href="https://game-icons.net" target="_blank" rel="noreferrer">
            game-icons.net
          </a>
          ,{' '}
          <a href="https://creativecommons.org/licenses/by/3.0/" target="_blank" rel="noreferrer">
            CC BY 3.0
          </a>
          .
        </p>
      </div>
      {NativeSettings && <NativeSettings />}
    </section>
  );
}
