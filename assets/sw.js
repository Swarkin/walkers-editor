const cacheName = 'walkers-editor';
const filesToCache = [
	'./',
	'./index.html',
	'./walkers-editor.js',
	'./walkers-editor_bg.wasm',
];

self.addEventListener('install', (e) => {
	e.waitUntil(
		caches.open(cacheName).then((cache) => {
			return cache.addAll(filesToCache);
		})
	);
});

self.addEventListener('fetch', (e) => {
	e.respondWith(
		caches.match(e.request).then((response) => {
			return response || fetch(e.request);
		})
	);
});
