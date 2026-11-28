import { createStyleModulePackageCodec } from "../../../../web/style-module-package.mjs";
import { validateUserStylesheet } from "../../../../web/user-stylesheet.mjs";
import type {
  PreferencesSnapshot,
  ReaderApplicationPreferences,
  ReaderBookPreferences,
  ReaderEffectivePreferences,
  StyleModule,
} from "../types.ts";

export const DEFAULT_APPLICATION_PREFS: ReaderApplicationPreferences = Object.freeze({
  theme: "system",
  brightness: 100,
  fontSize: 19,
  fontFamily: "book",
  density: "standard",
});

export const DEFAULT_BOOK_PREFS: ReaderBookPreferences = Object.freeze({
  sourceStyles: true,
  userStylesEnabled: true,
  readingMode: "paged",
  pageMargin: "standard",
  paragraphIndent: "none",
  paragraphSpacing: "book",
  styleModules: [],
});

const MARGIN_PX: Record<ReaderBookPreferences["pageMargin"], number> = {
  narrow: 24,
  standard: 32,
  wide: 48,
};

export const stylePackageCodec = createStyleModulePackageCodec(validateUserStylesheet);

function validateApp(value: unknown): ReaderApplicationPreferences {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("invalid-application-preferences");
  }
  const v = value as Record<string, unknown>;
  const theme = v.theme ?? "system";
  const brightness = v.brightness ?? 100;
  const fontSize = v.fontSize ?? 19;
  const fontFamily = v.fontFamily ?? "book";
  const density = v.density ?? "standard";
  if (
    !["system", "light", "paper", "dark"].includes(String(theme)) ||
    !Number.isInteger(brightness) ||
    Number(brightness) < 70 ||
    Number(brightness) > 120 ||
    !Number.isInteger(fontSize) ||
    Number(fontSize) < 16 ||
    Number(fontSize) > 40 ||
    !["book", "serif", "sans"].includes(String(fontFamily)) ||
    !["compact", "standard", "comfortable"].includes(String(density))
  ) {
    throw new Error("invalid-application-preferences");
  }
  return {
    theme: theme as ReaderApplicationPreferences["theme"],
    brightness: Number(brightness),
    fontSize: Number(fontSize),
    fontFamily: fontFamily as ReaderApplicationPreferences["fontFamily"],
    density: density as ReaderApplicationPreferences["density"],
  };
}

function validateBook(value: unknown): ReaderBookPreferences {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("invalid-book-preferences");
  }
  const v = value as Record<string, unknown>;
  const sourceStyles = v.sourceStyles ?? true;
  const userStylesEnabled = v.userStylesEnabled ?? true;
  const readingMode = v.readingMode ?? "paged";
  const pageMargin = v.pageMargin ?? "standard";
  const paragraphIndent = v.paragraphIndent === "book" ? "none" : (v.paragraphIndent ?? "none");
  const paragraphSpacing = v.paragraphSpacing ?? "book";
  const rawModules = Array.isArray(v.styleModules) ? v.styleModules : [];
  if (
    typeof sourceStyles !== "boolean" ||
    typeof userStylesEnabled !== "boolean" ||
    !["paged", "scroll"].includes(String(readingMode)) ||
    !["narrow", "standard", "wide"].includes(String(pageMargin)) ||
    !["none", "two"].includes(String(paragraphIndent)) ||
    !["book", "compact", "comfortable"].includes(String(paragraphSpacing))
  ) {
    throw new Error("invalid-book-preferences");
  }
  const modules = [...stylePackageCodec.validateModules(rawModules, true)] as StyleModule[];
  return {
    sourceStyles,
    userStylesEnabled,
    readingMode: readingMode as ReaderBookPreferences["readingMode"],
    pageMargin: pageMargin as ReaderBookPreferences["pageMargin"],
    paragraphIndent: paragraphIndent as ReaderBookPreferences["paragraphIndent"],
    paragraphSpacing: paragraphSpacing as ReaderBookPreferences["paragraphSpacing"],
    styleModules: modules.map((m) => ({ ...m })),
  };
}

