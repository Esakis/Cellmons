import { avatars, cards, fluentKey } from '../../app/content';
import { t } from '../../i18n';
import type { AvatarState, Mutation } from './state';

export function progressionDescription(mutation: Mutation, owned: AvatarState[] = []): string {
  if (mutation.action === 'shade')
    return t('collection-shade-choice', { school: t(`school-${mutation.school}-name`) });
  if (mutation.action !== 'reincarnate') return '';
  const c = mutation.choice;
  const choice =
    c.kind === 'path'
      ? t(fluentKey(avatars.get(c.id)?.key ?? ''))
      : c.kind === 'specialization'
        ? t(`collection-spec-${c.name}`)
        : t(fluentKey(cards.get(c.card_id)?.key ?? ''));
  const donor = owned.find((a) => a.id === mutation.sacrifice);
  return donor
    ? `${choice} · ${t('collection-consuming', { character: `${t(fluentKey(avatars.get(donor.path_id ?? donor.template_id)?.key ?? ''))} #${donor.slot + 1}` })}`
    : choice;
}
