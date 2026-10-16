import { createMemo, createSignal, For, Show } from 'solid-js';
import { avatars, cardArgs, cards, content, fluentKey } from '../../app/content';
import { addTranslations, t } from '../../i18n';
import en from './progression.en.ftl?raw';
import pl from './progression.pl.ftl?raw';

addTranslations({ pl, en });

import type { AvatarState, Mutation } from './state';

const schools = ['combat', 'divine', 'element', 'arcane', 'necro', 'tech'];
const name = (avatar: AvatarState) =>
  t(fluentKey(avatars.get(avatar.path_id ?? avatar.template_id)?.key ?? ''));
export function Progression(props: {
  avatar: AvatarState;
  avatars: AvatarState[];
  disabled: boolean;
  confirm: (mutation: Mutation) => void;
}) {
  const [donor, setDonor] = createSignal('');
  const [school, setSchool] = createSignal('combat');
  const path = () => content.paths.find((p) => p.id === props.avatar.path_id);
  const choices = () =>
    content.paths.filter((p) => p.path.base_avatar === props.avatar.template_id);
  const ready = () =>
    props.avatar.level === 20 && props.avatar.xp >= content.rules.progression.reincarnation_xp;
  const donors = createMemo(() =>
    props.avatars.filter(
      (a) =>
        a.id !== props.avatar.id &&
        a.tier === props.avatar.tier + 1 &&
        a.schools[schools.indexOf(props.avatar.shade_school ?? '')] === 0,
    ),
  );
  const chosenDonor = () => donors().find((a) => a.id === donor()) ?? donors()[0];
  const disabled = () => props.disabled || !ready() || (props.avatar.is_shade && !chosenDonor());
  function reincarnate(choice: Extract<Mutation, { action: 'reincarnate' }>['choice']) {
    props.confirm({
      action: 'reincarnate',
      id: props.avatar.id,
      revision: props.avatar.revision,
      choice,
      ...(props.avatar.is_shade ? { sacrifice: chosenDonor()?.id } : {}),
    });
  }
  return (
    <section class="collection-progression" aria-label={t('collection-progression')}>
      <h2>
        {t('collection-progression')} · {t(`collection-tier-${props.avatar.tier}`)}
      </h2>
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
            {t('collection-gift')}: {t(fluentKey(gift().key))} —{' '}
            {t(fluentKey(gift().key, 'text'), cardArgs(gift()))}
          </p>
        )}
      </Show>
      <Show when={props.avatar.tier < 3} fallback={<p>{t('collection-final-tier')}</p>}>
        <p>
          {t('collection-reincarnation-ready', {
            xp: props.avatar.xp,
            target: content.rules.progression.reincarnation_xp,
            bonus: content.rules.progression.reincarnation_bonus_dp[props.avatar.tier] ?? 0,
          })}
        </p>
        <Show when={props.avatar.is_shade}>
          <p>
            {t('collection-donor-rule', {
              tier: t(`collection-tier-${props.avatar.tier + 1}`),
              school: t(`school-${props.avatar.shade_school}-name`),
            })}
          </p>
          <label>
            {t('collection-donor')}{' '}
            <select
              value={chosenDonor()?.id ?? ''}
              onChange={(e) => setDonor(e.currentTarget.value)}
            >
              <Show when={!donors().length}>
                <option value="">{t('collection-no-donor')}</option>
              </Show>
              <For each={donors()}>
                {(a) => (
                  <option value={a.id}>
                    {name(a)} · {t('collection-level', { level: a.level })} · #{a.slot + 1}
                  </option>
                )}
              </For>
            </select>
          </label>
        </Show>
        <Show when={props.avatar.tier === 0}>
          <div class="collection-paths">
            <For each={choices()}>
              {(p) => (
                <article>
                  <h3>{t(fluentKey(p.key))}</h3>
                  <p>
                    {t('collection-path-stats', {
                      af: p.base.phys_atk,
                      of: p.base.phys_def,
                      am: p.base.mag_atk,
                      om: p.base.mag_def,
                      hp: p.base.hp,
                      stamina: p.base.stamina,
                    })}
                  </p>
                  <p>
                    {Object.entries(p.schools)
                      .map(([s, n]) => `${t(`school-${s}-name`)} ${n}`)
                      .join(' · ')}
                  </p>
                  <p>
                    {t('collection-specialization')}:{' '}
                    {p.path.specializations.map((s) => t(`collection-spec-${s}`)).join(' / ')}
                  </p>
                  <p>
                    {t('collection-gift')}:{' '}
                    {p.path.gifts.map((key) => t(fluentKey(key))).join(' / ')}
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
        <Show when={props.avatar.tier === 2 && path()}>
          {(p) => (
            <div class="collection-paths">
              <For each={p().path.gifts}>
                {(key) => {
                  const card = () => content.cards.find((c) => c.key === key);
                  return (
                    <article>
                      <h3>{t(fluentKey(key))}</h3>
                      <Show when={card()}>
                        {(c) => <p>{t(fluentKey(key, 'text'), cardArgs(c()))}</p>}
                      </Show>
                      <button
                        type="button"
                        disabled={disabled() || !card()}
                        onClick={() => {
                          const c = card();
                          if (c) reincarnate({ kind: 'gift', card_id: c.id });
                        }}
                      >
                        {t('collection-choose-gift', { name: t(fluentKey(key)) })}
                      </button>
                    </article>
                  );
                }}
              </For>
            </div>
          )}
        </Show>
      </Show>
      <Show when={props.avatar.tier === 3 && !props.avatar.is_shade}>
        <p>{t('collection-shade-explain')}</p>
        <label>
          {t('collection-shade-school')}{' '}
          <select value={school()} onChange={(e) => setSchool(e.currentTarget.value)}>
            <For each={schools}>{(s) => <option value={s}>{t(`school-${s}-name`)}</option>}</For>
          </select>
        </label>{' '}
        <button
          type="button"
          disabled={props.disabled || !ready()}
          onClick={() =>
            props.confirm({
              action: 'shade',
              id: props.avatar.id,
              revision: props.avatar.revision,
              new_id: crypto.randomUUID(),
              school: school(),
            })
          }
        >
          {t('collection-create-shade')}
        </button>
      </Show>
    </section>
  );
}

import '../../app/cardCatalog';
