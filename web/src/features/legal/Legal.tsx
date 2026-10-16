import { A, useLocation } from '@solidjs/router';
import { createSignal, For, Show } from 'solid-js';
import data from '../../generated/legal.json';
import { language } from '../../i18n';
import './legal.css';

type Kind = 'terms' | 'privacy' | 'deletion';
export function Legal() {
  const [software, setSoftware] = createSignal('');
  const [softwareError, setSoftwareError] = createSignal(false);
  const location = useLocation();
  const locale = () => (language() === 'pl' ? data.pl : data.en);
  const kind = (): Kind => {
    const value = new URLSearchParams(location.search).get('doc');
    return value === 'privacy' || value === 'deletion' ? value : 'terms';
  };
  const document = () => locale()[kind()];
  const labels = () =>
    language() === 'pl'
      ? [
          'Operator',
          'Adres',
          'Pomoc i reklamacje',
          'Kontakt w sprawie danych',
          'Zgłoszenia bezpieczeństwa',
          'Podmioty przetwarzające',
          'Transfery danych',
          'Retencja danych bezpieczeństwa',
          'Retencja kopii',
        ]
      : [
          'Operator',
          'Postal address',
          'Support and complaints',
          'Data protection contact',
          'Security reports',
          'Processors',
          'Data transfers',
          'Security retention',
          'Backup retention',
        ];
  const fields = [
    'operator',
    'postal_address',
    'support_email',
    'privacy_email',
    'security_email',
    'processors',
    'transfers',
    'security_retention',
    'backup_retention',
  ] as const;
  return (
    <section class="legal-page">
      <h1>{document().title}</h1>
      <p>{data.version}</p>
      <Show when={data.draft}>
        <p role="status">{locale().draft}</p>
      </Show>
      <nav aria-label={locale().heading}>
        <For each={['terms', 'privacy', 'deletion'] as const}>
          {(value) => <A href={`/legal?doc=${value}`}>{locale()[value].title}</A>}
        </For>
      </nav>
      <For each={document().sections}>
        {([title, text]) => (
          <section>
            <h2>{title}</h2>
            <p>{text}</p>
          </section>
        )}
      </For>
      <Show when={kind() === 'deletion'}>
        <A class="button primary" href="/account?privacy=1">
          {language() === 'pl' ? 'Otwórz panel danych' : 'Open the data panel'}
        </A>
      </Show>
      <h2>{locale().contact}</h2>
      <dl>
        <For each={fields}>
          {(key, index) => (
            <>
              <dt>{labels()[index()]}</dt>
              <dd>{data.site[key] || '—'}</dd>
            </>
          )}
        </For>
      </dl>
      <details>
        <summary>{language() === 'pl' ? 'Licencje czcionek' : 'Font licenses'}</summary>
        <For each={data.credits}>{(text) => <pre lang="en">{text}</pre>}</For>
      </details>
      <details
        onToggle={(event) => {
          if (event.currentTarget.open && !software())
            void import('./software')
              .then((module) => {
                setSoftwareError(false);
                setSoftware(module.default);
              })
              .catch(() => setSoftwareError(true));
        }}
      >
        <summary>{language() === 'pl' ? 'Licencje oprogramowania' : 'Software licenses'}</summary>
        <pre lang="en">{software()}</pre>
        <Show when={softwareError()}>
          <p role="alert">
            {language() === 'pl'
              ? 'Nie udało się pobrać licencji. Połącz się z siecią i otwórz tę sekcję ponownie.'
              : 'Could not load the licenses. Connect to the network and reopen this section.'}
          </p>
        </Show>
      </details>
    </section>
  );
}
