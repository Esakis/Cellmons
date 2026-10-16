import { createSignal, Show } from 'solid-js';
import { account } from '../app/account';
import { t } from '../i18n';
import {
  awake,
  haptics,
  metadata,
  nativeUpdateRequired,
  option,
  permission,
  platformError,
  reminders,
} from './android';
import './translations';
import './settings.css';
export function NativeSettings() {
  const [busy, setBusy] = createSignal(false);
  return (
    <section class="native-settings">
      <h2>{t('android-title')}</h2>
      <p>
        {t('android-native-version')}: {metadata()?.nativeVersion ?? '—'}
      </p>
      <Show when={platformError()}>
        <p role="alert">{platformError()}</p>
      </Show>
      <Show when={nativeUpdateRequired()}>
        <p role="alert">{t('android-update-required')}</p>
        <Show when={metadata()?.storeUrl}>
          <a href={metadata()?.storeUrl} target="_blank" rel="noopener noreferrer">
            {t('android-update-app')}
          </a>
        </Show>
      </Show>
      <Show when={metadata() && Number(metadata()?.webViewVersion.split('.')[0]) < 113}>
        <p role="alert">{t('android-webview-old')}</p>
        <a
          href="https://play.google.com/store/apps/details?id=com.google.android.webview"
          target="_blank"
          rel="noopener noreferrer"
        >
          {t('android-webview-update')}
        </a>
      </Show>
      <label>
        <input
          type="checkbox"
          checked={haptics()}
          onChange={(e) => option('haptics', e.currentTarget.checked)}
        />
        {t('android-haptics')}
      </label>
      <label>
        <input
          type="checkbox"
          checked={awake()}
          onChange={(e) => option('awake', e.currentTarget.checked)}
        />
        {t('android-awake')}
      </label>
      <Show when={account()?.kind === 'account'}>
        <label>
          <input
            type="checkbox"
            checked={reminders()}
            disabled={busy()}
            onChange={async (e) => {
              const enabled = e.currentTarget.checked;
              if (!enabled) {
                option('reminders', false);
                return;
              }
              setBusy(true);
              await permission();
              setBusy(false);
            }}
          />
          {t('android-reminders')}
        </label>
        <Show when={metadata() && !metadata()?.notificationsAllowed}>
          <p>{t('android-notifications-denied')}</p>
        </Show>
      </Show>
    </section>
  );
}
