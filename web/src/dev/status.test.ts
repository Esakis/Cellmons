import { describe, expect, it } from 'vitest';
import { fetchDevStatus } from './status';

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

/** Atrapa fetch: zwraca odpowiedzi dla podanych ścieżek, a dla pozostałych zgłasza błąd sieci. */
function fakeFetch(routes: Record<string, () => Response>) {
  return async (url: string) => {
    const route = routes[url];
    if (!route) {
      throw new TypeError('Failed to fetch');
    }
    return route();
  };
}

describe('fetchDevStatus', () => {
  it('serwer i baza działają', async () => {
    const status = await fetchDevStatus(
      fakeFetch({
        '/healthz': () => json(200, { status: 'ok', version: '0.1.0' }),
        '/readyz': () => json(200, { status: 'ok', database: 'ok' }),
      }),
    );
    expect(status).toEqual({ server: 'ok', database: 'ok', version: '0.1.0' });
  });

  it('serwer działa, a baza nie odpowiada', async () => {
    const status = await fetchDevStatus(
      fakeFetch({
        '/healthz': () => json(200, { status: 'ok', version: '0.1.0' }),
        '/readyz': () => json(503, { status: 'unavailable', database: 'unavailable' }),
      }),
    );
    expect(status).toEqual({ server: 'ok', database: 'unavailable', version: '0.1.0' });
  });

  it('serwer nie odpowiada, a proxy Vite zwraca błąd', async () => {
    const status = await fetchDevStatus(
      fakeFetch({ '/healthz': () => new Response('', { status: 502 }) }),
    );
    expect(status).toEqual({ server: 'unavailable', database: 'unknown' });
  });

  it('brak połączenia z serwerem Vite', async () => {
    const status = await fetchDevStatus(fakeFetch({}));
    expect(status).toEqual({ server: 'unavailable', database: 'unknown' });
  });
});
