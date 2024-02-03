import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import environment from 'vite-plugin-environment';
import dotenv from 'dotenv';

dotenv.config();

// https://vitejs.dev/config/
export default defineConfig({
  optimizeDeps: {
    esbuildOptions: {
      define: {
        global: 'globalThis',
      },
    },
  },
  preview: {
    proxy: {},
  },
  server: {
    // Local IC replica proxy
    proxy: {
      '/api': {
        target: 'http://localhost:4943',
        changeOrigin: true,
      
      },
    },
  },
  plugins: [
    react(),
    environment('all', { prefix: 'CANISTER_' }),
    environment('all', { prefix: 'DFX_' }),
    environment({ GITPOD_WORKSPACE_URL: '' }),
  ],
});
