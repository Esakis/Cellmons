import { createEffect, createSignal, For, onCleanup, Show, untrack } from 'solid-js';
import { account, accountEpoch, api } from '../../app/account';
import { t } from '../../i18n';
import './translations';

type Notice = { id: string; kind?: string; body: string; created_at?: string };
export function Notices(props: { bulletin?: boolean }) {
  const [notices, setNotices] = createSignal<Notice[]>([]),
    [maintenance, setMaintenance] = createSignal('');
  let disposed = false,
    sequence = 0;
  async function load() {
    if (account()?.kind === 'guest') return;
    const epoch = accountEpoch(),
      request = ++sequence;
    try {
      const value = await api<{
        notices?: Notice[];
        announcements?: Notice[];
        control?: { maintenance: boolean; notice: string };
      }>(props.bulletin ? '/api/bulletin' : '/api/notices');
      if (!disposed && epoch === accountEpoch() && request === sequence) {
        setNotices(value.notices ?? value.announcements ?? []);
        setMaintenance(
          value.control?.maintenance ? value.control.notice || t('report-maintenance') : '',
        );
      }
    } catch {
      if (!disposed && epoch === accountEpoch() && request === sequence) {
        setNotices([]);
        setMaintenance('');
      }
    }
  }
  createEffect(() => {
    account();
    sequence++;
    setNotices([]);
    setMaintenance('');
    void untrack(load);
  });
  const timer = setInterval(() => {
    if (document.visibilityState === 'visible') void load();
  }, 30000);
  onCleanup(() => {
    disposed = true;
    sequence++;
    clearInterval(timer);
  });
  return (
    <>
      <Show when={maintenance()}>
        <p role="status" class="lobby-notice">
          {maintenance()}
        </p>
      </Show>
      <Show when={notices().length}>
        <details class="account-notices">
          <summary>
            {t(props.bulletin ? 'report-announcements' : 'report-decisions')} ({notices().length})
          </summary>
          <ul>
            <For each={notices()}>
              {(notice) => (
                <li>
                  <Show when={notice.kind}>
                    <strong>{t(`report-kind-${notice.kind}`)}: </strong>
                  </Show>
                  <span>{notice.body}</span>
                  <Show when={notice.created_at}>
                    <small> · {new Date(notice.created_at ?? '').toLocaleString()}</small>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </details>
      </Show>
    </>
  );
}
