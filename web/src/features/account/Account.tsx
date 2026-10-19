import { A, useLocation, useNavigate } from '@solidjs/router';
import { createEffect, createSignal, For, lazy, onCleanup, onMount, Show } from 'solid-js';
import {
  ApiError,
  account,
  api,
  authClient,
  enterGuest,
  forgetAccount,
  leaveGuest,
  restoreAccount,
  signIn,
  signOut,
} from '../../app/account';
import { icon } from '../../app/content';
import { onlineIcon } from '../../app/onlineIcons';
import { Scene } from '../../components/Scene';
import { version as legalVersion } from '../../generated/legal-version.json';
import { addTranslations, hasTranslation, t } from '../../i18n';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';

addTranslations({ pl, en });
// The account's painting (art/paintings/scenes).
const [painting] = Object.values(
  import.meta.glob<string>('../../assets/paintings/scenes/account.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);
const Notices = lazy(() => import('../moderation/Notices').then((m) => ({ default: m.Notices })));
const Privacy = lazy(() => import('../privacy/Privacy').then((m) => ({ default: m.Privacy })));

import { loadCaptcha } from './captcha';
import '../online.css';

type Mode = 'login' | 'register' | 'request' | 'verify' | 'reset';
type Session = { id: string; kind: string; device: string; current: boolean; last_seen_at: string };

function Captcha(props: {
  token: (value: string) => void;
  ready: (value: boolean) => void;
  error: () => void;
}) {
  let element!: HTMLDivElement;
  let disposed = false;
  let widget: string | undefined;
  onMount(async () => {
    props.ready(false);
    props.token('');
    try {
      const options = await api<{ captcha_site_key: string | null }>('/api/auth/options');
      if (disposed) return;
      if (!options.captcha_site_key) {
        props.ready(true);
        return;
      }
      const turnstile = await loadCaptcha();
      if (disposed) return;
      widget = turnstile.render(element, {
        sitekey: options.captcha_site_key,
        action: 'register',
        callback: (token) => {
          props.token(token);
          props.ready(true);
        },
        'expired-callback': () => {
          props.token('');
          props.ready(false);
        },
        'error-callback': () => {
          props.ready(false);
          props.error();
        },
      });
    } catch {
      if (!disposed) props.error();
    }
  });
  onCleanup(() => {
    disposed = true;
    if (widget) window.turnstile?.remove(widget);
  });
  return <div ref={element} />;
}

export function AccountPage() {
  const [privacyOpen, setPrivacyOpen] = createSignal(
    new URLSearchParams(location.hash.split('?')[1] ?? '').get('privacy') === '1',
  );
  const navigate = useNavigate();
  const route = useLocation();
  const query = new URLSearchParams(location.hash.split('?')[1] ?? '');
  const action = query.get('action');
  const [mode, setMode] = createSignal<Mode>(
    action === 'verify' || action === 'reset' ? action : 'login',
  );
  const [token, setToken] = createSignal(query.get('token') ?? '');
  const [busy, setBusy] = createSignal(false);
  const [notice, setNotice] = createSignal('');
  const [error, setError] = createSignal('');
  const [captchaToken, setCaptchaToken] = createSignal('');
  const [captchaReady, setCaptchaReady] = createSignal(false);
  const [captchaEpoch, setCaptchaEpoch] = createSignal(0);
  const [sessions, setSessions] = createSignal<Session[]>([]);
  const profile = () => {
    const value = account();
    return value?.kind === 'account' ? value.profile : null;
  };
  const isGuest = () => account()?.kind === 'guest';
  let mounted = true;
  onCleanup(() => {
    mounted = false;
  });
  onMount(() => {
    // Tokens stay in memory; remove them from the visible address/history before any request.
    if (query.has('token'))
      history.replaceState(
        null,
        '',
        `${location.pathname}${location.search}#/account?action=${mode()}`,
      );
    void restoreAccount();
  });
  createEffect(() => {
    // Hash-only email navigation reuses this page instead of remounting it.
    const query = new URLSearchParams(route.search);
    if (!query.has('token')) return;
    const action = query.get('action');
    if (action === 'verify' || action === 'reset') {
      setMode(action);
      setToken(query.get('token') ?? '');
      setNotice('');
      setError('');
    }
    history.replaceState(
      null,
      '',
      `${location.pathname}${location.search}#/account?action=${action === 'reset' ? 'reset' : 'verify'}`,
    );
  });
  createEffect(() => {
    const id = profile()?.id;
    if (id)
      void api<{ sessions: Session[] }>('/api/auth/sessions')
        .then((result) => {
          if (mounted && profile()?.id === id) setSessions(result.sessions);
        })
        .catch(() => {});
    else setSessions([]);
  });

  function showError(reason: unknown) {
    if (!mounted) return;
    const code = reason instanceof ApiError ? reason.code : 'SERVER_UNAVAILABLE';
    setError(
      t(
        hasTranslation(`auth-error-${code}`)
          ? `auth-error-${code}`
          : 'auth-error-SERVER_UNAVAILABLE',
        { seconds: reason instanceof ApiError ? reason.retryAfter : 0 },
      ),
    );
  }
  async function perform(action: () => Promise<void>) {
    if (busy()) return;
    setBusy(true);
    setNotice('');
    setError('');
    try {
      await action();
    } catch (reason) {
      showError(reason);
    } finally {
      if (mounted) setBusy(false);
    }
  }
  function switchMode(value: Mode) {
    setMode(value);
    setNotice('');
    setError('');
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    const value = (key: string) => String(data.get(key) ?? '');
    const current = mode();
    await perform(async () => {
      if (current === 'login') {
        await signIn(value('identity'), value('password'));
      } else if (current === 'register') {
        try {
          await api('/api/auth/register', {
            login: value('login'),
            email: value('email'),
            password: value('password'),
            school: value('school'),
            client: authClient(),
            captcha_token: captchaToken() || null,
            terms_version: legalVersion,
            age_confirmed: data.get('agreement') === 'on',
          });
          setMode('login');
          setNotice(t('auth-registered'));
        } finally {
          setCaptchaEpoch((value) => value + 1);
        }
      } else if (current === 'request') {
        await api('/api/auth/password-reset/request', { email: value('email') });
        setNotice(t('auth-email-sent'));
      } else if (current === 'verify') {
        await api('/api/auth/verify-email', { token: token() });
        // Refresh first: the sign-in form appears only when the reset below can no longer
        // clear what the player has started typing into it.
        await restoreAccount();
        setToken('');
        setMode('login');
        setNotice(t('auth-verified'));
      } else {
        await api('/api/auth/password-reset/confirm', {
          token: token(),
          password: value('password'),
        });
        forgetAccount();
        setToken('');
        setMode('login');
        setNotice(t('auth-reset-done'));
      }
      form.reset();
    });
    const password = form.elements.namedItem('password');
    if (password instanceof HTMLInputElement) password.value = '';
  }

  return (
    <section class="account-page">
      <Show when={account()?.kind !== 'guest'}>
        <Notices />
      </Show>
      <Scene src={painting}>
        <h1 class="plaque">{t('auth-account')}</h1>
      </Scene>
      <Show when={!isGuest()}>
        <details
          open={privacyOpen()}
          onToggle={(e) => {
            if (e.currentTarget.open) setPrivacyOpen(true);
          }}
        >
          <summary>{t('privacy-title')}</summary>
          <Show when={privacyOpen()}>
            <Privacy />
          </Show>
        </details>
      </Show>
      <Show when={notice()}>
        <p role="status">{notice()}</p>
      </Show>
      <Show when={error()}>
        <p class="account-error" role="alert">
          {error()}
        </p>
      </Show>
      <Show when={isGuest()}>
        <h2>{t('auth-guest')}</h2>
        <p>{t('auth-guest-description')}</p>
        <A class="button primary" href="/">
          {t('training')}
        </A>{' '}
        <button type="button" onClick={leaveGuest}>
          {t('auth-leave-guest')}
        </button>
      </Show>
      <Show when={profile()} keyed>
        {(user) => (
          <div>
            <h2>{user.login}</h2>
            <A class="button primary" href="/lobby">
              {t('lobby')}
            </A>
            <Show when={['admin', 'moderator'].includes(user.role)}>
              <A class="button" href="/admin">
                {t('administration')}
              </A>
            </Show>
            <p>
              {user.email} · {t(user.email_verified ? 'auth-confirmed' : 'auth-unconfirmed')}
            </p>
            <p>
              <img class="icon" src={icon(`school-${user.start_school}`)} alt="" />
              {t(`school-${user.start_school}-name`)} ·{' '}
              <img class="icon" src={onlineIcon('level')} alt="" />
              {t('auth-level', { level: user.avatar?.level ?? 1, xp: user.avatar?.xp ?? 0 })}
            </p>
            <Show when={!user.email_verified}>
              <button
                type="button"
                disabled={busy()}
                onClick={() =>
                  void perform(async () => {
                    await api('/api/auth/verify-email/request', { email: user.email });
                    setNotice(t('auth-email-sent'));
                  })
                }
              >
                {t('auth-resend')}
              </button>{' '}
            </Show>
            <button type="button" disabled={busy()} onClick={() => void perform(signOut)}>
              {t('auth-logout')}
            </button>
            <h3>{t('auth-sessions')}</h3>
            <ul class="account-sessions">
              <For each={sessions()}>
                {(session) => (
                  <li>
                    <span>
                      {session.device} {session.current ? t('auth-current') : ''}
                    </span>
                    <button
                      type="button"
                      disabled={busy()}
                      onClick={() =>
                        void perform(async () => {
                          await api(`/api/auth/sessions/${session.id}`, undefined, 'DELETE');
                          if (session.current) forgetAccount();
                          else setSessions((rows) => rows.filter((row) => row.id !== session.id));
                        })
                      }
                    >
                      {t('auth-revoke')}
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </div>
        )}
      </Show>
      <Show when={!isGuest() && (!profile() || mode() === 'verify' || mode() === 'reset')}>
        <nav class="account-tabs" aria-label={t('auth-account')}>
          <button
            type="button"
            aria-pressed={mode() === 'login'}
            onClick={() => switchMode('login')}
          >
            {t('auth-login')}
          </button>
          <button
            type="button"
            aria-pressed={mode() === 'register'}
            onClick={() => switchMode('register')}
          >
            {t('auth-register')}
          </button>
        </nav>
        <h2>{t(`auth-mode-${mode()}`)}</h2>
        <form class="account-form" onSubmit={(event) => void submit(event)}>
          <Show when={mode() === 'login'}>
            <label>
              {t('auth-identity')}
              <input name="identity" autocomplete="username" required maxLength={254} />
            </label>
          </Show>
          <Show when={mode() === 'register'}>
            <label>
              {t('auth-login-name')}
              <input
                name="login"
                autocomplete="username"
                required
                minLength={3}
                maxLength={20}
                pattern={String.raw`[A-Za-z0-9_\-]+`}
              />
            </label>
          </Show>
          <Show when={mode() === 'register' || mode() === 'request'}>
            <label>
              {t('auth-email')}
              <input name="email" type="email" autocomplete="email" required maxLength={254} />
            </label>
          </Show>
          <Show when={mode() === 'login' || mode() === 'register' || mode() === 'reset'}>
            <label>
              {t('auth-password')}
              <input
                name="password"
                type="password"
                autocomplete={mode() === 'login' ? 'current-password' : 'new-password'}
                required
                minLength={mode() === 'login' ? 1 : 10}
                maxLength={128}
              />
            </label>
          </Show>
          <Show when={mode() === 'register' || mode() === 'reset'}>
            <small>{t('auth-password-hint')}</small>
          </Show>
          <Show when={mode() === 'register'}>
            <label>
              {t('auth-school')}
              <select name="school">
                <For each={['combat', 'divine', 'element', 'arcane', 'necro', 'tech']}>
                  {(school) => <option value={school}>{t(`school-${school}-name`)}</option>}
                </For>
              </select>
            </label>
            <small>{t('auth-school-hint')}</small>
            <label class="account-agreement">
              <input type="checkbox" name="agreement" required />
              {t('auth-agreement')}
            </label>
            <p>
              <A href="/legal">{t('legal')}</A>
            </p>
            <For each={[captchaEpoch()]}>
              {(_) => (
                <Captcha
                  token={setCaptchaToken}
                  ready={setCaptchaReady}
                  error={() => showError(new ApiError('CAPTCHA_REQUIRED'))}
                />
              )}
            </For>
          </Show>
          <Show when={mode() === 'verify' || mode() === 'reset'}>
            <label>
              {t('auth-token')}
              <input
                name="token"
                value={token()}
                onInput={(event) => setToken(event.currentTarget.value)}
                autocomplete="off"
                required
                pattern="[a-fA-F0-9]{64}"
              />
            </label>
          </Show>
          <button
            class="primary"
            type="submit"
            disabled={busy() || (mode() === 'register' && !captchaReady())}
          >
            {t(busy() ? 'auth-working' : `auth-submit-${mode()}`)}
          </button>
        </form>
        <Show when={mode() === 'login'}>
          <button class="quiet" type="button" onClick={() => switchMode('request')}>
            {t('auth-forgot')}
          </button>
        </Show>
      </Show>
      <Show when={!isGuest()}>
        <aside class="account-guest">
          <h2>{t('auth-no-account')}</h2>
          <p>{t('auth-guest-description')}</p>
          <button
            type="button"
            disabled={busy()}
            onClick={() => {
              enterGuest();
              navigate('/');
            }}
          >
            {t('auth-play-guest')}
          </button>
        </aside>
      </Show>
    </section>
  );
}
