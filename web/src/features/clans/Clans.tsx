import { A, useBeforeLeave } from '@solidjs/router';
import { createEffect, createMemo, createSignal, For, lazy, onCleanup, Show } from 'solid-js';
import { ApiError, account, accountEpoch, api, restoreAccount } from '../../app/account';
import { content } from '../../app/content';
import { Modal } from '../../components/Modal';
import { Scene } from '../../components/Scene';
import { addTranslations, hasTranslation, language, t } from '../../i18n';
import en from './en.ftl?raw';
import pl from './pl.ftl?raw';
import type { Member, Mutation, Profile, Snapshot } from './types';
import '../online.css';

addTranslations({ pl, en });
// The clans' painting (art/paintings/scenes), the same as in the lobby's clan room.
const [painting] = Object.values(
  import.meta.glob<string>('../../assets/paintings/scenes/clan.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);
const Report = lazy(() => import('../moderation/Report').then((m) => ({ default: m.Report })));

export function Clans() {
  const [data, setData] = createSignal<Snapshot>();
  const [selected, setSelected] = createSignal<string>();
  const [search, setSearch] = createSignal('');
  const [page, setPage] = createSignal(0);
  const [name, setName] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [notice, setNotice] = createSignal('');
  const [confirm, setConfirm] = createSignal<Mutation>();
  const [reported, setReported] = createSignal<{ target: string; label: string }>();
  const [description, setDescription] = createSignal('');
  const [motd, setMotd] = createSignal('');
  const [www, setWww] = createSignal('');
  const [email, setEmail] = createSignal('');
  const [visible, setVisible] = createSignal(25);
  let creationId = crypto.randomUUID();
  let identity = '';
  let editIdentity = '';
  let disposed = false;
  let request = 0;
  const profile = () => data()?.profile;
  const userId = () => {
    const current = account();
    return current?.kind === 'account' ? current.profile.id : '';
  };
  const own = () => Boolean(profile() && profile()?.id === data()?.own);
  const role = () => (own() ? data()?.own_role : undefined);
  const officer = () => role() === 'master' || role() === 'vice';
  const editable = () => officer() || role() === 'scribe';
  const dirty = createMemo(
    () =>
      editable() &&
      Boolean(profile()) &&
      (description() !== profile()?.description ||
        motd() !== profile()?.motd ||
        www() !== profile()?.www ||
        email() !== profile()?.email),
  );
  const version = (p: Profile) => ({ clan: p.id, revision: p.revision });
  function failure(reason: unknown) {
    const code = reason instanceof ApiError ? reason.code : 'UNAVAILABLE';
    setError(t(hasTranslation(`clan-error-${code}`) ? `clan-error-${code}` : 'clan-error-default'));
  }
  async function load() {
    if (account()?.kind !== 'account') return;
    const epoch = accountEpoch();
    const current = ++request;
    setLoading(true);
    setError('');
    const path = (clan?: string) => {
      const query = new URLSearchParams({ search: search(), page: String(page()) });
      if (clan) query.set('clan', clan);
      return `/api/clans?${query}`;
    };
    try {
      let snapshot: Snapshot;
      try {
        snapshot = await api<Snapshot>(path(selected()));
      } catch (reason) {
        if (!(reason instanceof ApiError && reason.code === 'NOT_FOUND' && selected()))
          throw reason;
        if (disposed || epoch !== accountEpoch() || current !== request) return;
        setSelected(undefined);
        snapshot = await api<Snapshot>(path());
      }
      if (snapshot.content_version !== content.version) throw new ApiError('CONTENT_MISMATCH');
      if (!disposed && epoch === accountEpoch() && current === request) setData(snapshot);
    } catch (reason) {
      if (!disposed && epoch === accountEpoch() && current === request) failure(reason);
    } finally {
      if (!disposed && epoch === accountEpoch() && current === request) setLoading(false);
    }
  }
  createEffect(() => {
    const current = account();
    const next = `${current?.kind}:${accountEpoch()}`;
    if (next === identity) return;
    identity = next;
    request++;
    setData(undefined);
    setSelected(undefined);
    setName('');
    setSearch('');
    setPage(0);
    creationId = crypto.randomUUID();
    setNotice('');
    setConfirm(undefined);
    if (current?.kind === 'account') void load();
    else if (!current && navigator.onLine) void restoreAccount();
  });
  createEffect(() => {
    const p = profile();
    const key = `${p?.id}:${p?.revision}:${role()}`;
    if (key === editIdentity) return;
    editIdentity = key;
    setDescription(p?.description ?? '');
    setMotd(p?.motd ?? '');
    setWww(p?.www ?? '');
    setEmail(p?.email ?? '');
    setVisible(25);
  });
  onCleanup(() => {
    disposed = true;
    request++;
  });
  useBeforeLeave((event) => {
    if (dirty() && !window.confirm(t('clan-unsaved'))) event.preventDefault();
  });
  function select(clan?: string) {
    if (dirty() && !window.confirm(t('clan-unsaved'))) return;
    setSelected(clan);
    void load();
  }
  async function mutate(mutation: Mutation, confirmed = false) {
    if (account()?.kind !== 'account' || loading()) return;
    if (mutation.action !== 'edit' && !confirmed && dirty() && !window.confirm(t('clan-unsaved')))
      return;
    const epoch = accountEpoch();
    setLoading(true);
    setError('');
    setNotice('');
    try {
      await api('/api/clans', mutation);
      if (disposed || epoch !== accountEpoch()) return;
      if (mutation.action === 'create') {
        setSelected(mutation.id);
        setName('');
        creationId = crypto.randomUUID();
      }
      if (mutation.action === 'leave' || mutation.action === 'disband') setSelected(undefined);
      setNotice(t('clan-saved'));
      await load();
    } catch (reason) {
      if (!disposed && epoch === accountEpoch()) {
        await load();
        failure(reason);
      }
    } finally {
      if (!disposed && epoch === accountEpoch()) setLoading(false);
    }
  }
  function confirming(mutation: Mutation) {
    setConfirm(mutation);
  }
  const confirmationTarget = () => {
    const mutation = confirm();
    return mutation && 'user' in mutation
      ? (profile()?.roster.find((m) => m.user === mutation.user)?.login ?? '')
      : (profile()?.name ?? '');
  };
  return (
    <section class="clans-page">
      <Scene src={painting}>
        <h1 class="plaque">{t('clans')}</h1>
        <p>
          <A href="/lobby">{t('lobby')}</A>
        </p>
      </Scene>
      <Show
        when={account()?.kind === 'account'}
        fallback={
          <p>
            {t('clans-gate')} <A href="/account">{t('auth-account')}</A>
          </p>
        }
      >
        <div class="clans-toolbar">
          <button type="button" disabled={loading()} onClick={() => void load()}>
            {t('clan-refresh')}
          </button>
          <Show when={data()?.own}>
            <button type="button" disabled={loading()} onClick={() => select()}>
              {t('clan-own')}
            </button>
          </Show>
        </div>
        <Show when={error()}>
          <p role="alert">{error()}</p>
        </Show>
        <Show when={notice()}>
          <p role="status">{notice()}</p>
        </Show>
        <Show when={data()}>
          {(snapshot) => (
            <>
              <Show when={!snapshot().own}>
                <form
                  class="clan-panel"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void mutate({ action: 'create', id: creationId, name: name() });
                  }}
                >
                  <h2>{t('clan-create')}</h2>
                  <p>{t('clan-founding-help', { min: snapshot().limits.min_members })}</p>
                  <label>
                    {t('clan-name')}
                    <input
                      required
                      minLength={3}
                      maxLength={40}
                      value={name()}
                      onInput={(e) => setName(e.currentTarget.value)}
                    />
                  </label>
                  <button type="submit" disabled={loading() || name().trim().length < 3}>
                    {t('clan-create')}
                  </button>
                </form>
                <Show when={snapshot().applications.length}>
                  <section class="clan-panel">
                    <h2>{t('clan-applications')}</h2>
                    <ul>
                      <For each={snapshot().applications}>
                        {(application) => (
                          <li>
                            <button
                              type="button"
                              class="quiet"
                              disabled={loading()}
                              onClick={() => select(application.clan)}
                            >
                              {application.name}
                            </button>
                            <button
                              type="button"
                              disabled={loading()}
                              onClick={() =>
                                void mutate({ action: 'withdraw', clan: application.clan })
                              }
                            >
                              {t('clan-withdraw')}
                            </button>
                          </li>
                        )}
                      </For>
                    </ul>
                  </section>
                </Show>
              </Show>
              <div class="clan-layout">
                <section class="clan-panel">
                  <h2>{t('clan-catalog')}</h2>
                  <form
                    class="clans-toolbar"
                    onSubmit={(e) => {
                      e.preventDefault();
                      setPage(0);
                      void load();
                    }}
                  >
                    <label>
                      {t('clan-search')}
                      <input
                        type="search"
                        maxLength={40}
                        value={search()}
                        onInput={(e) => setSearch(e.currentTarget.value)}
                      />
                    </label>
                    <button type="submit" disabled={loading()}>
                      {t('clan-search')}
                    </button>
                  </form>
                  <ul class="clan-catalog">
                    <For each={snapshot().clans}>
                      {(clan) => (
                        <li>
                          <button
                            type="button"
                            class="quiet"
                            disabled={loading()}
                            onClick={() => select(clan.id)}
                          >
                            {clan.name}
                          </button>
                          <span>
                            {t(`clan-status-${clan.status}`)} · {clan.members}/
                            {snapshot().limits.max_members}
                          </span>
                          <Show when={clan.rank !== null}>
                            <span>
                              {t('clan-ranking', { rank: clan.rank ?? 0, score: clan.score })}
                            </span>
                          </Show>
                        </li>
                      )}
                    </For>
                  </ul>
                  <Show when={!snapshot().clans.length}>
                    <p>{t('clan-empty')}</p>
                  </Show>
                  <div class="clans-toolbar">
                    <button
                      type="button"
                      disabled={loading() || page() === 0}
                      onClick={() => {
                        setPage((v) => v - 1);
                        void load();
                      }}
                    >
                      {t('clan-previous')}
                    </button>
                    <span>{page() + 1}</span>
                    <button
                      type="button"
                      disabled={loading() || !snapshot().has_more}
                      onClick={() => {
                        setPage((v) => v + 1);
                        void load();
                      }}
                    >
                      {t('clan-next')}
                    </button>
                  </div>
                </section>
                <Show when={profile()}>
                  {(p) => (
                    <section class="clan-panel" aria-label={t('clan-profile')}>
                      <h2>{p().name}</h2>
                      <button
                        type="button"
                        onClick={() => setReported({ target: p().id, label: p().name })}
                      >
                        {t('clan-report')}
                      </button>
                      <Show when={p().status === 'founding'}>
                        <p>{t('clan-founding-help', { min: snapshot().limits.min_members })}</p>
                      </Show>
                      <p>
                        {t(`clan-status-${p().status}`)} ·{' '}
                        {t('clan-member-count', { count: p().members })}
                      </p>
                      <Show when={p().rank !== null}>
                        <p>{t('clan-ranking', { rank: p().rank ?? 0, score: p().score })}</p>
                      </Show>
                      <p>{t('clan-total-ducats', { value: p().ducats })}</p>
                      <Show when={p().founded_ms}>
                        <p>
                          {t('clan-founded', {
                            date: new Date(p().founded_ms ?? 0).toLocaleDateString(language()),
                          })}
                        </p>
                      </Show>
                      <p class="clan-prose">{p().description}</p>
                      <Show when={p().www}>
                        <p>
                          <a href={p().www} target="_blank" rel="noopener noreferrer">
                            {p().www}
                          </a>
                        </p>
                      </Show>
                      <Show when={p().email}>
                        <p>{p().email}</p>
                      </Show>
                      <Show when={own()}>
                        <p>{t('clan-your-role', { role: t(`clan-role-${role()}`) })}</p>
                        <Show when={p().motd}>
                          <p class="clan-prose">
                            {t('clan-motd')}: {p().motd}
                          </p>
                        </Show>
                        <button
                          type="button"
                          disabled={loading()}
                          onClick={() => confirming({ action: 'leave', ...version(p()) })}
                        >
                          {t('clan-leave')}
                        </button>
                        <Show when={role() === 'master'}>
                          <button
                            type="button"
                            class="danger"
                            disabled={loading()}
                            onClick={() => confirming({ action: 'disband', ...version(p()) })}
                          >
                            {t('clan-disband')}
                          </button>
                        </Show>
                      </Show>
                      <Show
                        when={
                          !snapshot().own && !snapshot().applications.some((a) => a.clan === p().id)
                        }
                      >
                        <button
                          type="button"
                          disabled={loading()}
                          onClick={() => void mutate({ action: 'apply', clan: p().id })}
                        >
                          {t('clan-apply')}
                        </button>
                      </Show>
                      <Show when={officer() && p().applicants.length}>
                        <h3>{t('clan-applicants')}</h3>
                        <ul>
                          <For each={p().applicants}>
                            {(a) => (
                              <li class="clans-toolbar">
                                <span>{a.login}</span>
                                <button
                                  type="button"
                                  disabled={loading()}
                                  onClick={() =>
                                    void mutate({ action: 'accept', user: a.user, ...version(p()) })
                                  }
                                >
                                  {t('clan-accept', { name: a.login })}
                                </button>
                                <button
                                  type="button"
                                  disabled={loading()}
                                  onClick={() =>
                                    void mutate({ action: 'reject', user: a.user, ...version(p()) })
                                  }
                                >
                                  {t('clan-reject', { name: a.login })}
                                </button>
                              </li>
                            )}
                          </For>
                        </ul>
                      </Show>
                      <Show when={editable()}>
                        <form
                          class="clan-editor"
                          onSubmit={(e) => {
                            e.preventDefault();
                            void mutate({
                              action: 'edit',
                              ...version(p()),
                              description: description(),
                              motd: motd(),
                              www: www(),
                              email: email(),
                            });
                          }}
                        >
                          <h3>{t('clan-edit')}</h3>
                          <label>
                            {t('clan-description')}
                            <textarea
                              maxLength={1000}
                              rows={4}
                              value={description()}
                              onInput={(e) => setDescription(e.currentTarget.value)}
                            />
                          </label>
                          <label>
                            {t('clan-motd')}
                            <textarea
                              maxLength={300}
                              rows={3}
                              value={motd()}
                              onInput={(e) => setMotd(e.currentTarget.value)}
                            />
                          </label>
                          <p>{t('clan-motd-private')}</p>
                          <label>
                            {t('clan-www')}
                            <input
                              type="url"
                              maxLength={200}
                              disabled={role() === 'scribe'}
                              value={www()}
                              onInput={(e) => setWww(e.currentTarget.value)}
                            />
                          </label>
                          <label>
                            {t('clan-email')}
                            <input
                              type="email"
                              maxLength={254}
                              disabled={role() === 'scribe'}
                              value={email()}
                              onInput={(e) => setEmail(e.currentTarget.value)}
                            />
                          </label>
                          <button type="submit" disabled={loading() || !dirty()}>
                            {t('clan-save')}
                          </button>
                        </form>
                      </Show>
                      <h3>{t('clan-members')}</h3>
                      <ul class="clan-roster">
                        <For each={p().roster.slice(0, visible())}>
                          {(member) => (
                            <li>
                              <strong>{member.login}</strong> · {t(`clan-role-${member.role}`)} ·{' '}
                              {member.rating}
                              <Show when={member.title}>
                                <p>{member.title}</p>
                              </Show>
                              <Show
                                when={
                                  officer() && (member.role !== 'master' || role() === 'master')
                                }
                              >
                                <MemberActions
                                  member={member}
                                  master={role() === 'master'}
                                  self={member.user === userId()}
                                  disabled={loading()}
                                  mutate={(mutation) =>
                                    void mutate({ ...mutation, ...version(p()) })
                                  }
                                  confirm={(action) =>
                                    confirming({ action, user: member.user, ...version(p()) })
                                  }
                                />
                              </Show>
                            </li>
                          )}
                        </For>
                      </ul>
                      <Show when={visible() < p().roster.length}>
                        <button type="button" onClick={() => setVisible((v) => v + 25)}>
                          {t('clan-more')}
                        </button>
                      </Show>
                    </section>
                  )}
                </Show>
              </div>
            </>
          )}
        </Show>
      </Show>
      <Show when={confirm()}>
        {(mutation) => (
          <Modal title={t('clan-confirm-title')} close={() => setConfirm(undefined)}>
            <p>{t(`clan-confirm-${mutation().action}`, { name: confirmationTarget() })}</p>
            <Show when={dirty()}>
              <p>{t('clan-discard-warning')}</p>
            </Show>
            <button
              type="button"
              class="danger"
              disabled={loading()}
              onClick={() => {
                const action = mutation();
                setConfirm(undefined);
                void mutate(action, true);
              }}
            >
              {t('clan-confirm')}
            </button>
          </Modal>
        )}
      </Show>
      <Show when={reported()}>
        {(value) => <Report kind="clan" {...value()} close={() => setReported(undefined)} />}
      </Show>
    </section>
  );
}

