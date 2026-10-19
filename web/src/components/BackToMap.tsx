import { A } from '@solidjs/router';
import { t } from '../i18n';

/** The arrow of the way back, drawn in the medal of `.back-to-map` (global.css). */
export function BackArrow() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M15.4 4.6 8 12l7.4 7.4 1.6-1.6L11.2 12 17 6.2z" />
    </svg>
  );
}

/** A screen that is a building of the town (the library, the shop, the clans) leads back to the map. */
export function BackToMap() {
  return (
    <A
      class="back-to-map medal"
      href="/lobby"
      aria-label={t('back-to-map')}
      title={t('back-to-map')}
    >
      <BackArrow />
    </A>
  );
}
