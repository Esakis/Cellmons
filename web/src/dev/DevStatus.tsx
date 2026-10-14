import { createResource, onCleanup, Show } from 'solid-js';
import styles from './DevStatus.module.css';
import { fetchDevStatus, type ServiceState } from './status';

const REFRESH_MS = 5000;

const STATE_LABELS: Record<ServiceState, string> = {
  ok: 'działa',
  unavailable: 'nie odpowiada',
  unknown: 'stan nieznany',
};

/** Panel stanu serwera gry i bazy danych; ładowany tylko w wersji deweloperskiej (`index.tsx`). */
export default function DevStatus() {
  const [status, { refetch }] = createResource(() => fetchDevStatus());
  const timer = setInterval(() => void refetch(), REFRESH_MS);
  onCleanup(() => clearInterval(timer));

  const row = (label: string, state: ServiceState) => (
    <li class={styles.row}>
      <span class={styles.dot} data-state={state} aria-hidden="true" />
      {label}: {STATE_LABELS[state]}
    </li>
  );

  return (
    <aside class={styles.panel} aria-label="Stan środowiska lokalnego">
      <p class={styles.title}>Środowisko lokalne</p>
      <ul class={styles.list}>
        {row('Serwer gry', status()?.server ?? 'unknown')}
        {row('Baza danych', status()?.database ?? 'unknown')}
      </ul>
      <Show when={status()?.version}>
        {(version) => <p class={styles.version}>Wersja serwera: {version()}</p>}
      </Show>
    </aside>
  );
}
