import { type PluginListenerHandle, registerPlugin } from '@capacitor/core';
export type Metadata = {
  nativeVersion: string;
  bundleVersion: number;
  storeUrl: string;
  webViewVersion: string;
  notificationsAllowed: boolean;
  screenAwake: boolean;
};
export interface NativePlatform {
  loadSession(): Promise<{ token: string | null; rotatedAt: number }>;
  saveSession(input: { token: string; rotatedAt: number }): Promise<void>;
  clearSession(): Promise<void>;
  metadata(): Promise<Metadata>;
  configure(input: {
    duel: boolean;
    online: boolean;
    awake: boolean;
    haptics: boolean;
    reminders: boolean;
    title: string;
    body: string;
  }): Promise<void>;
  enableReminders(): Promise<{ allowed: boolean }>;
  haptic(input: { kind: 'hit' | 'combo' | 'end' }): Promise<void>;
  minimize(): Promise<void>;
  checkBundle(input?: {
    force: boolean;
  }): Promise<{ enabled: boolean; pending: number; needsNative: boolean }>;
  applyBundle(input: { idle: boolean }): Promise<void>;
  confirmBundle(): Promise<void>;
  addListener(
    event: 'resume' | 'pause' | 'back',
    listener: () => void,
  ): Promise<PluginListenerHandle>;
  addListener(
    event: 'network',
    listener: (state: { connected: boolean }) => void,
  ): Promise<PluginListenerHandle>;
}
export const native = registerPlugin<NativePlatform>('GamePlatform');
