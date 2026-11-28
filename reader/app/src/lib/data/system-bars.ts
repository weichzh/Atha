const safeEdges = ["top", "right", "bottom", "left"] as const;
type SafeAreaInsets = Record<typeof safeEdges[number], number>;

interface WindowGeometry extends SafeAreaInsets {
  imeVisible: boolean;
}

export interface SystemBarsPort {
  postMessage(message: string): void;
  onmessage: ((event: MessageEvent<string>) => void) | null;
}

function isWindowGeometry(value: unknown): value is WindowGeometry {
  if (!value || typeof value !== "object" || !("imeVisible" in value) || typeof value.imeVisible !== "boolean") return false;
  return safeEdges.every((edge) => {
    const inset: unknown = (value as Record<string, unknown>)[edge];
    return typeof inset === "number" && Number.isFinite(inset) && inset >= 0;
  });
}

let barsMode: string | undefined;
let bodySelection = false;
let batteryPercent: number | null = null;

/** Only the Android app-origin port can supply a percentage. */
export function watchReaderBattery(onRead: (percent: number | null) => void): () => void {
  const port = window.AthaSystemBars;
  if (!port) return () => {};
  const update = () => onRead(batteryPercent);
  const request = () => port.postMessage("battery:read");
  window.addEventListener("atha-reader-battery", update);
  update();
  request();
  const timer = window.setInterval(request, 60_000);
  return () => {
    window.clearInterval(timer);
    window.removeEventListener("atha-reader-battery", update);
  };
}

export function bindSystemBarsListeners(): void {
  const port = window.AthaSystemBars;
  if (!port) return;
  let geometry: WindowGeometry | null = null;

  const sync = () => {
    if (!geometry) return;
    const { style } = document.documentElement;
    let changed = false;
    for (const edge of safeEdges) {
      const property = `--safe-area-${edge}`;
      const value = `${geometry[edge] / window.devicePixelRatio}px`;
      if (style.getPropertyValue(property) === value) continue;
      style.setProperty(property, value);
      changed = true;
    }
    if (changed) window.dispatchEvent(new Event("atha-safe-area-change"));
  };
  const subscribe = () => {
    port.postMessage("subscribe");
    if (barsMode) port.postMessage(barsMode);
    if (bodySelection) port.postMessage("selection:body");
  };

  port.onmessage = ({ data }) => {
    let next: unknown;
    try {
      next = JSON.parse(data);
    } catch {
      return;
    }
    if (next && typeof next === "object" && "batteryPercent" in next) {
      const level = next.batteryPercent;
      if (level === null || (typeof level === "number" && Number.isInteger(level) && level >= 0 && level <= 100)) {
        batteryPercent = level;
        window.dispatchEvent(new Event("atha-reader-battery"));
      }
      return;
    }
    if (!isWindowGeometry(next)) return;
    const imeHidden = geometry?.imeVisible === true && !next.imeVisible;
    geometry = next;
    sync();
    if (imeHidden) {
      const input = document.activeElement;
      if (input instanceof HTMLInputElement && input.type === "search") input.blur();
    }
  };
  window.addEventListener("resize", sync);
  window.addEventListener("pageshow", (event) => {
    if (event.persisted) subscribe();
  });
  subscribe();
}

export function syncSystemBarsMode(immersive: boolean, dark: boolean): void {
  const next = `bars:${immersive ? "immersive" : "visible"}:${dark ? "dark" : "light"}`;
  if (next === barsMode) return;
  barsMode = next;
  window.AthaSystemBars?.postMessage(next);
}

export function setNativeSelectionSuppressed(selected: boolean): void {
  if (selected === bodySelection) return;
  bodySelection = selected;
  window.AthaSystemBars?.postMessage(selected ? "selection:body" : "selection:none");
}
