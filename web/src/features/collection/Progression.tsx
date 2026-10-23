import { For, Show } from 'solid-js';
import { cardArgs, cards, content, fluentKey, isPlayableAvatar } from '../../app/content';
import { Stats } from '../../components/Stats';
import { addTranslations, t } from '../../i18n';
import en from './progression.en.ftl?raw';
import pl from './progression.pl.ftl?raw';
import type { AvatarState, Mutation } from './state';

addTranslations({ pl, en });
export function Progression(props: {
  avatar: AvatarState;
  accountLevel: number;
  disabled: boolean;
  confirm: (mutation: Mutation) => void;
}) {
  const path = () => content.paths.find((p) => p.id === props.avatar.path_id);
  const required = () => (props.avatar.tier === 0 ? 12 : 18);
  const disabled = () =>
    props.disabled ||
    !isPlayableAvatar(props.avatar.template_id) ||
    props.avatar.level !== 20 ||
    props.accountLevel < required() ||
    (props.avatar.advancement?.trial_wins ?? 0) < 3;
  function reincarnate(choice: Extract<Mutation, { action: 'reincarnate' }>['choice']) {
    props.confirm({
      action: 'reincarnate',
      id: props.avatar.id,
      revision: props.avatar.revision,
      choice,
    });
  }
  return (
    <section class="collection-progression" aria-label={t('collection-progression')}>
      <h2>
        {t('collection-progression')} · {t(`collection-tier-${props.avatar.tier}`)}
      </h2>
      <p>
        {t('collection-permanent', {
          total: props.avatar.advancement?.permanent_points ?? 0,
          available: props.avatar.advancement?.permanent_unspent ?? 0,
        })}
      </p>
      <Show when={props.avatar.specialization}>
        {(spec) => (
          <p>
            {t('collection-specialization')}: {t(`collection-spec-${spec()}`)}
          </p>
        )}
      </Show>
      <Show when={cards.get(props.avatar.gift_id ?? 0)}>
        {(gift) => (
          <p>
            {t('collection-legacy-gift')}: {t(fluentKey(gift().key))} —{' '}
            {t(fluentKey(gift().key, 'text'), cardArgs(gift()))}
          </p>
        )}
      </Show>
      <Show when={props.avatar.tier < 2} fallback={<p>{t('collection-final-tier')}</p>}>
        <p>
          {t('collection-trial', {
            level: required(),
            wins: props.avatar.advancement?.trial_wins ?? 0,
          })}
        </p>
        <p>{t('collection-trial-help')}</p>
        <p>
          {t('collection-cycle', {
            xp: props.avatar.tier === 0 ? 6000 : 9000,
            bonus: (props.avatar.tier + 1) * 3,
          })}
        </p>
        <Show when={props.avatar.tier === 0}>
          <div class="collection-paths">
            <For
              each={content.paths.filter((p) => p.path.base_avatar === props.avatar.template_id)}
            >
              {(p) => (
                <article>
                  <h3>{t(fluentKey(p.key))}</h3>
                  <Stats
                    values={[
                      p.base.phys_atk,
                      p.base.phys_def,
                      p.base.mag_atk,
                      p.base.mag_def,
                      p.base.hp,
                      p.base.stamina,
                    ]}
                  />
                  <p>
                    {Object.entries(p.schools)
                      .map(([s, n]) => `${t(`school-${s}-name`)} ${n}`)
                      .join(' · ')}
                  </p>
                  <p>
                    {t('collection-specialization')}:{' '}
                    {p.path.specializations.map((s) => t(`collection-spec-${s}`)).join(' / ')}
                  </p>
                  <button
                    type="button"
                    disabled={disabled()}
                    onClick={() => reincarnate({ kind: 'path', id: p.id })}
                  >
                    {t('collection-choose-path', { name: t(fluentKey(p.key)) })}
                  </button>
                </article>
              )}
            </For>
          </div>
        </Show>
        <Show when={props.avatar.tier === 1 && path()}>
          {(p) => (
            <div class="collection-paths">
              <For each={p().path.specializations}>
                {(spec) => (
                  <article>
                    <h3>{t(`collection-spec-${spec}`)}</h3>
                    <p>{t(`collection-spec-${spec}-help`)}</p>
                    <button
                      type="button"
                      disabled={disabled()}
                      onClick={() => reincarnate({ kind: 'specialization', name: spec })}
                    >
                      {t('collection-choose-spec', { name: t(`collection-spec-${spec}`) })}
                    </button>
                  </article>
                )}
              </For>
            </div>
          )}
        </Show>
      </Show>
    </section>
  );
}

import '../../app/cardCatalog';
