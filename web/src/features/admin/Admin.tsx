import { A, useBeforeLeave } from '@solidjs/router';
import { batch, createEffect, createSignal, For, lazy, onCleanup, Show, untrack } from 'solid-js';
import { ApiError, account, accountEpoch, api, restoreAccount } from '../../app/account';
import { content, fluentKey } from '../../app/content';
import { Modal } from '../../components/Modal';
import { addTranslations, hasTranslation, t } from '../../i18n';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import type { Gate, Operation, Page, Request, Reward, Row, Setup } from './types';
import './admin.css';

addTranslations({ pl, en });
const CardOptions = lazy(() =>
  import('../../app/CardCatalogView').then((m) => ({ default: m.CardOptions })),
);
const categories = [
  'players',
  'reports',
  'chat',
  'signals',
  'clans',
  'filters',
  'announcements',
  'control',
  'tournaments',
  'audit',
];
export function Admin() {
  const [gate, setGate] = createSignal<Gate>();
  const [setup, setSetup] = createSignal<Setup>();
  const [data, setData] = createSignal<Page>();
  const [category, setCategory] = createSignal('players');
  const [search, setSearch] = createSignal('');
  const [page, setPage] = createSignal(0);
  const [selected, setSelected] = createSignal<Row>();
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal('');
  const [notice, setNotice] = createSignal('');
  const [reason, setReason] = createSignal('');
  const [pending, setPending] = createSignal<Request>();
  const [confirm, setConfirm] = createSignal(false);
  const [attempted, setAttempted] = createSignal(false);
  const [dirty, setDirty] = createSignal(false);
  const [playerAction, setPlayerAction] = createSignal('warn');
  const [rewardKind, setRewardKind] = createSignal('ducats');
  const [password, setPassword] = createSignal('');
  const [code, setCode] = createSignal('');
  const [now, setNow] = createSignal(Date.now());
  let factor: Record<string, unknown> | undefined;
  let disposed = false,
    sequence = 0,
    identity = '';
  const staff = () => {
    const a = account();
    return a?.kind === 'account' && ['admin', 'moderator'].includes(a.profile.role);
  };
  const admin = () => gate()?.role === 'admin';
  const valid = (epoch: number) => !disposed && epoch === accountEpoch() && staff();
  function failure(value: unknown) {
    const code = value instanceof ApiError ? value.code : 'UNAVAILABLE';
    setError(
      t(hasTranslation(`admin-error-${code}`) ? `admin-error-${code}` : 'admin-error-default'),
    );
    if (['STEP_UP_REQUIRED', 'FORBIDDEN', 'UNAUTHENTICATED'].includes(code)) {
      setGate(undefined);
      setData(undefined);
      setSelected(undefined);
    }
    return code;
  }
  async function readGate() {
    if (!staff()) return;
    const epoch = accountEpoch();
    try {
      const value = await api<Gate>('/api/admin/session');
      if (valid(epoch)) {
        setGate(value);
        if (!value.authorized) {
          setData(undefined);
          setSelected(undefined);
        }
      }
    } catch (e) {
      if (valid(epoch)) failure(e);
    }
  }
  async function load() {
    if (!gate()?.authorized || category() === 'tournaments') return;
    const epoch = accountEpoch(),
      request = ++sequence;
    try {
      const value = await api<Page>(
        `/api/admin?${new URLSearchParams({ category: category(), page: String(page()), q: search() })}`,
      );
      if (valid(epoch) && request === sequence) setData(value);
    } catch (e) {
      if (valid(epoch) && request === sequence) failure(e);
    }
  }
  async function authenticate(event: SubmitEvent) {
    event.preventDefault();
    if (busy()) return;
    setBusy(true);
    setError('');
    const epoch = accountEpoch();
    factor ??=
      !gate()?.enrolled && !setup()
        ? { action: 'setup', password: password() }
        : setup()
          ? { action: 'confirm', request: crypto.randomUUID(), code: code() }
          : { action: 'unlock', request: crypto.randomUUID(), password: password(), code: code() };
    try {
      const reply = await api<{ gate: Gate; setup?: Setup }>('/api/admin/session', factor);
      if (valid(epoch)) {
        setGate(reply.gate);
        setSetup(reply.setup);
        setPassword('');
        setCode('');
        factor = undefined;
      }
    } catch (e) {
      if (valid(epoch)) {
        const c = failure(e);
        if (!['SERVER_UNAVAILABLE', 'UNAVAILABLE', 'INTERNAL'].includes(c)) factor = undefined;
      }
    } finally {
      if (valid(epoch)) setBusy(false);
    }
  }
  async function lock() {
    if (busy()) return;
    setBusy(true);
    const epoch = accountEpoch();
    try {
      const value = await api<{ gate: Gate }>('/api/admin/session', { action: 'lock' });
      if (valid(epoch)) {
        setGate(value.gate);
        setSetup(undefined);
        setPassword('');
        setCode('');
        factor = undefined;
        setData(undefined);
        setSelected(undefined);
      }
    } catch (e) {
      if (valid(epoch)) failure(e);
    } finally {
      if (valid(epoch)) setBusy(false);
    }
  }
  function propose(operation: Operation) {
    if (pending() || busy()) return;
    if (reason().trim().length < 5) {
      setError(t('admin-reason-required'));
      return;
    }
    setPending({ request: crypto.randomUUID(), reason: reason().trim(), operation });
    setAttempted(false);
    setConfirm(true);
  }
  async function apply() {
    const request = pending();
    if (!request || busy()) return;
    const epoch = accountEpoch();
    setBusy(true);
    setError('');
    setAttempted(true);
    try {
      await api('/api/admin', request);
      if (valid(epoch)) {
        setPending(undefined);
        setConfirm(false);
        setSelected(undefined);
        setReason('');
        setDirty(false);
        setNotice(t('admin-saved'));
        await load();
        await restoreAccount();
      }
    } catch (e) {
      if (valid(epoch)) {
        const c = failure(e);
        if (!['SERVER_UNAVAILABLE', 'UNAVAILABLE', 'INTERNAL'].includes(c)) {
          setPending(undefined);
          setConfirm(false);
        }
      }
    } finally {
      if (valid(epoch)) setBusy(false);
    }
  }
  function change(value: string) {
    if (pending() || busy()) return;
    sequence++;
    batch(() => {
      setCategory(value);
      setPage(0);
      setSearch('');
      setSelected(undefined);
      setData(undefined);
    });
  }
  function form(event: SubmitEvent, build: (form: FormData) => Operation) {
    event.preventDefault();
    propose(build(new FormData(event.currentTarget as HTMLFormElement)));
  }
  const string = (f: FormData, key: string) => String(f.get(key) ?? '');
  const number = (f: FormData, key: string) => Number(f.get(key));
  const enabled = (f: FormData) => f.get('enabled') === 'on';
  const decisionDetails = (): [string, string][] => {
    const op = pending()?.operation;
    if (!op) return [];
    const yn = (value: boolean) => t(value ? 'admin-enabled' : 'admin-disabled');
    switch (op.action) {
      case 'ban':
        return [
          ['admin-duration', op.seconds === null ? t('admin-permanent') : String(op.seconds / 60)],
        ];
      case 'mute':
        return [['admin-duration', String(op.seconds / 60)]];
      case 'role':
        return [['admin-field-role', t(`admin-role-${op.role}`)]];
      case 'filter':
        return [
          ['admin-field-phrase', op.phrase],
          ['admin-field-enabled', yn(op.enabled)],
        ];
      case 'announcement':
        return [
          ['admin-field-body', op.body],
          ['admin-duration', String(op.seconds / 60)],
          ['admin-field-enabled', yn(op.enabled)],
        ];
      case 'maintenance':
        return [
          ['admin-field-maintenance', yn(op.enabled)],
          ['admin-field-notice', op.notice],
        ];
      case 'clan_edit':
        return [
          ['admin-field-description', op.description],
          ['admin-field-motd', op.motd],
        ];
      case 'resolve':
        return [['admin-field-status', t(op.accepted ? 'admin-accept' : 'admin-reject')]];
      case 'system_cancel':
      case 'review_signal':
        return [['admin-target-id', op.id]];
      case 'system_tournament': {
        const r = op.reward;
        const value =
          r.kind === 'ducats'
            ? String(r.amount)
            : r.kind === 'avatar'
              ? t(fluentKey(avatarsForReward(r.template)))
              : r.kind === 'card'
                ? String(r.card)
                : r.booster;
        return [
          ['admin-tournament-name', op.name],
          ['admin-teams', String(op.terms.teams)],
          ['admin-team-size', String(op.terms.team_size)],
          ['admin-allowed-clans', op.terms.allowed_clans.join(', ')],
          ['admin-games', String(op.terms.max_games)],
          ['admin-min-level', String(op.terms.min_level)],
          ['admin-max-level', String(op.terms.max_level)],
          [`admin-prize-${r.kind}`, value],
        ];
      }
      default:
        return [];
    }
  };
  const avatarsForReward = (id: number) => content.avatars.find((a) => a.id === id)?.key ?? '';
  createEffect(() => {
    const a = account(),
      next = `${a?.kind}:${a?.kind === 'account' ? a.profile.id : ''}:${accountEpoch()}`;
    if (identity === next) return;
    identity = next;
    sequence++;
    setGate(undefined);
    setData(undefined);
    setSetup(undefined);
    setPending(undefined);
    setConfirm(false);
    setPassword('');
    setCode('');
    setReason('');
    setDirty(false);
    setAttempted(false);
    factor = undefined;
    setBusy(false);
    setSelected(undefined);
    if (staff()) void untrack(readGate);
  });
  createEffect(() => {
    if (gate()?.authorized) {
      category();
      page();
      void untrack(load);
    }
  });
  const timer = setInterval(() => {
    setNow(Date.now());
    if (gate()?.expires_ms && (gate()?.expires_ms ?? 0) <= Date.now()) {
      setGate((g) => g && { ...g, authorized: false });
      setData(undefined);
      setSelected(undefined);
    }
  }, 1000);
  const poll = setInterval(() => {
    if (document.visibilityState === 'visible' && !busy() && staff()) void readGate();
  }, 30000);
  useBeforeLeave((event) => {
    if (pending() || setup() || dirty()) {
      event.preventDefault();
      if (window.confirm(t('admin-leave'))) {
        setPending(undefined);
        setSetup(undefined);
        factor = undefined;
        event.retry(true);
      }
    }
  });
  onCleanup(() => {
    disposed = true;
    sequence++;
    clearInterval(timer);
    clearInterval(poll);
    factor = undefined;
    setPassword('');
    setCode('');
    setSetup(undefined);
  });
  return (
    <section
      class="admin-page"
      onInput={(event) => {
        if ((event.target as HTMLElement).closest('fieldset')) setDirty(true);
      }}
    >
      <p class="eyebrow">SPELLGAMBIT</p>
      <h1>{t('admin-title')}</h1>
      <Show
        when={staff()}
        fallback={
          <p>
            {t('admin-gate')} <A href="/account">{t('auth-account')}</A>
          </p>
        }
      >
        <Show when={error()}>
          <p role="alert" class="admin-error">
            {error()}
          </p>
        </Show>
        <Show when={notice()}>
          <p role="status">{notice()}</p>
        </Show>
        <Show
          when={gate()}
          fallback={
            <button type="button" onClick={() => void readGate()}>
              {t('refresh')}
            </button>
          }
        >
          <Show
            when={gate()?.authorized}
            fallback={
              <form class="admin-form" onSubmit={(e) => void authenticate(e)}>
                <h2>
                  {t(
                    setup()
                      ? 'admin-confirm-factor'
                      : gate()?.enrolled
                        ? 'admin-unlock'
                        : 'admin-enroll',
                  )}
                </h2>
                <p>{t('admin-factor-help')}</p>
                <Show when={!setup()}>
                  <label>
                    {t('auth-password')}
                    <input
                      type="password"
                      required
                      autocomplete="current-password"
                      maxLength={512}
                      value={password()}
                      onInput={(e) => {
                        setPassword(e.currentTarget.value);
                        factor = undefined;
                      }}
                      disabled={busy()}
                    />
                  </label>
                </Show>
                <Show when={setup()}>
                  {(value) => (
                    <div class="admin-secret">
                      <p>{t('admin-manual-secret')}</p>
                      <code>{value().secret}</code>
                      <p>
                        <a href={value().uri}>{t('admin-open-authenticator')}</a>
                      </p>
                      <p>
                        {t('admin-setup-expiry', {
                          minutes: Math.max(0, Math.ceil((value().expires_ms - now()) / 60000)),
                        })}
                      </p>
                      <button
                        type="button"
                        disabled={busy()}
                        onClick={() => {
                          setSetup(undefined);
                          setCode('');
                          factor = undefined;
                        }}
                      >
                        {t('admin-restart-factor')}
                      </button>
                    </div>
                  )}
                </Show>
                <Show when={setup() || gate()?.enrolled}>
                  <label>
                    {t('admin-otp')}
                    <input
                      inputmode="numeric"
                      autocomplete="one-time-code"
                      required
                      pattern="[0-9]{6}"
                      maxLength={6}
                      value={code()}
                      onInput={(e) => {
                        setCode(e.currentTarget.value);
                        factor = undefined;
                      }}
                      disabled={busy()}
                    />
                  </label>
                </Show>
                <button class="primary" type="submit" disabled={busy()}>
                  {t(
                    setup()
                      ? 'admin-confirm-factor'
                      : gate()?.enrolled
                        ? 'admin-unlock'
                        : 'admin-enroll',
                  )}
                </button>
              </form>
            }
          >
            <div class="admin-heading">
              <p>
                {t('admin-session-expiry', {
                  minutes: Math.max(0, Math.ceil(((gate()?.expires_ms ?? 0) - now()) / 60000)),
                })}
              </p>
              <button type="button" disabled={busy()} onClick={() => void lock()}>
                {t('admin-lock')}
              </button>
            </div>
            <nav class="admin-tabs" aria-label={t('admin-title')}>
              <For
                each={categories.filter(
                  (c) =>
                    admin() || !['filters', 'announcements', 'control', 'tournaments'].includes(c),
                )}
              >
                {(c) => (
                  <button
                    type="button"
                    aria-current={category() === c ? 'page' : undefined}
                    disabled={busy() || !!pending()}
                    onClick={() => change(c)}
                  >
                    {t(`admin-tab-${c}`)}
                  </button>
                )}
              </For>
            </nav>
            <Show when={category() !== 'tournaments'}>
              <form
                class="admin-search"
                onSubmit={(e) => {
                  e.preventDefault();
                  setPage(0);
                  void load();
                }}
              >
                <label>
                  {t('admin-search')}
                  <input
                    value={search()}
                    maxLength={100}
                    onInput={(e) => setSearch(e.currentTarget.value)}
                    disabled={busy() || !!pending()}
                  />
                </label>
                <button type="submit" disabled={busy() || !!pending()}>
                  {t('refresh')}
                </button>
              </form>
            </Show>
            <label>
              {t('admin-reason')}
              <textarea
                value={reason()}
                onInput={(e) => setReason(e.currentTarget.value)}
                minLength={5}
                maxLength={2000}
                rows={3}
                disabled={busy() || !!pending()}
              />
            </label>
            <Show when={pending()}>
              <div class="admin-pending">
                <p>{t('admin-pending')}</p>
                <button type="button" disabled={busy()} onClick={() => setConfirm(true)}>
                  {t('admin-review')}
                </button>
              </div>
            </Show>
            <Show when={category() !== 'tournaments'}>
              <ul class="admin-rows">
                <For each={data()?.rows}>
                  {(row) => (
                    <li>
                      <strong>
                        {String(
                          row.login ??
                            row.name ??
                            row.phrase ??
                            row.body ??
                            row.action ??
                            row.kind ??
                            '',
                        )}
                      </strong>
                      <dl>
                        <For
                          each={Object.entries(row).filter(
                            ([key]) =>
                              !['details', 'context', 'id', 'description', 'motd'].includes(key),
                          )}
                        >
                          {([key, value]) => (
                            <>
                              <dt>
                                {hasTranslation(`admin-field-${key}`)
                                  ? t(`admin-field-${key}`)
                                  : key}
                              </dt>
                              <dd>{String(value ?? '—')}</dd>
                            </>
                          )}
                        </For>
                      </dl>
                      <Show when={row.context || row.details}>
                        <details>
                          <summary>{t('admin-evidence')}</summary>
                          <pre>{JSON.stringify(row.context ?? row.details, null, 2)}</pre>
                        </details>
                      </Show>
                      <Show
                        when={[
                          'players',
                          'clans',
                          'filters',
                          'announcements',
                          'reports',
                          'signals',
                        ].includes(category())}
                      >
                        <button
                          type="button"
                          disabled={busy() || !!pending()}
                          onClick={() => setSelected(row)}
                        >
                          {t('admin-select')}
                        </button>
                      </Show>
                    </li>
                  )}
                </For>
              </ul>
              <div class="admin-pagination">
                <button
                  type="button"
                  disabled={page() === 0 || busy() || !!pending()}
                  onClick={() => setPage((p) => p - 1)}
                >
                  {t('admin-previous')}
                </button>
                <span>{page() + 1}</span>
                <button
                  type="button"
                  disabled={!data()?.has_more || busy() || !!pending()}
                  onClick={() => setPage((p) => p + 1)}
                >
                  {t('admin-next')}
                </button>
              </div>
            </Show>
            <fieldset disabled={busy() || !!pending()}>
              <legend>{t('admin-action')}</legend>
              <Show when={category() === 'players' && selected()}>
                {(row) => (
                  <form
                    class="admin-form"
                    onSubmit={(e) =>
                      form(e, (f) => {
                        const user = String(row().id),
                          action = playerAction();
                        if (action === 'role') return { action, user, role: string(f, 'role') };
                        if (action === 'mute')
                          return { action, user, seconds: number(f, 'minutes') * 60 };
                        if (action === 'ban')
                          return {
                            action,
                            user,
                            seconds: f.get('permanent') ? null : number(f, 'minutes') * 60,
                          };
                        return { action: action as 'warn' | 'unban' | 'unmute', user };
                      })
                    }
                  >
                    <h2>{row().login}</h2>
                    <label>
                      {t('admin-action')}
                      <select
                        value={playerAction()}
                        onChange={(e) => setPlayerAction(e.currentTarget.value)}
                      >
                        <For
                          each={[
                            'warn',
                            'ban',
                            'unban',
                            'mute',
                            'unmute',
                            ...(admin() ? ['role'] : []),
                          ]}
                        >
                          {(a) => <option value={a}>{t(`admin-action-${a}`)}</option>}
                        </For>
                      </select>
                    </label>
                    <Show when={['ban', 'mute'].includes(playerAction())}>
                      <label>
                        {t('admin-duration')}
                        <input
                          name="minutes"
                          type="number"
                          min="1"
                          max={admin() ? 525600 : playerAction() === 'ban' ? 10080 : 4320}
                          value="60"
                          required
                        />
                      </label>
                    </Show>
                    <Show when={playerAction() === 'ban' && admin()}>
                      <label class="admin-check">
                        <input name="permanent" type="checkbox" />
                        {t('admin-permanent')}
                      </label>
                    </Show>
                    <Show when={playerAction() === 'role'}>
                      <label>
                        {t('admin-field-role')}
                        <select name="role">
                          <For each={['player', 'mentor', 'moderator']}>
                            {(r) => <option value={r}>{t(`admin-role-${r}`)}</option>}
                          </For>
                        </select>
                      </label>
                    </Show>
                    <button type="submit">{t('admin-review')}</button>
                  </form>
                )}
              </Show>
              <Show when={category() === 'reports' && selected()?.status === 'open'}>
                <p>{selected()?.reason as string}</p>
                <button
                  type="button"
                  onClick={() =>
                    propose({ action: 'resolve', report: String(selected()?.id), accepted: true })
                  }
                >
                  {t('admin-accept')}
                </button>
                <button
                  type="button"
                  onClick={() =>
                    propose({ action: 'resolve', report: String(selected()?.id), accepted: false })
                  }
                >
                  {t('admin-reject')}
                </button>
              </Show>
              <Show when={category() === 'signals' && selected() && !selected()?.reviewed}>
                <p>{t('admin-signal-help')}</p>
                <button
                  type="button"
                  onClick={() => propose({ action: 'review_signal', id: String(selected()?.id) })}
                >
                  {t('admin-mark-reviewed')}
                </button>
              </Show>
              <Show when={category() === 'clans' && selected()}>
                {(row) => (
                  <form
                    class="admin-form"
                    onSubmit={(e) =>
                      form(e, (f) => ({
                        action: 'clan_edit',
                        clan: String(row().id),
                        description: string(f, 'description'),
                        motd: string(f, 'motd'),
                      }))
                    }
                  >
                    <h2>{row().name}</h2>
                    <label>
                      {t('admin-field-description')}
                      <textarea name="description" maxLength={1000} value={row().description} />
                    </label>
                    <label>
                      {t('admin-field-motd')}
                      <textarea name="motd" maxLength={300} value={row().motd} />
                    </label>
                    <button type="submit">{t('admin-review')}</button>
                  </form>
                )}
              </Show>
              <Show when={category() === 'filters' && admin()}>
                <form
                  class="admin-form"
                  onSubmit={(e) =>
                    form(e, (f) => ({
                      action: 'filter',
                      id: selected()?.id ?? crypto.randomUUID(),
                      phrase: string(f, 'phrase'),
                      enabled: enabled(f),
                    }))
                  }
                >
                  <label>
                    {t('admin-field-phrase')}
                    <input
                      name="phrase"
                      minLength={2}
                      maxLength={80}
                      required
                      value={selected()?.phrase ?? ''}
                    />
                  </label>
                  <label class="admin-check">
                    <input type="checkbox" name="enabled" checked={selected()?.enabled ?? true} />
                    {t('admin-enabled')}
                  </label>
                  <button type="submit">{t('admin-review')}</button>
                  <button type="button" onClick={() => setSelected(undefined)}>
                    {t('admin-new')}
                  </button>
                </form>
              </Show>
              <Show when={category() === 'announcements' && admin()}>
                <form
                  class="admin-form"
                  onSubmit={(e) =>
                    form(e, (f) => ({
                      action: 'announcement',
                      id: selected()?.id ?? crypto.randomUUID(),
                      body: string(f, 'body'),
                      seconds: number(f, 'minutes') * 60,
                      enabled: enabled(f),
                    }))
                  }
                >
                  <label>
                    {t('admin-field-body')}
                    <textarea
                      name="body"
                      maxLength={1000}
                      required
                      value={selected()?.body ?? ''}
                    />
                  </label>
                  <label>
                    {t('admin-duration')}
                    <input type="number" name="minutes" min="1" max="43200" value="60" required />
                  </label>
                  <label class="admin-check">
                    <input type="checkbox" name="enabled" checked={selected()?.enabled ?? true} />
                    {t('admin-enabled')}
                  </label>
                  <button type="submit">{t('admin-review')}</button>
                  <button type="button" onClick={() => setSelected(undefined)}>
                    {t('admin-new')}
                  </button>
                </form>
              </Show>
              <Show when={category() === 'control' && admin()}>
                <form
                  class="admin-form"
                  onSubmit={(e) =>
                    form(e, (f) => ({
                      action: 'maintenance',
                      enabled: enabled(f),
                      notice: string(f, 'notice'),
                    }))
                  }
                >
                  <label class="admin-check">
                    <input
                      name="enabled"
                      type="checkbox"
                      checked={Boolean(data()?.rows[0]?.maintenance)}
                    />
                    {t('admin-maintenance')}
                  </label>
                  <label>
                    {t('admin-field-notice')}
                    <textarea
                      name="notice"
                      maxLength={1000}
                      value={String(data()?.rows[0]?.notice ?? '')}
                    />
                  </label>
                  <button type="submit">{t('admin-review')}</button>
                </form>
              </Show>
              <Show when={category() === 'tournaments' && admin()}>
                <p>{t('admin-prize-help')}</p>
                <form
                  class="admin-form"
                  onSubmit={(e) =>
                    form(e, (f) => {
                      const kind = rewardKind();
                      const reward: Reward =
                        kind === 'ducats'
                          ? { kind, amount: number(f, 'prize') }
                          : kind === 'avatar'
                            ? { kind, template: number(f, 'template') }
                            : kind === 'card'
                              ? { kind, card: number(f, 'card') }
                              : { kind: 'booster', booster: string(f, 'booster') };
                      return {
                        action: 'system_tournament',
                        id: crypto.randomUUID(),
                        name: string(f, 'name'),
                        reward,
                        terms: {
                          teams: number(f, 'teams'),
                          team_size: number(f, 'team_size'),
                          fee: 0,
                          max_games: number(f, 'games'),
                          min_level: number(f, 'min_level'),
                          max_level: number(f, 'max_level'),
                          tiers: [0, 1, 2, 3],
                          only_clan: null,
                          allowed_clans: string(f, 'clans')
                            .split(',')
                            .map((v) => v.trim())
                            .filter(Boolean),
                        },
                      };
                    })
                  }
                >
                  <label>
                    {t('admin-tournament-name')}
                    <input name="name" required minLength={3} maxLength={60} />
                  </label>
                  <label>
                    {t('admin-teams')}
                    <input name="teams" type="number" min="2" max="64" value="2" required />
                  </label>
                  <label>
                    {t('admin-team-size')}
                    <input name="team_size" type="number" min="1" max="16" value="1" required />
                  </label>
                  <label>
                    {t('admin-allowed-clans')}
                    <input name="clans" maxLength={2368} />
                  </label>
                  <label>
                    {t('admin-games')}
                    <input name="games" type="number" min="1" max="9" value="1" required />
                  </label>
                  <label>
                    {t('admin-min-level')}
                    <input name="min_level" type="number" min="1" max="20" value="1" required />
                  </label>
                  <label>
                    {t('admin-max-level')}
                    <input name="max_level" type="number" min="1" max="20" value="20" required />
                  </label>
                  <label>
                    {t('admin-prize')}
                    <select
                      value={rewardKind()}
                      onChange={(e) => setRewardKind(e.currentTarget.value)}
                    >
                      <For each={['ducats', 'avatar', 'card', 'booster']}>
                        {(kind) => <option value={kind}>{t(`admin-prize-${kind}`)}</option>}
                      </For>
                    </select>
                  </label>
                  <Show when={rewardKind() === 'ducats'}>
                    <label>
                      {t('admin-prize-ducats')}
                      <input
                        name="prize"
                        type="number"
                        min="1"
                        max="1000000"
                        value="100"
                        required
                      />
                    </label>
                  </Show>
                  <Show when={rewardKind() === 'avatar'}>
                    <label>
                      {t('admin-prize-avatar')}
                      <select name="template">
                        <For
                          each={content.avatars.filter(
                            (a) => !['junk', 'shade'].includes(a.rarity),
                          )}
                        >
                          {(a) => <option value={a.id}>{t(fluentKey(a.key))}</option>}
                        </For>
                      </select>
                    </label>
                  </Show>
                  <Show when={rewardKind() === 'card'}>
                    <label>
                      {t('admin-prize-card')}
                      <select name="card">
                        <CardOptions />
                      </select>
                    </label>
                  </Show>
                  <Show when={rewardKind() === 'booster'}>
                    <label>
                      {t('admin-prize-booster')}
                      <select name="booster">
                        <For each={['basic', 'bronze', 'silver', 'gold']}>
                          {(b) => <option value={b}>{b}</option>}
                        </For>
                      </select>
                    </label>
                  </Show>
                  <button type="submit">{t('admin-review')}</button>
                </form>
                <form
                  class="admin-form"
                  onSubmit={(e) =>
                    form(e, (f) => ({ action: 'system_cancel', id: string(f, 'id') }))
                  }
                >
                  <label>
                    {t('admin-cancel-tournament')}
                    <input name="id" required pattern="[0-9a-fA-F-]{36}" maxLength={36} />
                  </label>
                  <button type="submit">{t('admin-review')}</button>
                </form>
              </Show>
            </fieldset>
          </Show>
        </Show>
        <Show when={confirm() && pending()}>
          <Modal
            title={t('admin-confirm')}
            close={() => {
              if (!busy()) setConfirm(false);
            }}
          >
            <h3>{t(`admin-action-${pending()?.operation.action}`)}</h3>
            <p>{String(selected()?.login ?? selected()?.name ?? '')}</p>
            <p>{pending()?.reason}</p>
            <dl class="admin-decision">
              <For each={decisionDetails()}>
                {([key, value]) => (
                  <>
                    <dt>{t(key)}</dt>
                    <dd>{value || '—'}</dd>
                  </>
                )}
              </For>
            </dl>
            <p>{t('admin-confirm-help')}</p>
            <button type="button" class="primary" disabled={busy()} onClick={() => void apply()}>
              {t('admin-apply')}
            </button>
            <button
              type="button"
              disabled={busy()}
              onClick={() => {
                setConfirm(false);
                setPending(undefined);
              }}
              hidden={attempted()}
            >
              {t('cancel')}
            </button>
          </Modal>
        </Show>
      </Show>
    </section>
  );
}
