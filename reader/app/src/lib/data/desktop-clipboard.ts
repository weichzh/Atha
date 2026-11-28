import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./runtime.ts";

export function usesDesktopAppMenu(): boolean {
  return isTauriRuntime() && !/\bAndroid\b/.test(navigator.userAgent);
}

export function readDesktopClipboard(): Promise<string> {
  return invoke<string>("plugin:clipboard-manager|read_text");
}

export function copyAppText(text: string): Promise<void> {
  return usesDesktopAppMenu()
    ? invoke<void>("plugin:clipboard-manager|write_text", { text })
    : navigator.clipboard.writeText(text);
}
