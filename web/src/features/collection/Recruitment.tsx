import { createMemo, createSignal, For, Show } from 'solid-js';
import { avatars, cardArgs, content, fluentKey, portrait } from '../../app/content';
import { Modal } from '../../components/Modal';
import { addTranslations, t } from '../../i18n';
import en from './recruitment.en.ftl?raw';
import pl from './recruitment.pl.ftl?raw';
import type { Mutation, RecruitmentState } from './state';
import './recruitment.css';

addTranslations({ pl, en });

export function Recruitment(props: {
  state: RecruitmentState;
  full: boolean;
  disabled: boolean;
  error: string;
  pictures: Map<string, string>;
  claim: (mutation: Mutation) => Promise<boolean>;
}) {
  const [preview, setPreview] = createSignal<number>();
  const ready = () =>
    props.state.milestones.filter((m) => !m.claimed && m.level <= props.state.level);
  const next = () => props.state.milestones.find((m) => !m.claimed && m.level > props.state.level);
  const choice = () => avatars.get(preview() ?? 0);
  const deck = createMemo(() => {
    const starter = content.starters.find((s) => s.avatar === choice()?.key);
    return Object.entries(starter?.deck ?? {}).flatMap(([key, count]) => {
      const card = content.cards.find((c) => c.key === key);
      return card ? [{ card, count }] : [];
    });
  });
  async function claim(template: number | null) {
    const milestone = ready()[0];
    if (!milestone || props.disabled) return;
    if (await props.claim({ action: 'recruit', level: milestone.level, template_id: template }))
      setPreview(undefined);
  }
  const className = (id: number) => t(fluentKey(avatars.get(id)?.key ?? ''));
  // Closed by default: the gold count in the summary says when a reward waits (the deck comes first).
  return (
    <details class="recruitment">
      <summary>
        {t('recruit-title')} · {t('recruit-level', { level: props.state.level })}
        <Show when={ready().length}>
          <span class="recruit-ready">{t('recruit-ready', { count: ready().length })}</span>
        </Show>
      </summary>
      <details class="recruit-history">
        <summary>{t('recruit-milestones')}</summary>
        <ol class="recruit-milestones" aria-label={t('recruit-milestones')}>
          <For each={props.state.milestones}>
            {(milestone) => (
              <li
                classList={{
                  earned: milestone.claimed,
                  seal: milestone.claimed && !milestone.template_id,
                  ready: !milestone.claimed && milestone.level <= props.state.level,
                }}
              >
                <strong>{t('recruit-level', { level: milestone.level })}</strong>
                <span>
                  {milestone.claimed
                    ? milestone.template_id
                      ? className(milestone.template_id)
                      : t('recruit-seal', { level: milestone.level })
                    : t(milestone.level <= props.state.level ? 'recruit-pick' : 'recruit-locked')}
                </span>
              </li>
            )}
          </For>
        </ol>
      </details>
      <Show
        when={ready().length > 0}
        fallback={
          <p>{next() ? t('recruit-next', { level: next()?.level ?? 0 }) : t('recruit-complete')}</p>
        }
      >
        <Show
          when={props.state.available.length > 0}
          fallback={
            <div class="recruit-seal-reward">
              <p>{t('recruit-all-owned')}</p>
              <strong>{t('recruit-seal', { level: ready()[0]?.level ?? 0 })}</strong>
              <button type="button" disabled={props.disabled} onClick={() => void claim(null)}>
                {t('recruit-claim-seal')}
              </button>
            </div>
          }
        >
          <p>{t('recruit-choose', { level: ready()[0]?.level ?? 0 })}</p>
          <Show when={props.full}>
            <p role="status">{t('recruit-full')}</p>
          </Show>
          <div class="recruit-choices">
            <For each={props.state.available}>
              {(id) => (
                <button
                  type="button"
                  onClick={() => setPreview(id)}
                  aria-label={t('recruit-preview', { name: className(id) })}
                >
                  <img
                    src={portrait(avatars.get(id)?.key ?? '')}
                    width="64"
                    height="64"
                    alt=""
                    loading="lazy"
                  />
                  <strong>{className(id)}</strong>
                </button>
              )}
            </For>
          </div>
        </Show>
      </Show>
      <Show when={choice()}>
        {(avatar) => (
          <Modal
            title={className(avatar().id)}
            close={() => !props.disabled && setPreview(undefined)}
            wide
          >
            <div class="recruit-preview-heading">
              <img src={portrait(avatar().key)} width="96" height="96" alt="" />
              <p>{t(`recruit-role-${avatar().id}`)}</p>
            </div>
            <p>{t('recruit-starter')}</p>
            <details>
              <summary>{t('recruit-deck')}</summary>
              <ul class="recruit-deck">
                <For each={deck()}>
                  {({ card, count }) => (
                    <li>
                      <img
                        src={props.pictures.get(card.key)}
                        width="80"
                        height="50"
                        alt=""
                        loading="lazy"
                      />
                      <div>
                        <strong>
                          {count} × {t(fluentKey(card.key))}
                        </strong>
                        <small>
                          {t('focus')} {card.cost.focus} · {t('stamina')} {card.cost.stamina}
                        </small>
                        <p>{t(fluentKey(card.key, 'text'), cardArgs(card))}</p>
                      </div>
                    </li>
                  )}
                </For>
              </ul>
            </details>
            <p>{t('recruit-confirm-help', { level: ready()[0]?.level ?? 0 })}</p>
            <Show when={props.error}>
              <p class="collection-error" role="alert">
                {props.error}
              </p>
            </Show>
            <Show when={props.full}>
              <p>{t('recruit-full')}</p>
            </Show>
            <button
              type="button"
              class="primary"
              disabled={
                props.disabled ||
                props.full ||
                !ready().length ||
                !props.state.available.includes(avatar().id)
              }
              onClick={() => void claim(avatar().id)}
            >
              {t('recruit-confirm', { name: className(avatar().id) })}
            </button>
          </Modal>
        )}
      </Show>
    </details>
  );
}
