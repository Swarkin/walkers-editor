// noinspection JSUnresolvedReference
// `const BUILD_TIME` unix time as integer (inserted on build time by trunk)

const CACHE_NAME = 'walkers-editor-'+BUILD_TIME;
const filesToCache = [
	'./',
	'./index.html',
	'./initializer.js',
	'./walkers-editor.js',
	'./walkers-editor_bg.wasm',
	'./apple-touch-icon.png',
];

self.addEventListener('activate', (event) => {
	console.debug("[sw] Activate " + BUILD_TIME);
	event.waitUntil(
		caches.keys().then((cacheNames) => {
			return Promise.all(
				cacheNames.map((cacheName) => {
					if (cacheName !== CACHE_NAME) {
						return caches.delete(cacheName);
					}
				})
			);
		})
	);
});

self.addEventListener('install', (e) => {
	console.debug("[sw] Install " + BUILD_TIME);
	e.waitUntil(
		caches.open(CACHE_NAME).then((cache) => {
			return cache.addAll(filesToCache);
		})
	);
});

self.addEventListener('fetch', (e) => {
	e.respondWith(
		caches.match(e.request).then((response) => {
			return response || fetch(e.request);
		}
	));
});
