/** Stan jednej usługi lokalnego środowiska. */
export type ServiceState = 'ok' | 'unavailable' | 'unknown';

/** Stan lokalnego środowiska pokazywany w wersji deweloperskiej. */
export interface DevStatus {
  server: ServiceState;
  database: ServiceState;
  version?: string;
}

type Fetch = (url: string) => Promise<Response>;

/**
 * Pyta serwer gry o `/healthz` i `/readyz` (przez proxy Vite, `vite.config.ts`).
 * Bazę sprawdza `/readyz`, więc gdy serwer nie odpowiada, stan bazy jest nieznany.
 */
export async function fetchDevStatus(fetchFn: Fetch = (url) => fetch(url)): Promise<DevStatus> {
  const health = await request(fetchFn, '/healthz');
  if (!health?.ok) {
    return { server: 'unavailable', database: 'unknown' };
  }
  const version = await readVersion(health);
  const ready = await request(fetchFn, '/readyz');
  return { server: 'ok', database: ready?.ok ? 'ok' : 'unavailable', version };
}

async function request(fetchFn: Fetch, url: string): Promise<Response | null> {
  try {
    return await fetchFn(url);
  } catch {
    return null;
  }
}

async function readVersion(response: Response): Promise<string | undefined> {
  try {
    const body: unknown = await response.json();
    if (typeof body === 'object' && body !== null && 'version' in body) {
      return typeof body.version === 'string' ? body.version : undefined;
    }
  } catch {
    // Odpowiedź bez JSON: wersja pozostaje nieznana.
  }
  return undefined;
}
