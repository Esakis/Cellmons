import { fileURLToPath } from 'node:url';
import { loadEnv } from 'vite';
import solid from 'vite-plugin-solid';
import { defineConfig } from 'vitest/config';

export default defineConfig(({ mode }) => {
  // Adres serwera gry z pliku .env w katalogu głównym repozytorium (ten sam plik czyta serwer).
  const rootEnv = loadEnv(mode, fileURLToPath(new URL('..', import.meta.url)), '');
  const listenAddr = rootEnv.LISTEN_ADDR || '0.0.0.0:8080';
  const gameServer = `http://${listenAddr.replace('0.0.0.0', '127.0.0.1')}`;

  return {
    // HMR (solid-refresh) tylko poza testami: Vitest nie ładuje modułu wirtualnego /@solid-refresh.
    plugins: [solid({ hot: mode !== 'test' })],
    server: {
      port: 5173,
      strictPort: true,
      // Lokalnie klient i serwer gry działają na różnych portach.
      proxy: { '/healthz': gameServer, '/readyz': gameServer },
    },
    preview: { port: 4173, strictPort: true },
    test: { environment: 'happy-dom' },
  };
});
