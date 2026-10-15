import { defineConfig, mergeConfig } from 'vite';
import base from './vite.config.ts';

export default defineConfig((environment) =>
  mergeConfig(typeof base === 'function' ? base(environment) : base, {
    preview: {
      proxy: {
        '/api': 'http://127.0.0.1:18092',
        '/ws': { target: 'http://127.0.0.1:18092', ws: true },
      },
    },
  }),
);
