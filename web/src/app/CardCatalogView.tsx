import { For } from 'solid-js';
import './cardCatalog';
import { t } from '../i18n';
import { cards, content, fluentKey } from './content';
export function CardOptions() {
  return (
    <For each={content.cards}>{(c) => <option value={c.id}>{t(fluentKey(c.key))}</option>}</For>
  );
}
export function ExpeditionCard(props: { id: number }) {
  return <p>{t('exp-award-card', { name: t(fluentKey(cards.get(props.id)?.key ?? '')) })}</p>;
}
export function TournamentCard(props: { id: number }) {
  return <p>{t('tourney-prize-card', { name: t(fluentKey(cards.get(props.id)?.key ?? '')) })}</p>;
}
