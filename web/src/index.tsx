import { lazy, Show } from 'solid-js';
import { render } from 'solid-js/web';
import { App } from './app/App';
import { account } from './app/account';
import './styles/global.css';
import { registerOffline } from './pwa';

// Panel stanu serwera tylko w wersji deweloperskiej; w buildzie produkcyjnym znika razem z importem.
const DevStatus = import.meta.env.DEV ? lazy(() => import('./dev/DevStatus')) : null;

const root = document.getElementById('root');
if (!root) {
  throw new Error('Brak elementu #root w index.html');
}

render(
  () => (
    <>
      <App />
      <Show when={account()?.kind !== 'guest'}>{DevStatus && <DevStatus />}</Show>
    </>
  ),
  root,
);

void registerOffline();
if (import.meta.env.VITE_PLATFORM === 'android')
  void import('./platform/android').then((m) => m.initialize()).catch(() => {});
