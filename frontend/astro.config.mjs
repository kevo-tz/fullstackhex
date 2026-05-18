// @ts-check
import { defineConfig } from 'astro/config';
import node from '@astrojs/node';

export default defineConfig({
  output: 'server',
  adapter: node({ mode: 'standalone' }),
  vite: {
    server: {
      proxy: {
        '/api': {
          target: 'http://localhost:8001',
          changeOrigin: true,
          ws: true,
          rewrite: (path) => path.replace(/^\/api/, ''),
        },
      },
    },
  },
  security: {
    csp: {
      scriptDirective: { resources: ["'self'"] },
      styleDirective: { resources: ["'self'", "'unsafe-inline'"] },
    },
  },
});
