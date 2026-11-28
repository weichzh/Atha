export type ShelfMobileColumns = 2 | 3 | 4;
export type ShelfDesktopDensity = "comfortable" | "standard" | "compact";
export type PageTurnStyle = "slide" | "cover" | "curl" | "scroll";
export type ReaderChromeStyle = "classic" | "minimal";

export interface UiPreferences {
  shelfMobileColumns: ShelfMobileColumns;
  shelfDesktopDensity: ShelfDesktopDensity;
  shelfShowMeta: boolean;
  shelfDockCollapsed: boolean;
  pageTurnStyle: PageTurnStyle;
  readerChromeStyle: ReaderChromeStyle;
  gesturePullBookmark: boolean;
  gestureSwipeUpNotes: boolean;
}

const STORAGE_KEY = "atha.ui.prefs.v1";

const DEFAULTS: UiPreferences = {
  shelfMobileColumns: 3,
  shelfDesktopDensity: "standard",
  shelfShowMeta: true,
  shelfDockCollapsed: false,
  pageTurnStyle: "slide",
  readerChromeStyle: "classic",
  gesturePullBookmark: true,
  gestureSwipeUpNotes: true,
};

function sanitize(raw: unknown): UiPreferences {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return { ...DEFAULTS };
  const obj = raw as Record<string, unknown>;
  return {
    shelfMobileColumns: [2, 3, 4].includes(Number(obj.shelfMobileColumns))
      ? (Number(obj.shelfMobileColumns) as ShelfMobileColumns)
      : DEFAULTS.shelfMobileColumns,
    shelfDesktopDensity: ["comfortable", "standard", "compact"].includes(String(obj.shelfDesktopDensity))
      ? (String(obj.shelfDesktopDensity) as ShelfDesktopDensity)
      : DEFAULTS.shelfDesktopDensity,
    shelfShowMeta: typeof obj.shelfShowMeta === "boolean" ? obj.shelfShowMeta : DEFAULTS.shelfShowMeta,
    shelfDockCollapsed:
      typeof obj.shelfDockCollapsed === "boolean" ? obj.shelfDockCollapsed : DEFAULTS.shelfDockCollapsed,
    pageTurnStyle: ["slide", "cover", "curl", "scroll"].includes(String(obj.pageTurnStyle))
      ? (String(obj.pageTurnStyle) as PageTurnStyle)
      : DEFAULTS.pageTurnStyle,
    readerChromeStyle: ["classic", "minimal"].includes(String(obj.readerChromeStyle))
      ? (String(obj.readerChromeStyle) as ReaderChromeStyle)
      : DEFAULTS.readerChromeStyle,
    gesturePullBookmark:
      typeof obj.gesturePullBookmark === "boolean" ? obj.gesturePullBookmark : DEFAULTS.gesturePullBookmark,
    gestureSwipeUpNotes:
      typeof obj.gestureSwipeUpNotes === "boolean" ? obj.gestureSwipeUpNotes : DEFAULTS.gestureSwipeUpNotes,
  };
}

function loadInitial(): UiPreferences {
  if (typeof window === "undefined") return { ...DEFAULTS };
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return raw ? sanitize(JSON.parse(raw)) : { ...DEFAULTS };
  } catch {
    return { ...DEFAULTS };
  }
}

class UiPrefsStore {
  state = $state<UiPreferences>(loadInitial());

  update(patch: Partial<UiPreferences>): void {
    const next = sanitize({ ...this.state, ...patch });
    this.state = next;
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Ignore quota/storage errors safely
    }
  }

  reload(): void {
    this.state = loadInitial();
  }
}

export const uiPrefs = new UiPrefsStore();
