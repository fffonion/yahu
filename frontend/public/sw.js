const CACHE = 'yahu-v2';
const PRECACHE = ['/manifest.json', '/icon.svg', '/icon-192.png', '/icon-512.png'];

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(CACHE).then((c) => c.addAll(PRECACHE)));
  self.skipWaiting();
});

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)))
    )
  );
  self.clients.claim();
});

self.addEventListener('fetch', (e) => {
  const { request } = e;
  // Skip non-GET and API/proxy requests
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (request.mode === 'navigate' || url.pathname === '/') {
    e.respondWith(fetch(request));
    return;
  }
  if (url.pathname.startsWith('/hermes') || url.pathname.startsWith('/api') || url.pathname.startsWith('/chat') || url.pathname.startsWith('/login') || url.pathname.startsWith('/logout') || url.pathname.startsWith('/health') || url.pathname.startsWith('/workspace') || url.pathname.startsWith('/images') || url.pathname.startsWith('/image-') || url.pathname.startsWith('/skills') || url.pathname.startsWith('/memory') || url.pathname.startsWith('/cron') || url.pathname.startsWith('/insights') || url.pathname.startsWith('/models') || url.pathname.startsWith('/watch')) return;

  e.respondWith(
    caches.match(request).then((cached) => {
      if (cached) return cached;
      return fetch(request).then((resp) => {
        if (resp.ok && resp.type === 'basic') {
          const clone = resp.clone();
          caches.open(CACHE).then((c) => c.put(request, clone));
        }
        return resp;
      });
    })
  );
});
