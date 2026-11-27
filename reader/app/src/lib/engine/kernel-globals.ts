import { parseSafeXhtml } from "../../../../web/content.mjs";

let installed = false;

export function ensureKernelGlobals(): void {
  if (installed) return;
  installed = true;
  globalThis.readerPixels = (value: number): number => {
    const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
    return Number(value) * dpr;
  };
  globalThis.parseSafeXhtml = parseSafeXhtml;
}
