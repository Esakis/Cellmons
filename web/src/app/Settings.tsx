import { lazy } from 'solid-js';
import { language, setLanguage, t } from '../i18n';
import { setSpeed, setTheme, speed, theme } from './session';

const NativeSettings =
  import.meta.env.VITE_PLATFORM === 'android'
    ? lazy(() => import('../platform/Settings').then((m) => ({ default: m.NativeSettings })))
    : null;

export function Settings() {
  return (
    <section class="settings-page">
      <p class="eyebrow">SPELLGAMBIT</p>
      <h1>{t('settings')}</h1>
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
      <p>{t('settings-note')}</p>
      {NativeSettings && <NativeSettings />}
    </section>
  );
}
