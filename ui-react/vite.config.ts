import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The desktop shell loads ../ui-react/dist; the demo mode runs with `npm run
// dev` and talks to the same in-memory mock kernel as the old frontend.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    // One digest-hashed bundle per entry; Tauri serves whatever lands here.
    assetsDir: "assets",
  },
  server: {
    port: 5183,
    strictPort: true,
  },
});
