import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  // Temporary debug identifier. The release ID is a separate F12A decision.
  appId: 'dev.game.training',
  appName: 'Spellgambit',
  webDir: '../web/dist',
  android: { allowMixedContent: false },
};
export default config;
