import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadEnv } from 'vite';
import solid from 'vite-plugin-solid';
import { defineConfig } from 'vitest/config';
import { offlinePlugin } from './pwa-plugin.ts';

export default defineConfig(({ mode }) => {
  // Keep descriptive names in source; shorten repeated module selectors in shipped JS/CSS.
  const moduleNames = new Map<string, Map<string, string>>();
  const scopedName = (name: string, filename: string) => {
    const file = relative(fileURLToPath(new URL('.', import.meta.url)), filename).replaceAll(
      '\\',
      '/',
    );
    const names = moduleNames.get(file) ?? new Map<string, string>();
    moduleNames.set(file, names);
    const prefix = createHash('sha256').update(file).digest('hex').slice(0, 8);
    const value = names.get(name) ?? `_m${prefix}_${names.size.toString(36)}`;
    names.set(name, value);
    return value;
  };
  // Adres serwera gry z pliku .env w katalogu głównym repozytorium (ten sam plik czyta serwer).
  const rootEnv = loadEnv(mode, fileURLToPath(new URL('..', import.meta.url)), '');
  const listenAddr = rootEnv.LISTEN_ADDR || '0.0.0.0:8080';
  const gameServer = `http://${listenAddr.replace('0.0.0.0', '127.0.0.1')}`;
  const android = (process.env.VITE_PLATFORM ?? rootEnv.VITE_PLATFORM) === 'android';
  const apiOrigin = android ? (process.env.VITE_API_ORIGIN ?? rootEnv.VITE_API_ORIGIN ?? '') : '';
  if (apiOrigin && (new URL(apiOrigin).origin !== apiOrigin || !apiOrigin.startsWith('https://'))) {
    throw new Error('VITE_API_ORIGIN must be an HTTPS origin without a path');
  }

  return {
    // HMR (solid-refresh) tylko poza testami: Vitest nie ładuje modułu wirtualnego /@solid-refresh.
    base: './',
    envDir: fileURLToPath(new URL('..', import.meta.url)),
    plugins: [
      solid({ hot: mode !== 'test' }),
      {
        name: 'fluent-without-source-comments',
        enforce: 'pre',
        async load(id) {
          if (!id.endsWith('.ftl?raw')) return;
          const source = await readFile(id.slice(0, -4), 'utf8');
          return `export default ${JSON.stringify(source.replace(/^#[^\r\n]*(?:\r?\n|$)/gm, ''))};`;
        },
      },
      {
        name: 'native-api-csp',
        transformIndexHtml(html) {
          return apiOrigin
            ? html.replace(
                "connect-src 'self'",
                `connect-src 'self' ${apiOrigin} ${apiOrigin.replace('https:', 'wss:')}`,
              )
            : html;
        },
      },
      {
        name: 'development-csp',
        apply: 'serve',
        transformIndexHtml(html) {
          return html.replace(/\s*<meta http-equiv="Content-Security-Policy"[^>]+>/, '');
        },
      },
      ...(mode === 'test' ? [] : [offlinePlugin()]),
    ],
    worker: { format: 'es' },
    css: { modules: { generateScopedName: scopedName } },
    build: { assetsInlineLimit: 0, manifest: true },
    server: {
      port: 5173,
      strictPort: true,
      // Lokalnie klient i serwer gry działają na różnych portach.
      proxy: {
        '/healthz': gameServer,
        '/readyz': gameServer,
        '/api': gameServer,
        '/ws': { target: gameServer, ws: true },
      },
    },
    preview: { port: 4173, strictPort: true },
    test: { environment: 'happy-dom' },
  };
});
