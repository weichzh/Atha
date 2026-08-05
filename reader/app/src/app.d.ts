declare module "virtual:atha-reader-runtime";

interface Window {
  __TAURI_INTERNALS__?: unknown;
  athaReaderIpc?: {
    postMessage(message: string): void;
  };
  athaEnsureMessageComposer?: () => Promise<void>;
  athaMessages?: import("./messages").MessageClient;
  athaMessageComposer?: {
    clear(): void;
    collapse(): void;
    focus(): void;
    render(target: HTMLElement, contentJson: string, fallback: string): void;
    setValue(contentJson: string, fallback: string): void;
    value(): {
      text: string;
      richText: import("./messages").RichTextInput;
    } | null;
  };
  ipc?: {
    postMessage(message: string): void;
  };
}
