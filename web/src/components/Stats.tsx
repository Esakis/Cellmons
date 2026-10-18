import { For } from 'solid-js';
import { icon, statIcons } from '../app/content';
import { t } from '../i18n';

// Stat values under their icons; each stat's name stays as the alt text and tooltip.
export function Stats(props: { values: readonly number[] }) {
  return (
    <dl class="stats">
      <For each={statIcons}>
        {([key, label], index) => (
          <>
            <dt>
              <img class="icon" src={icon(key)} alt={t(label)} title={t(label)} />
            </dt>
            <dd>{props.values[index()] ?? '—'}</dd>
          </>
        )}
      </For>
    </dl>
  );
}
