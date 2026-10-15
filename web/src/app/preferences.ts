let ephemeral = false;
const memory = new Map<string, string>();

export function useEphemeralPreferences(enabled: boolean) {
  ephemeral = enabled;
  memory.clear();
}

export function readPreference(key: string, fallback: string): string {
  if (ephemeral) return memory.get(key) ?? fallback;
  try {
    return localStorage.getItem(`game.${key}`) ?? fallback;
  } catch {
    return fallback;
  }
}
export function writePreference(key: string, value: string) {
  if (ephemeral) {
    memory.set(key, value);
    return;
  }
  try {
    localStorage.setItem(`game.${key}`, value);
  } catch {
    /* Preferences are optional. */
  }
}
