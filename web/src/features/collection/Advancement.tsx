import { For, Show } from 'solid-js';
import { avatars, fluentKey, portrait } from '../../app/content';
import { addTranslations, t } from '../../i18n';
import en from './advancement.en.ftl?raw';
import pl from './advancement.pl.ftl?raw';
import type { AdvancementState, Mutation, RecruitmentState } from './state';
import './advancement.css';

addTranslations({ pl, en });
export function Advancement(props: {
  state: AdvancementState;
  recruitment: RecruitmentState;
  disabled: boolean;
  full: boolean;
  mutate: (m: Mutation) => Promise<boolean>;
}) {
  const progress = (id: number) => props.state.contracts.find((c) => c.template_id === id);
  return (
    <details class="advancement">
      <summary>{t('advancement-title')}</summary>
      <Show when={props.recruitment.level >= 30}>
        <p class="mastery-badge">{t('advancement-master-title')}</p>
      </Show>
      <p>
        {t('advancement-daily', { count: props.state.daily_fights })}{' '}
        {props.state.daily_rewarded ? t('advancement-daily-done') : ''}
      </p>
      {/* Collection.tsx shows this panel from account level 4, when contracts open. */}
      <h2>{t('contract-title')}</h2>
      <p>{t('contract-help')}</p>
      <Show when={props.full}>
        <p>{t('contract-full')}</p>
      </Show>
      <div class="contract-grid">
        <For each={props.recruitment.available} fallback={<p>{t('contract-complete')}</p>}>
          {(id) => (
            <article classList={{ active: progress(id)?.active }}>
              <img
                src={portrait(avatars.get(id)?.key ?? '')}
                alt=""
                width="64"
                height="64"
                loading="lazy"
              />
              <h3>{t(fluentKey(avatars.get(id)?.key ?? ''))}</h3>
              <p>{t('contract-progress', { count: progress(id)?.fragments ?? 0 })}</p>
              <progress
                max={30}
                value={progress(id)?.fragments ?? 0}
                aria-label={t('contract-progress', { count: progress(id)?.fragments ?? 0 })}
              />
              <button
                type="button"
                disabled={
                  props.disabled ||
                  (progress(id)?.fragments === 30 ? props.full : progress(id)?.active)
                }
                onClick={() =>
                  void props.mutate({
                    action: 'contract',
                    template_id: id,
                    claim: progress(id)?.fragments === 30,
                  })
                }
              >
                {t(
                  progress(id)?.fragments === 30
                    ? 'contract-claim'
                    : progress(id)?.active
                      ? 'contract-active'
                      : 'contract-select',
                )}
              </button>
            </article>
          )}
        </For>
      </div>
      <Show when={props.recruitment.level >= 7}>
        <h2>{t('mastery-title')}</h2>
        <p>{t('mastery-help')}</p>
      </Show>
    </details>
  );
}
