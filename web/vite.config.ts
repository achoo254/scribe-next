import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Dev: proxy /api → BE axum (127.0.0.1:8787) để FE + BE cùng origin (cookie same-site, không CORS).
// Prod: nginx đảm nhận cùng origin (FE dist ở /, /api proxy BE).
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: "http://127.0.0.1:8787", changeOrigin: true },
    },
  },
});
