import { useBeforeLeave } from '@solidjs/router';
import { createEffect, createSignal, onCleanup, Show } from 'solid-js';
import {
  ApiError,
  account,
  accountEpoch,
  api,
  forgetAccount,
  signIn,
  signOut,
} from '../../app/account';
import { setAccountRightsBusy } from '../../app/session';
import { Modal } from '../../components/Modal';
import { addTranslations, hasTranslation, t } from '../../i18n';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import './privacy.css';

addTranslations({ pl, en });
type Summary = {
  id: string;
  login: string;
  status: string;
  busy: boolean;
  clan_master: boolean;
  session: string;
};
export function Privacy() {
  const [data, setData] = createSignal<Summary>();
  const [busy, setBusy] = createSignal(false);
  const [open, setOpen] = createSignal(false);
  const [error, setError] = createSignal('');
  const [notice, setNotice] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [confirmation, setConfirmation] = createSignal('');
  const [ack, setAck] = createSignal(false);
  let disposed = false,
    sequence = 0;
  onCleanup(() => {
    setAccountRightsBusy(false);
    disposed = true;
    setPassword('');
  });
  createEffect(() => setAccountRightsBusy(busy()));
  function failure(reason: unknown, session = false) {
    const code = reason instanceof ApiError ? reason.code : 'DEFAULT';
    if (session && code === 'UNAUTHENTICATED') {
      setData(undefined);
      setOpen(false);
      setPassword('');
    }
    setError(
      t(
        hasTranslation(`privacy-error-${code}`) ? `privacy-error-${code}` : 'privacy-error-DEFAULT',
      ),
    );
  }
  async function load() {
    const request = ++sequence,
      epoch = accountEpoch();
    try {
      const value = await api<Summary>('/api/me/privacy');
      if (!disposed && epoch === accountEpoch() && request === sequence) setData(value);
    } catch {
      if (!disposed && epoch === accountEpoch() && request === sequence) setData(undefined);
    }
  }
  createEffect(() => {
    account();
    setPassword('');
    setOpen(false);
    setData(undefined);
    void load();
  });
  useBeforeLeave((event) => {
    if (!open() && !busy()) return;
    event.preventDefault();
    if (!busy() && window.confirm(t('privacy-leave'))) event.retry(true);
  });
  const close = () => {
    if (!busy()) {
      setOpen(false);
      setPassword('');
      setConfirmation('');
      setAck(false);
    }
  };
  async function download() {
    if (busy() || !data()) return;
    const epoch = accountEpoch(),
      expected = data()?.id;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const fresh = await api<Summary>('/api/me/privacy');
      if (disposed || epoch !== accountEpoch()) return;
      if (fresh.id !== expected) throw new ApiError('SESSION_CHANGED');
      if (import.meta.env.VITE_PLATFORM === 'android') {
        const { saveExport } = await import('../../platform/export');
        const result = await saveExport(fresh.session);
        if (!disposed && epoch === accountEpoch() && result.saved)
          setNotice(t('privacy-export-saved'));
      } else {
        const link = document.createElement('a');
        link.href = `/api/me/export?account=${encodeURIComponent(fresh.id)}`;
        link.download = 'spellgambit-data.json';
        link.click();
        setNotice(t('privacy-export-started'));
      }
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) failure(reason, true);
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  async function remove(event: SubmitEvent) {
    event.preventDefault();
    const own = data();
    if (!own || busy() || !ack() || confirmation() !== own.login) return;
    const epoch = accountEpoch(),
      secret = password();
    setBusy(true);
    setError('');
    setNotice('');
    try {
      try {
        await api('/api/me', { password: secret, confirm_login: confirmation() }, 'DELETE');
      } catch (reason) {
        if (disposed || epoch !== accountEpoch()) return;
        if (
          !(reason instanceof ApiError) ||
          !['SERVER_UNAVAILABLE', 'UNAUTHENTICATED'].includes(reason.code)
        )
          throw reason;
        const status = await api<{ pending: boolean }>('/api/auth/deletion-status', {
          identity: own.login,
          password: secret,
        });
        if (!status.pending) throw reason;
      }
      if (disposed || epoch !== accountEpoch()) return;
      setOpen(false);
      setPassword('');
      setData(undefined);
      forgetAccount();
      setNotice(t('privacy-requested'));
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) failure(reason, true);
    } finally {
      if (!disposed) {
        setBusy(false);
        setPassword('');
      }
    }
  }
  async function restore(event: SubmitEvent) {
    event.preventDefault();
    if (busy()) return;
    const form = event.currentTarget as HTMLFormElement,
      values = new FormData(form),
      epoch = accountEpoch();
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await api('/api/auth/cancel-deletion', {
        identity: values.get('identity'),
        password: values.get('password'),
      });
      if (!disposed && epoch === accountEpoch()) {
        form.reset();
        setNotice(t('privacy-restored'));
      }
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) failure(reason);
    } finally {
      const field = form.elements.namedItem('password');
      if (field instanceof HTMLInputElement) field.value = '';
      if (!disposed) setBusy(false);
    }
  }
  async function login(event: SubmitEvent) {
    event.preventDefault();
    if (busy()) return;
    const form = event.currentTarget as HTMLFormElement,
      values = new FormData(form);
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await signIn(String(values.get('identity')), String(values.get('password')), true);
      if (!disposed) {
        form.reset();
        await load();
      }
    } catch (reason) {
      if (!disposed) failure(reason);
    } finally {
      const field = form.elements.namedItem('password');
      if (field instanceof HTMLInputElement) field.value = '';
      if (!disposed) setBusy(false);
    }
  }
  return (
    <section class="privacy-controls" aria-busy={busy()}>
      <h2>{t('privacy-heading')}</h2>
      <p>{t('privacy-intro')}</p>
      <Show when={notice()}>
        <p role="status">{notice()}</p>
      </Show>
      <Show when={error()}>
        <p role="alert">{error()}</p>
      </Show>
      <Show when={data()} fallback={<p>{t('privacy-sign-in')}</p>}>
        {(own) => (
          <>
            <p>{t('privacy-account', { login: own().login })}</p>
            <button type="button" disabled={busy()} onClick={() => void download()}>
              {t('privacy-export')}
            </button>
            <Show when={account()?.kind !== 'account'}>
              <button
                type="button"
                disabled={busy()}
                onClick={() =>
                  void signOut()
                    .then(() => setData(undefined))
                    .catch(failure)
                }
              >
                {t('privacy-logout')}
              </button>
            </Show>
            <Show when={own().busy}>
              <p>{t('privacy-busy')}</p>
            </Show>
            <Show when={own().clan_master}>
              <p>{t('privacy-master')}</p>
            </Show>
            <button
              type="button"
              disabled={busy() || own().busy || own().clan_master}
              onClick={() => {
                setOpen(true);
                setAck(false);
                setConfirmation('');
                setPassword('');
              }}
            >
              {t('privacy-delete')}
            </button>
          </>
        )}
      </Show>
      <Show when={!data()}>
        <form onSubmit={login} class="privacy-restore" aria-label={t('privacy-login-title')}>
          <h3>{t('privacy-login-title')}</h3>
          <p>{t('privacy-login-help')}</p>
          <label>
            {t('privacy-login-identity')}
            <input name="identity" autocomplete="username" required maxLength={254} />
          </label>
          <label>
            {t('privacy-password')}
            <input
              type="password"
              name="password"
              autocomplete="current-password"
              required
              maxLength={512}
            />
          </label>
          <button type="submit" disabled={busy()}>
            {t('privacy-login')}
          </button>
        </form>
      </Show>
      <form onSubmit={restore} class="privacy-restore" aria-label={t('privacy-restore-title')}>
        <h3>{t('privacy-restore-title')}</h3>
        <p>{t('privacy-restore-help')}</p>
        <label>
          {t('privacy-identity')}
          <input name="identity" autocomplete="username" required maxLength={254} />
        </label>
        <label>
          {t('privacy-password')}
          <input
            type="password"
            name="password"
            autocomplete="current-password"
            required
            maxLength={512}
          />
        </label>
        <button type="submit" disabled={busy()}>
          {t('privacy-restore')}
        </button>
      </form>
      <Show when={open() && data()}>
        {(own) => (
          <Modal title={t('privacy-delete')} close={close}>
            <form onSubmit={remove} class="privacy-confirm">
              <p>{t('privacy-delete-warning', { login: own().login })}</p>
              <label>
                {t('privacy-confirm-login')}
                <input
                  value={confirmation()}
                  onInput={(e) => setConfirmation(e.currentTarget.value)}
                  autocomplete="off"
                  required
                />
              </label>
              <label>
                {t('privacy-password')}
                <input
                  type="password"
                  value={password()}
                  onInput={(e) => setPassword(e.currentTarget.value)}
                  autocomplete="current-password"
                  required
                  maxLength={512}
                />
              </label>
              <label class="privacy-check">
                <input
                  type="checkbox"
                  checked={ack()}
                  onChange={(e) => setAck(e.currentTarget.checked)}
                />
                {t('privacy-ack')}
              </label>
              <button type="submit" disabled={busy() || !ack() || confirmation() !== own().login}>
                {t(busy() ? 'privacy-working' : 'privacy-confirm-delete')}
              </button>
              <button type="button" disabled={busy()} onClick={close}>
                {t('cancel')}
              </button>
            </form>
          </Modal>
        )}
      </Show>
    </section>
  );
}