export function resolveEffectiveTheme(theme: ReaderApplicationPreferences["theme"]): "light" | "paper" | "dark" {
  if (theme === "system") {
    return typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return theme;
}

export function combinedUserCss(modules: StyleModule[]): string {
  return modules
    .filter((m) => m.enabled && m.css.trim().length > 0)
    .map((m) => `/* ${m.name} */\n${m.css}`)
    .join("\n\n");
}

export function createReaderPreferences({
  content,
  reader,
  host,
  frame,
  onSync,
}: {
  content: {
    book: HTMLElement;
    setStyles: (styles: {
      sourceStyles: boolean;
      userStylesEnabled: boolean;
      userStylesheet: string;
      colorScheme: "light" | "dark" | null;
    }) => void;
  };
  reader: HTMLElement;
  host: HTMLElement;
  frame: HTMLElement;
  onSync?: (snapshot: PreferencesSnapshot) => void;
}) {
  let application: ReaderApplicationPreferences = { ...DEFAULT_APPLICATION_PREFS };
  let book: ReaderBookPreferences = { ...DEFAULT_BOOK_PREFS, styleModules: [] };
  let handlers: {
    onUpdate?: (scope: "application" | "book", patch: Record<string, unknown>) => Promise<unknown>;
    onReset?: (scope: "application" | "book") => Promise<unknown>;
  } = {};

  function computeEffective(): ReaderEffectivePreferences {
    return {
      ...application,
      ...book,
      styleModules: book.styleModules.map((m) => ({ ...m })),
    };
  }

  function snapshot(): PreferencesSnapshot {
    return {
      application: { ...application },
      book: { ...book, styleModules: book.styleModules.map((m) => ({ ...m })) },
      effective: computeEffective(),
    };
  }

  function applyVisualPresentation(effective: ReaderEffectivePreferences) {
    const resolvedTheme = resolveEffectiveTheme(effective.theme);
    const canvasMap = {
      light: "#FAF9F5",
      paper: "#F4EFE4",
      dark: "#141413",
    };
    const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
    const marginPx = MARGIN_PX[effective.pageMargin];
    const padY = effective.pageMargin === "wide" ? 56 : effective.pageMargin === "narrow" ? 36 : 46;

    frame.style.setProperty("--reader-canvas", canvasMap[resolvedTheme]);
    frame.style.setProperty("--reader-brightness", String(effective.brightness / 100));
    frame.style.setProperty("--reader-dpr", String(dpr));
    frame.style.setProperty("--reader-pad-x", `${Math.round(marginPx * dpr)}px`);
    frame.style.setProperty("--reader-pad-y", `${Math.round(padY * dpr)}px`);
    frame.style.setProperty("--reader-column-gap", `${Math.round(48 * dpr)}px`);
    frame.style.setProperty(
      "--reader-col-w",
      "max(120px, min(calc(var(--reader-width, 100vw) - var(--reader-pad-x, 32px) * 2), var(--reader-content-max-width, 760px)))",
    );

    reader.dataset.readingMode = effective.readingMode;
    reader.dataset.theme = resolvedTheme;
    host.dataset.theme = resolvedTheme;
    host.dataset.fontFamily = effective.fontFamily;
    host.dataset.density = effective.density;
    host.dataset.readingMode = effective.readingMode;
    host.dataset.paragraphIndent = effective.paragraphIndent;
    host.dataset.paragraphSpacing = effective.paragraphSpacing;
    const book = content.book;
    book.dataset.theme = resolvedTheme;
    book.dataset.fontFamily = effective.fontFamily;
    book.dataset.density = effective.density;
    book.dataset.readingMode = effective.readingMode;
    book.dataset.paragraphIndent = effective.paragraphIndent;
    book.dataset.paragraphSpacing = effective.paragraphSpacing;

    content.setStyles({
      sourceStyles: effective.sourceStyles,
      userStylesEnabled: effective.userStylesEnabled,
      userStylesheet: combinedUserCss(effective.styleModules),
      colorScheme: effective.theme === "system" ? null : resolvedTheme === "dark" ? "dark" : "light",
    });

    const snap = snapshot();
    onSync?.(snap);
    return snap;
  }

  function restore(next: { application: unknown; book: unknown }): PreferencesSnapshot {
    application = validateApp(next.application);
    book = validateBook(next.book);
    return applyVisualPresentation(computeEffective());
  }

  function update(scope: "application" | "book", patch: Record<string, unknown>): PreferencesSnapshot {
    if (scope === "application") {
      application = validateApp({ ...application, ...patch });
    } else {
      book = validateBook({ ...book, ...patch });
    }
    return applyVisualPresentation(computeEffective());
  }

  function reset(scope: "application" | "book"): PreferencesSnapshot {
    if (scope === "application") {
      application = { ...DEFAULT_APPLICATION_PREFS };
    } else {
      book = { ...DEFAULT_BOOK_PREFS, styleModules: [] };
    }
    return applyVisualPresentation(computeEffective());
  }

  function bind(nextHandlers: typeof handlers) {
    handlers = nextHandlers;
  }

  return Object.freeze({
    bind,
    handlers: () => handlers,
    reset,
    restore,
    snapshot,
    update,
  });
}
