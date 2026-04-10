import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		port: 5173,
		proxy: {
			'/api': {
				target: 'http://localhost:8080',
				// Inject X-Forwarded-For so the rate limiter (SmartIpKeyExtractor)
				// can extract a client IP from header instead of ConnectInfo.
				headers: { 'X-Forwarded-For': '127.0.0.1' }
			},
			'/auth': {
				target: 'http://localhost:8080',
				headers: { 'X-Forwarded-For': '127.0.0.1' }
			}
		}
	}
});
