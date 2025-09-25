// Basic service worker for photoframe-rs PWA.
// Caches static assets but forces reload when new versions are detected.

const CACHE = 'photoframe-rs-v2'; // Increment when you want to force cache refresh
const CORE_ASSETS = [
  '/site.webmanifest',
  '/favicon-32x32.png',
  '/favicon-16x16.png',
  '/android-chrome-192x192.png',
  '/android-chrome-512x512.png',
  '/apple-touch-icon.png',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(CORE_ASSETS)).then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))
      )
    ).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (event) => {
  const req = event.request;
  if (req.method !== 'GET') return; // pass through

  const url = new URL(req.url);
  
  // Always use network-first for navigation and API requests to ensure fresh content
  if (req.mode === 'navigate' || url.pathname.startsWith('/api/')) {
    event.respondWith(
      fetch(req).catch(() => caches.match('/'))
    );
    return;
  }
  
  // Network-first with fast cache fallback for JS/CSS to detect updates quickly
  if (url.pathname.includes('/_next/') || url.pathname.endsWith('.js') || url.pathname.endsWith('.css')) {
    event.respondWith(
      fetch(req).then((resp) => {
        // Only cache successful responses
        if (resp.ok) {
          const copy = resp.clone();
          caches.open(CACHE).then((c) => c.put(req, copy));
        }
        return resp;
      }).catch(() => caches.match(req))
    );
    return;
  }
  
  // Cache-first for static assets (icons, manifest, etc.)
  if (url.origin === location.origin && CORE_ASSETS.includes(url.pathname)) {
    event.respondWith(
      caches.match(req).then((cached) =>
        cached || fetch(req).then((resp) => {
          if (resp.ok) {
            const copy = resp.clone();
            caches.open(CACHE).then((c) => c.put(req, copy));
          }
          return resp;
        })
      )
    );
  }
});
