import { HashRouter, Route } from '@solidjs/router';
import { lazy } from 'solid-js';
import { TrainingSetup } from '../features/training/Setup';
import { Settings } from './Settings';
import { Shell } from './Shell';

const Duel = lazy(() =>
  import('../features/training/Training').then((module) => ({ default: module.Duel })),
);
const AccountPage = lazy(() =>
  import('../features/account/Account').then((module) => ({ default: module.AccountPage })),
);

export function App() {
  return (
    <HashRouter root={Shell}>
      <Route path="/" component={TrainingSetup} />
      <Route path="/duel" component={Duel} />
      <Route path="/settings" component={Settings} />
      <Route path="/account" component={AccountPage} />
      <Route path="*" component={TrainingSetup} />
    </HashRouter>
  );
}
