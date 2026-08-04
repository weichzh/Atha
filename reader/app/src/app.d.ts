declare module "virtual:atha-reader-runtime";

interface Window {
  __TAURI_INTERNALS__?: unknown;
  athaReaderIpc?: {
    postMessage(message: string): void;
  };
  athaMessages?: import("./messages").MessageClient;
  ipc?: {
    postMessage(message: string): void;
  };
}
