import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const appRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  base: "./",
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    fs: {
      allow: [resolve(appRoot, "..")],
    },
  },
  resolve: {
    dedupe: [
      "svelte",
      "bits-ui",
      "@floating-ui/dom",
      "@lucide/svelte",
      "@tauri-apps/api",
      "@tiptap/core",
      "@tiptap/pm",
    ],
  },
  build: {
    target: "es2023",
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("node_modules/markdown-it") || id.includes("node_modules/prosemirror-markdown")) {
            return "vendor-markdown";
          }
          if (id.includes("node_modules/@tiptap") || id.includes("node_modules/prosemirror")) {
            return "vendor-editor";
          }
          if (id.includes("node_modules/bits-ui") || id.includes("node_modules/@internationalized")) {
            return "vendor-ui";
          }
          if (id.includes("node_modules/@lucide")) {
            return "vendor-icons";
          }
          return undefined;
        },
      },
    },
  },
});
