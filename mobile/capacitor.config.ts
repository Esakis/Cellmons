import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  // Debug fallback; release uses the same owner-chosen ID as Gradle.
  appId: process.env.ANDROID_APPLICATION_ID ?? 'dev.game.training',
  appName: 'Spellgambit',
  webDir: '../web/dist',
  // Bridge arguments/results include credentials. Never emit them to Android or JS logs.
  loggingBehavior: 'none',
  android: { allowMixedContent: false },
};
export default config;
