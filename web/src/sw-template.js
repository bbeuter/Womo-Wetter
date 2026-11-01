/* Service Worker „Sonnen-Radar“ – wird beim Build nach dist/sw.js geschrieben.
   App-Shell: cache-first. /data/*: network-first mit Cache-Fallback (offline letzter Stand). */
const VERSION = '__VERSION__';
const SHELL = __SHELL__;
const SHELL_CACHE = 'shell-' + VERSION;
const DATA_CACHE = 'data-v1';

self.addEventListener('install', e => {
  e.waitUntil(caches.open(SHELL_CACHE).then(c => c.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener('activate', e => {
  e.waitUntil(caches.keys()
    .then(keys => Promise.all(keys.filter(k => k.startsWith('shell-') && k !== SHELL_CACHE).map(k => caches.delete(k))))
    .then(() => self.clients.claim()));
});
self.addEventListener('fetch', e => {
  const req = e.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;
  if (url.pathname.includes('/data/')) {
    e.respondWith(networkFirst(req));
  } else if (req.mode === 'navigate') {
    e.respondWith(caches.match('./', { ignoreSearch: true }).then(r => r || fetch(req)));
  } else {
    e.respondWith(caches.match(req, { ignoreSearch: true }).then(r => r || fetch(req).then(res => {
      if (res.ok) { const copy = res.clone(); caches.open(SHELL_CACHE).then(c => c.put(req, copy)); }
      return res;
    })));
  }
});
async function networkFirst(req) {
  const cache = await caches.open(DATA_CACHE);
  try {
    const res = await fetch(req, { cache: 'no-cache' });
    if (res.ok) cache.put(req, res.clone());
    return res;
  } catch (err) {
    const hit = await cache.match(req, { ignoreSearch: true });
    if (hit) return hit;
    throw err;
  }
}
