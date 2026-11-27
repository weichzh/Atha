export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

let fetchBridgeInstalled = false;

export function installDemoFetchBridge(): void {
  if (fetchBridgeInstalled || isTauriRuntime() || typeof window === "undefined") return;
  fetchBridgeInstalled = true;
  const nativeFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const rawUrl =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    if (
      rawUrl.startsWith("https://atha-book.localhost/") ||
      rawUrl.startsWith("atha-book://localhost/")
    ) {
      const parsed = new URL(rawUrl.replace("atha-book://localhost/", "https://atha-book.localhost/"));
      const cleanPath = parsed.pathname.replace(/^\/+/, "");
      const localUrl = new URL(cleanPath + parsed.search, new URL("./", window.location.href));
      return nativeFetch(localUrl.href, init);
    }
    return nativeFetch(input, init);
  };
}

export function hasReaderLaunchParams(search: string = typeof location !== "undefined" ? location.search : ""): boolean {
  const params = new URLSearchParams(search);
  return params.has("manifest") || params.has("book") || params.has("entry");
}
