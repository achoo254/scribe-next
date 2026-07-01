import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath } from "node:url";

// Web client sibling (Phase 4). Alias tới source packages/meeting-shared (chưa build riêng → import src).
// Chạy verify build từ repo root: npx vite build --config apps/web/vite.config.ts (dùng node_modules root).
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  plugins: [vue()],
  resolve: {
    alias: {
      "@onescribe/meeting-shared": fileURLToPath(
        new URL("../../packages/meeting-shared/src/index.ts", import.meta.url),
      ),
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