function MemberActions(props: {
  member: Member;
  master: boolean;
  self: boolean;
  disabled: boolean;
  mutate: (
    mutation:
      | { action: 'title'; user: string; title: string }
      | { action: 'role'; user: string; role: 'vice' | 'scribe' | 'member' },
  ) => void;
  confirm: (action: 'kick' | 'transfer') => void;
}) {
  const [title, setTitle] = createSignal(props.member.title);
  createEffect(() => setTitle(props.member.title));
  return (
    <div class="clan-member-actions">
      <Show when={props.member.role !== 'master'}>
        <label>
          {t('clan-role-for', { name: props.member.login })}
          <select
            aria-label={t('clan-role-for', { name: props.member.login })}
            value={props.member.role}
            disabled={props.disabled}
            onChange={(e) => {
              const role = e.currentTarget.value;
              e.currentTarget.value = props.member.role;
              if (role === 'vice' || role === 'scribe' || role === 'member')
                props.mutate({ action: 'role', user: props.member.user, role });
            }}
          >
            <For each={['vice', 'scribe', 'member']}>
              {(role) => <option value={role}>{t(`clan-role-${role}`)}</option>}
            </For>
          </select>
        </label>
      </Show>
      <form
        class="clans-toolbar"
        onSubmit={(e) => {
          e.preventDefault();
          props.mutate({ action: 'title', user: props.member.user, title: title() });
        }}
      >
        <label>
          {t('clan-title-for', { name: props.member.login })}
          <input maxLength={64} value={title()} onInput={(e) => setTitle(e.currentTarget.value)} />
        </label>
        <button type="submit" disabled={props.disabled || title() === props.member.title}>
          {t('clan-title-save')}
        </button>
      </form>
      <Show when={!props.self && props.member.role !== 'master'}>
        <button type="button" disabled={props.disabled} onClick={() => props.confirm('kick')}>
          {t('clan-kick', { name: props.member.login })}
        </button>
        <Show when={props.master}>
          <button type="button" disabled={props.disabled} onClick={() => props.confirm('transfer')}>
            {t('clan-transfer', { name: props.member.login })}
          </button>
        </Show>
      </Show>
    </div>
  );
}
