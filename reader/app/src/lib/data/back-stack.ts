export type BackSource = "back" | "escape";
export type BackHandler = (source: BackSource) => boolean;

const handlers: BackHandler[] = [];

/** Register a LIFO back-navigation handler. Returning `true` consumes the event. */
export function registerBackHandler(handler: BackHandler): () => void {
  handlers.push(handler);
  return () => {
    const idx = handlers.lastIndexOf(handler);
    if (idx >= 0) handlers.splice(idx, 1);
  };
}

/** Dispatch a navigation back or Escape action from top-most handler down. */
export function dispatchBackNavigation(source: BackSource = "back"): boolean {
  for (let i = handlers.length - 1; i >= 0; i -= 1) {
    try {
      if (handlers[i](source)) return true;
    } catch {
      // Ignore faulty handler and continue down stack
    }
  }
  return false;
}

/** Install Android bridge entrypoint (`window.__athaHandleBack`) and desktop Escape shortcut. */
export function installGlobalBackDispatcher(): () => void {
  if (typeof window === "undefined") return () => {};
  window.__athaHandleBack = () => dispatchBackNavigation("back");
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || event.defaultPrevented || event.isComposing) return;
    if (dispatchBackNavigation("escape")) {
      event.preventDefault();
    }
  };
  window.addEventListener("keydown", onKeyDown);
  return () => {
    delete window.__athaHandleBack;
    window.removeEventListener("keydown", onKeyDown);
  };
}
