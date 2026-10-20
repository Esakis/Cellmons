import { A } from '@solidjs/router';
import { Dynamic } from 'solid-js/web';
import { t } from '../i18n';

/** The arrow of the way back, drawn in the medal of `.back-to-map` (global.css). */
export function BackArrow() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M15.4 4.6 8 12l7.4 7.4 1.6-1.6L11.2 12 17 6.2z" />
    </svg>
  );
}

/**
 * The way back to the town map: a link from a screen that is a building of the town (the
 * library, the shop, the clans), or a button inside a lobby room (`onClick`).
 */
export function BackToMap(props: { onClick?: () => void }) {
  return (
    <Dynamic
      component={props.onClick ? 'button' : A}
      type={props.onClick ? 'button' : undefined}
      href={props.onClick ? undefined : '/lobby'}
      class="back-to-map medal"
      aria-label={t('back-to-map')}
      title={t('back-to-map')}
      onClick={props.onClick}
    >
      <BackArrow />
    </Dynamic>
  );
}
