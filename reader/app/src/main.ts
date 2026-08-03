import { invoke } from "@tauri-apps/api/core";
import { mount } from "svelte";

import App from "./App.svelte";
import "./shell.css";

const target = document.querySelector<HTMLElement>("#app");
if (!target) throw new Error("missing-app-root");

mount(App, { target });

if (window.__TAURI_INTERNALS__) {
  let pending = Promise.resolve();
  window.athaReaderIpc = {
    postMessage(message) {
      pending = pending.then(() => invoke<void>("reader_event", { message })).catch(() =>
        console.error("Atha reader telemetry failed"),
      );
    },
  };
}

await import("virtual:atha-reader-runtime");
