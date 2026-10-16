import { A, HashRouter, Route } from '@solidjs/router';
import { createSignal, lazy, onCleanup, Show } from 'solid-js';
import { TrainingSetup } from '../features/training/Setup';
import { t } from '../i18n';
import { account } from './account';
import { Settings } from './Settings';
import { Shell } from './Shell';

const Duel = lazy(() => import('../features/offline').then((module) => ({ default: module.Duel })));
const AccountPage = lazy(() =>
  import('../features/account/Account').then((module) => ({ default: module.AccountPage })),
);
const Lobby = lazy(() => import('../features/lobby/Lobby').then((m) => ({ default: m.Lobby })));
const CollectionPage = lazy(() =>
  import('../features/offline').then((m) => ({ default: m.Collection })),
);
const Store = lazy(() => import('../features/store/Store').then((m) => ({ default: m.Store })));
const Clans = lazy(() => import('../features/clans/Clans').then((m) => ({ default: m.Clans })));
const Admin = lazy(() => import('../features/admin/Admin').then((m) => ({ default: m.Admin })));
const Legal = lazy(() => import('../features/legal/Legal').then((m) => ({ default: m.Legal })));
function OnlineRoute(props: {
  account?: boolean;
  store?: boolean;
  clans?: boolean;
  admin?: boolean;
}) {
  const [online, setOnline] = createSignal(navigator.onLine);
  const [entered, setEntered] = createSignal(navigator.onLine);
  const update = () => {
    setOnline(navigator.onLine);
    if (navigator.onLine) setEntered(true);
  };
  window.addEventListener('online', update);
  window.addEventListener('offline', update);
  onCleanup(() => {
    window.removeEventListener('online', update);
    window.removeEventListener('offline', update);
  });
  return (
    <Show
      when={(entered() || online()) && (props.account || account()?.kind !== 'guest')}
      fallback={
        <section class="settings-page">
          <h1>{t(props.clans ? 'clans' : props.store ? 'store' : 'lobby')}</h1>
          <p>{t(props.clans ? 'clans-gate' : props.store ? 'store-gate' : 'lobby-gate')}</p>
          <A href="/">{t('training')}</A> · <A href="/account">{t('auth-account')}</A>
        </section>
      }
    >
      <Show
        when={props.account}
        fallback={
          <Show
            when={props.store}
            fallback={
              <Show
                when={props.clans}
                fallback={
                  <Show when={props.admin} fallback={<Lobby />}>
                    <Admin />
                  </Show>
                }
              >
                <Clans />
              </Show>
            }
          >
            <Store />
          </Show>
        }
      >
        <AccountPage />
      </Show>
    </Show>
  );
}

export function App() {
  return (
    <HashRouter root={Shell}>
      <Route path="/" component={TrainingSetup} />
      <Route path="/duel" component={Duel} />
      <Route path="/settings" component={Settings} />
      <Route path="/account" component={() => <OnlineRoute account />} />
      <Route path="/lobby" component={OnlineRoute} />
      <Route path="/store" component={() => <OnlineRoute store />} />
      <Route path="/clans" component={() => <OnlineRoute clans />} />
      <Route path="/admin" component={() => <OnlineRoute admin />} />
      <Route path="/legal" component={Legal} />
      <Route path="/collection" component={CollectionPage} />
      <Route path="*" component={TrainingSetup} />
    </HashRouter>
  );
}
