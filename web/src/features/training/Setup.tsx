import { A } from '@solidjs/router';
import { createSignal, For, Show } from 'solid-js';
import { account, enterGuest } from '../../app/account';
import { avatars, content, fluentKey, icon, portrait } from '../../app/content';
import { readPreference, writePreference } from '../../app/preferences';
import { t } from '../../i18n';
import styles from './Training.module.css';

const profiles = ['novice', 'apprentice', 'tactician', 'archmage'];
export function TrainingSetup() {
  const [selected, setSelected] = createSignal(Number(readPreference('avatar', '1')));
  const [bot, setBot] = createSignal(readPreference('bot', 'novice'));
  const avatarId = () => (avatars.has(selected()) ? selected() : 1);
  const profile = () => (profiles.includes(bot()) ? bot() : 'novice');
  function selectAvatar(id: number) {
    setSelected(id);
    writePreference('avatar', String(id));
  }
  function selectBot(id: string) {
    setBot(id);
    writePreference('bot', id);
  }
  return (
    <div class={styles.setup}>
      <section class={styles.hero}>
        <div>
          <p class="eyebrow">{t('eyebrow')}</p>
          <h1>{t('home-title')}</h1>
          <p class={styles.intro}>{t('home-intro')}</p>
        </div>
        <div class={styles.sigil} aria-hidden="true">
          <img src={icon('school-arcane')} alt="" />
          <span>F · M</span>
        </div>
      </section>
      <aside class="guest-entry">
        <Show
          when={account()?.kind === 'guest'}
          fallback={
            <>
              <p>{t('auth-guest-short')}</p>
              <button type="button" onClick={enterGuest}>
                {t('auth-play-guest')}
              </button>
              <A href="/account">{t('auth-account')}</A>
            </>
          }
        >
          <p role="status">{t('auth-guest-active')}</p>
          <A href="/account">{t('auth-account')}</A>
        </Show>
      </aside>
      <div class={styles.setupGrid}>
        <section class={styles.avatarSection} aria-labelledby="avatars-title">
          <div class={styles.sectionHeading}>
            <h2 id="avatars-title">
              <span>01</span>
              {t('choose-avatar')}
            </h2>
            <p>{t('starter-deck')}</p>
          </div>
          <div class={styles.avatars}>
            <For each={content.avatars}>
              {(avatar) => (
                <button
                  type="button"
                  class={styles.avatarChoice}
                  aria-pressed={avatarId() === avatar.id}
                  onClick={() => selectAvatar(avatar.id)}
                >
                  <img
                    class={styles.avatarPortrait}
                    src={portrait(avatar.key)}
                    alt=""
                    width="120"
                    height="120"
                  />
                  <strong>{t(fluentKey(avatar.key))}</strong>
                  <span>{t(`school-${avatar.starter_for}-name`)}</span>
                  <span class={styles.choiceStats}>
                    {avatar.base.hp} {t('hp')} · {avatar.base.stamina} W
                  </span>
                </button>
              )}
            </For>
          </div>
        </section>
        <section class={styles.botSection} aria-labelledby="bots-title">
          <div class={styles.sectionHeading}>
            <h2 id="bots-title">
              <span>02</span>
              {t('choose-bot')}
            </h2>
          </div>
          <div class={styles.bots}>
            <For each={profiles}>
              {(id, index) => (
                <button
                  type="button"
                  class={styles.botChoice}
                  aria-pressed={profile() === id}
                  onClick={() => selectBot(id)}
                >
                  <span class={styles.botLevel}>L{index()}</span>
                  <span>
                    <strong>{t(id)}</strong>
                    <small>{t(`${id}-desc`)}</small>
                  </span>
                  <span class={styles.radio} aria-hidden="true" />
                </button>
              )}
            </For>
          </div>
          <A class="button primary" href={`/duel?avatar=${avatarId()}&bot=${profile()}`}>
            {t('start')} <span aria-hidden="true">↗</span>
          </A>
          <A class="button secondary" href={`/duel?avatar=${avatarId()}&bot=${profile()}&demo=1`}>
            {t('demo')}
          </A>
          <p class={styles.localNote}>{t('local-note')}</p>
        </section>
      </div>
      <section class={styles.guide}>
        <h2>{t('how-title')}</h2>
        <For each={['how-attack', 'how-defend', 'how-card']}>
          {(key, index) => (
            <p>
              <span>0{index() + 1}</span>
              {t(key)}
            </p>
          )}
        </For>
      </section>
    </div>
  );
}
