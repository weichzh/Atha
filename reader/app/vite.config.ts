import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig, type Plugin } from "vite";

const appRoot = fileURLToPath(new URL(".", import.meta.url));
const readerRoot = resolve(appRoot, "..");
const runtimeId = "virtual:atha-reader-runtime";
const resolvedRuntimeId = `\0${runtimeId}`;
const runtimeSources = [
  "content.mjs",
  "locator.mjs",
  "style-module-package.mjs",
  "preferences.mjs",
  "pagination.mjs",
  "session.mjs",
  "navigation.mjs",
  "interaction.mjs",
  "content-actions.mjs",
  "structured-actions.mjs",
  "reader-state.mjs",
  "bookmarks.mjs",
  "search.mjs",
  "annotation-store.mjs",
  "message-store.mjs",
  "annotations.mjs",
  "conversations.mjs",
  "diagnostics.mjs",
  "app.mjs",
];

function readerRuntime(): Plugin {
  return {
    name: "atha-reader-runtime",
    resolveId(id) {
      return id === runtimeId ? resolvedRuntimeId : undefined;
    },
    load(id) {
      if (id !== resolvedRuntimeId) return undefined;
      return runtimeSources
        .map((file) => readFileSync(resolve(readerRoot, "web", file), "utf8"))
        .join("\n");
    },
  };
}

export default defineConfig({
  base: "./",
  plugins: [svelte(), readerRuntime()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2023",
    sourcemap: true,
  },
});
