import { createSignal, onCleanup } from 'solid-js';
import { ApiError, accountEpoch, api } from '../../app/account';
import { Modal } from '../../components/Modal';
import { t } from '../../i18n';
import './translations';

export function Report(props: {
  kind: 'chat' | 'player' | 'clan';
  target: string;
  label: string;
  close: () => void;
}) {
  const [reason, setReason] = createSignal(''),
    [busy, setBusy] = createSignal(false),
    [error, setError] = createSignal('');
  const id = crypto.randomUUID();
  let pending: unknown,
    disposed = false;
  onCleanup(() => {
    disposed = true;
    pending = undefined;
  });
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy()) return;
    const epoch = accountEpoch();
    setBusy(true);
    setError('');
    pending ??= { id, kind: props.kind, target: props.target, reason: reason().trim() };
    try {
      await api('/api/reports', pending);
      if (!disposed && epoch === accountEpoch()) props.close();
    } catch (e) {
      if (!disposed && epoch === accountEpoch()) {
        setError(
          t(e instanceof ApiError && e.code === 'RATE_LIMITED' ? 'report-limit' : 'report-error'),
        );
        if (
          e instanceof ApiError &&
          !['SERVER_UNAVAILABLE', 'INTERNAL', 'UNAVAILABLE'].includes(e.code)
        )
          pending = undefined;
      }
    } finally {
      if (!disposed && epoch === accountEpoch()) setBusy(false);
    }
  }
  return (
    <Modal
      title={t('report-title')}
      close={() => {
        if (!busy()) props.close();
      }}
    >
      <p>{props.label}</p>
      <p>{t('report-help')}</p>
      <form class="account-form" onSubmit={(e) => void submit(e)}>
        <label>
          {t('report-reason')}
          <textarea
            minLength={5}
            maxLength={2000}
            required
            rows={4}
            value={reason()}
            disabled={busy() || !!pending}
            onInput={(e) => setReason(e.currentTarget.value)}
          />
        </label>
        <p role="alert">{error()}</p>
        <button type="submit" disabled={busy()}>
          {t('report-send')}
        </button>
      </form>
    </Modal>
  );
}
