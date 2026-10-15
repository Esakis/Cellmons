// Filled with the complete, hashed build asset list by the Vite plugin.
const CACHE = '__CACHE__';
const ASSETS = __ASSETS__;
self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(ASSETS)));
});
self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys())
        if (key.startsWith('game-') && key !== CACHE) await caches.delete(key);
      await self.clients.claim();
      for (const client of await self.clients.matchAll()) client.postMessage('offline-ready');
    })(),
  );
});
self.addEventListener('message', (event) => {
  // Updates in another tab must also respect this tab's duel.
  if (event.data === 'activate')
    event.waitUntil(
      (async () => {
        const clients = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
        const idle = await Promise.all(
          clients.map(
            (client) =>
              new Promise((resolve) => {
                const channel = new MessageChannel();
                const timeout = setTimeout(() => {
                  channel.port1.close();
                  resolve(false);
                }, 1500);
                channel.port1.onmessage = (message) => {
                  clearTimeout(timeout);
                  channel.port1.close();
                  resolve(message.data === true);
                };
                client.postMessage('can-update', [channel.port2]);
              }),
          ),
        );
        if (idle.every(Boolean)) await self.skipWaiting();
      })(),
    );
  if (event.data === 'status') event.source?.postMessage('offline-ready');
});
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);
  if (
    event.request.method !== 'GET' ||
    url.origin !== self.location.origin ||
    url.pathname.startsWith('/api/') ||
    url.pathname.startsWith('/ws') ||
    url.pathname === '/healthz' ||
    url.pathname === '/readyz'
  )
    return;
  const path = url.pathname.slice(new URL(self.registration.scope).pathname.length);
  if (event.request.mode === 'navigate') {
    event.respondWith(
      caches
        .open(CACHE)
        .then((cache) => cache.match('./index.html'))
        .then((cached) => cached || fetch(event.request)),
    );
  } else if (ASSETS.includes(`./${path}`)) {
    // These are immutable public build files. Vite's Vary: Origin differs between
    // precache fetches and module requests; origin was already checked above.
    event.respondWith(
      caches
        .open(CACHE)
        .then((cache) => cache.match(event.request, { ignoreVary: true }))
        .then((cached) => cached || fetch(event.request)),
    );
  }
});
