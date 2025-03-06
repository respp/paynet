import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig(async () => ({
  plugins: [tailwindcss()],
  root: "src",

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
