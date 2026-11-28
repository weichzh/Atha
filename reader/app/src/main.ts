import "./styles/tokens.css";
import "./styles/app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { hasReaderLaunchParams, installDemoFetchBridge, isTauriRuntime } from "./lib/data/runtime.ts";
import { bindSystemBarsListeners, syncSystemBarsMode } from "./lib/data/system-bars.ts";
import { readAppTheme, readReaderAppearance } from "./library.ts";
import { messageClient } from "./messages.ts";

const libraryReturnKey = "atha.reader.library-return-origin";

function appHostOrigin(loc: Pick<Location, "protocol" | "host">): string {
  return `${loc.protocol}//${loc.host}`;
}

export function isExactAppLibraryRoot(url: Pick<URL, "pathname" | "search" | "hash" | "href">): boolean {
  const normalized = new URL("/", url.href).href;
  const bareOrigin = normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
  return (
    (url.pathname === "/" || url.pathname === "") &&
    !url.search &&
    !url.hash &&
    (url.href === normalized || url.href === bareOrigin)
  );
}

function consumeLibraryReturn(): boolean {
  try {
    const origin = sessionStorage.getItem(libraryReturnKey);
    sessionStorage.removeItem(libraryReturnKey);
    return origin === appHostOrigin(location);
  } catch {
    return false;
  }
}

function trackLibraryDeparture() {
  try {
    sessionStorage.removeItem(libraryReturnKey);
  } catch {
    return;
  }
  window.addEventListener("pagehide", () => {
    try {
      if (isExactAppLibraryRoot(location)) sessionStorage.setItem(libraryReturnKey, appHostOrigin(location));
      else sessionStorage.removeItem(libraryReturnKey);
    } catch {
      // Fallback handled in exit handler when sessionStorage is unavailable.
    }
  });
}

if (isTauriRuntime()) {
  window.athaMessages = messageClient;
} else {
  installDemoFetchBridge();
}

const root = document.documentElement;
const readerRoute = hasReaderLaunchParams();
const hasLibraryReturn = readerRoute && consumeLibraryReturn();
if (!readerRoute) trackLibraryDeparture();

const params = new URLSearchParams(location.search);
const appearance = readerRoute
  ? readReaderAppearance(params.has("verify") && !params.has("persist") ? null : undefined, params.has("state-probe"))
  : { theme: readAppTheme(), brightness: 100 };
const tone =
  appearance.theme === "dark" ||
  (appearance.theme === "system" && matchMedia("(prefers-color-scheme: dark)").matches)
    ? "dark"
    : "light";

if (readerRoute) {
  root.dataset.theme = appearance.theme;
  root.dataset.themeTone = tone;
  root.dataset.appTheme = tone;
  root.style.setProperty("--reader-brightness", String(appearance.brightness / 100));
} else {
  root.dataset.appTheme = tone;
  root.dataset.appTone = tone;
}

bindSystemBarsListeners();
syncSystemBarsMode(false, tone === "dark");

const app = mount(App, {
  target: document.getElementById("app")!,
  props: { hasLibraryReturn },
});

export default app;
