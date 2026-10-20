import { A } from '@solidjs/router';
import { createResource, type ParentProps, Show } from 'solid-js';
import { account, api } from '../../app/account';
import { t } from '../../i18n';
import './translations';
import { canEnter, type Journey, requiredLevel } from './journey';
export function JourneyGate(props: ParentProps<{ place: string }>) {
  const [journey, { refetch }] = createResource(
    () => {
      const session = account();
      return session?.kind === 'account' ? session.profile.id : false;
    },
    () => api<Journey>('/api/journey'),
  );
  return (
    <Show
      when={account()?.kind !== 'account' || (!journey.error && canEnter(journey(), props.place))}
      fallback={
        <section class="settings-page">
          <h1>{t(`lobby-room-${props.place}`)}</h1>
          <Show when={!journey.loading} fallback={<p>{t('lobby-status-connecting')}</p>}>
            <p>
              {t(journey.error ? 'lobby-error-default' : 'lobby-location-locked', {
                level: requiredLevel(journey.error ? undefined : journey(), props.place),
              })}
            </p>
            <Show when={journey.error}>
              <button type="button" onClick={() => void refetch()}>
                {t('lobby-reconnect')}
              </button>
            </Show>
          </Show>
          <A href="/lobby">{t('lobby-tab-map')}</A>
        </section>
      }
    >
      {props.children}
    </Show>
  );
}
