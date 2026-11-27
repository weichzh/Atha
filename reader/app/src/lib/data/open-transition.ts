import { readReaderAppearance } from "../../library.ts";

export interface ReaderLaunchLabel {
  bookId: string;
  title: string;
  author: string;
}

const TRANSITION_KEY = "atha.open-transition.v1";
const TRANSITION_TTL_MS = 5000;

// Only the label crosses documents; an on-screen rectangle belongs to its own document.
export function saveReaderLaunchLabel(payload: ReaderLaunchLabel): void {
  if (typeof window === "undefined") return;
  try {
    const full = { ...payload, ts: Date.now() };
    window.sessionStorage.setItem(TRANSITION_KEY, JSON.stringify(full));
  } catch {
    // Session storage is optional
  }
}

export function consumeReaderLaunchLabel(bookId: string): ReaderLaunchLabel | null {
  try {
    const raw = window.sessionStorage.getItem(TRANSITION_KEY);
    window.sessionStorage.removeItem(TRANSITION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as ReaderLaunchLabel & { ts: number };
    const matchesBook = parsed?.bookId === bookId ||
      (/^[a-f0-9]{16}$/.test(bookId) && /^[a-f0-9]{64}$/.test(parsed?.bookId) && parsed.bookId.startsWith(bookId));
    if (!matchesBook || typeof parsed.ts !== "number" || Date.now() - parsed.ts > TRANSITION_TTL_MS) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export interface OpeningCover {
  art: HTMLElement;
  rect: DOMRect;
}

export interface OpeningPaperMeasure {
  left: number;
  width: number;
}

export function openingCoverFrame(rect: DOMRect, width: number, height: number) {
  const scale = Math.min((width * 0.72) / rect.width, (height * 0.61) / rect.height, 365 / rect.width);
  const size = { width: rect.width * scale, height: rect.height * scale };
  return {
    left: (width - size.width) / 2,
    top: height * 0.47 - size.height / 2,
    ...size,
    scale,
  };
}

/** Match the laid-out paper's measure to the closed book without reflow or stretching. */
export function openingPaperApproach(
  cover: ReturnType<typeof openingCoverFrame>,
  paper: OpeningPaperMeasure,
) {
  const scale = cover.width / paper.width;
  return {
    fromX: cover.left - paper.left * scale,
    scale,
    coverToX: paper.left - cover.left,
    coverToScale: cover.scale / scale,
  };
}

export function captureOpeningCover(trigger?: HTMLElement): OpeningCover | null {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return null;
  const stage = trigger?.querySelector<HTMLElement>(".cover-stage");
  const cover = stage?.querySelector<HTMLElement>(".cover-art");
  const rect = cover?.getBoundingClientRect();
  if (!stage || !cover || !rect?.width || !rect.height ||
    rect.right <= 0 || rect.bottom <= 0 || rect.left >= innerWidth || rect.top >= innerHeight) return null;
  // The shelf may be showing a generated cover or a failed image, not the catalog URL.
  const art = stage.cloneNode(false) as HTMLElement;
  art.append(cover.cloneNode(true));
  const image = art.querySelector("img");
  if (image && !image.classList.contains("loaded")) image.remove();
  else image?.removeAttribute("loading");
  return { art, rect };
}

export function readOpeningTheme(): "light" | "paper" | "dark" {
  const params = new URLSearchParams(location.search);
  const { theme } = readReaderAppearance(
    params.has("verify") && !params.has("persist") ? null : undefined,
    params.has("state-probe"),
  );
  if (theme === "paper" || theme === "dark" || theme === "light") return theme;
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
