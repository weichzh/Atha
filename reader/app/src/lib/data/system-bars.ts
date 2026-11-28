const safeEdges = ["top", "right", "bottom", "left"] as const;
type SafeAreaInsets = Record<typeof safeEdges[number], number>;

interface WindowGeometry extends SafeAreaInsets {
  imeVisible: boolean;
}

export interface SystemBarsPort {
  postMessage(message: string): void;
  onmessage: ((event: MessageEvent<string>) => void) | null;
}

function parseWindowGeometry(raw: string): WindowGeometry | null {
  try {
    const value = JSON.parse(raw) as WindowGeometry | null;
    if (value && typeof value.imeVisible === "boolean" &&
        safeEdges.every((edge) => Number.isFinite(value[edge]) && value[edge] >= 0)) {
      return value;
    }
  } catch {
    // A malformed message does not change the last known geometry.
  }
  return null;
}

let barsMode: string | undefined;
let bodySelection = false;

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
    const next = parseWindowGeometry(data);
    if (!next) return;
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
