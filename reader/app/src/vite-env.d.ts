/// <reference types="svelte" />
/// <reference types="vite/client" />

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    __athaFlushOnExit?: () => void;
    __athaHandleBack?: () => boolean;
    athaMessages?: unknown;
    AthaSystemBars?: { getSafeAreaInsets?: () => string };
  }

  var readerPixels: ((value: number) => number) | undefined;
  var parseSafeXhtml: ((source: string) => Document | null) | undefined;
}

export {};
