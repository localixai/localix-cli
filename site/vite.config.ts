import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The site is served from the domain root (https://llx.sh/); public/ keeps every static URL as it was.
export default defineConfig({
  plugins: [react()],
  base: '/',
  build: {
    outDir: 'dist',
    target: 'es2022',
  },
});
