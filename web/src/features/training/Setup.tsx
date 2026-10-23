import { A } from '@solidjs/router';
import { createSignal, For, Show } from 'solid-js';
import { account, enterGuest } from '../../app/account';
import { fluentKey, isPlayableAvatar, playableAvatars, portrait } from '../../app/content';
import { readPreference, writePreference } from '../../app/preferences';
import { t } from '../../i18n';
import styles from './Training.module.css';

const profiles = ['novice', 'apprentice', 'tactician', 'archmage'];
// The welcome card is painted with the training grounds' scene (art/paintings/scenes, global.css).
const [painting] = Object.values(
  import.meta.glob<string>('../../assets/paintings/scenes/home.avif', {
    eager: true,
    query: '?url',
    import: 'default',
  }),
);
export function TrainingSetup() {
  const [selected, setSelected] = createSignal(Number(readPreference('avatar', '1')));
  const [bot, setBot] = createSignal(readPreference('bot', 'novice'));
  const avatarId = () => (isPlayableAvatar(selected()) ? selected() : 1);
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
      <section
        class={`${styles.hero} painted`}
        style={{ '--scene': painting ? `url("${painting}")` : 'none' }}
      >
        <div>
          <p class="eyebrow">{t('eyebrow')}</p>
          <h1>{t('home-title')}</h1>
          <ol class={styles.rules}>
            <For each={['how-attack', 'how-defend', 'how-card']}>{(key) => <li>{t(key)}</li>}</For>
          </ol>
        </div>
        {/* The way into a duel comes first; the avatar and bot below are remembered between visits. */}
        <div class={styles.heroActions}>
          <A class="button primary" href={`/duel?avatar=${avatarId()}&bot=${profile()}`}>
            {t('start')}
          </A>
          <A href={`/duel?avatar=${avatarId()}&bot=${profile()}&demo=1`}>{t('demo')}</A>
          <p class={styles.localNote}>{t('local-note')}</p>
        </div>
      </section>
      {/* A signed-in player needs no invitation to play as a guest. */}
      <Show when={account()?.kind !== 'account'}>
        <aside class="guest-entry">
          <Show
            when={account()?.kind === 'guest'}
            fallback={
              <>
                <p>{t('auth-guest-short')}</p>
                <button type="button" onClick={enterGuest}>
                  {t('auth-play-guest')}
                </button>
              </>
            }
          >
            <p role="status">{t('auth-guest-active')}</p>
          </Show>
        </aside>
      </Show>
      <div class={styles.setupGrid}>
        <section class={styles.avatarSection} aria-labelledby="avatars-title">
          <div class={styles.sectionHeading}>
            <h2 id="avatars-title">
              <span>01</span>
              {t('choose-avatar')}
            </h2>
            <p>{t('starter-deck')}</p>
          </div>
          {/* Portraits load lazily: the painted welcome card is the largest paint and goes first. */}
          <div class={styles.avatars}>
            <For each={playableAvatars}>
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
                    loading="lazy"
                  />
                  <strong>{t(fluentKey(avatar.key))}</strong>
                  <span>
                    {t(`school-${avatar.starter_for ?? Object.keys(avatar.schools)[0]}-name`)}
                  </span>
                  <span class={styles.choiceStats}>
                    {t('hp')} {avatar.base.hp} · {t('stamina')} {avatar.base.stamina}
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
        </section>
      </div>
    </div>
  );
}
