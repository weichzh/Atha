import { invoke } from "@tauri-apps/api/core";

export type DictionaryFormat = "kindle-mobi6" | "mdict";

export interface LocalDictionary {
  id: string;
  title: string;
  format: DictionaryFormat;
  entryCount: number;
  resourceCount: number;
  importedAt: number;
}

export interface DictionaryLookup {
  dictionaryId: string;
  headword: string;
  definition: string;
  definitionHtml: string;
}

export const dictionaryFontScales = [0.85, 1, 1.15, 1.3, 1.5, 1.75] as const;
export type DictionaryFontScale = (typeof dictionaryFontScales)[number];

export interface DictionaryPreferences {
  dictionaryId: string;
  fontScale: DictionaryFontScale;
}

interface StorageReader {
  getItem(key: string): string | null;
}

interface StorageWriter {
  setItem(key: string, value: string): void;
}

const preferencesKey = "atha.reader.dictionary.preferences.v1";
const defaultPreferences: DictionaryPreferences = { dictionaryId: "", fontScale: 1 };

function validDictionaryId(value: unknown): value is string {
  return typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
}

function validFontScale(value: unknown): value is DictionaryFontScale {
  return dictionaryFontScales.includes(value as DictionaryFontScale);
}

export function readDictionaryPreferences(storage?: StorageReader | null): DictionaryPreferences {
  try {
    const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
    const raw = target?.getItem(preferencesKey);
    if (!raw || raw.length > 1024) return { ...defaultPreferences };
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object" || (value as { schema?: unknown }).schema !== 1) {
      return { ...defaultPreferences };
    }
    const stored = value as { dictionaryId?: unknown; fontScale?: unknown };
    return {
      dictionaryId: validDictionaryId(stored.dictionaryId) ? stored.dictionaryId : "",
      fontScale: validFontScale(stored.fontScale) ? stored.fontScale : 1,
    };
  } catch {
    return { ...defaultPreferences };
  }
}

export function writeDictionaryPreferences(
  preferences: DictionaryPreferences,
  storage?: StorageWriter | null,
): boolean {
  try {
    const target = storage ?? (typeof window === "undefined" ? null : window.localStorage);
    if (!target) return false;
    target.setItem(preferencesKey, JSON.stringify({ schema: 1, ...preferences }));
    return true;
  } catch {
    return false;
  }
}

export const dictionaryAvailable =
  typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

export function listDictionaries(): Promise<LocalDictionary[]> {
  return dictionaryAvailable ? invoke("list_local_dictionaries") : Promise.resolve([]);
}

export function importDictionary(): Promise<LocalDictionary[] | null> {
  return invoke("import_local_dictionary");
}

export function lookupDictionary(
  dictionaryId: string,
  query: string,
): Promise<DictionaryLookup | null> {
  return invoke("lookup_local_dictionary", { dictionaryId, query });
}

export function removeDictionary(dictionaryId: string): Promise<LocalDictionary[]> {
  return invoke("remove_local_dictionary", { dictionaryId });
}

export function dictionaryErrorMessage(code: unknown): string {
  switch (String(code)) {
    case "invalid-dictionary-import":
      return "请选择一个 MDX（可同时选择最多四个 MDD），或一个经典 Kindle MOBI 词典。";
    case "invalid-dictionary-source":
    case "corrupt-dictionary-source":
      return "词典文件无法读取或已经损坏。";
    case "dictionary-source-too-large":
    case "dictionary-definition-too-large":
    case "dictionary-resource-too-large":
      return "词典内容超过安全上限。";
    case "unsupported-dictionary":
      return "暂不支持这种词典。";
    case "dictionary-link-depth":
      return "词条链接层级过深。";
    case "invalid-dictionary-query":
      return "请选择不超过 128 个字符的正文。";
    default:
      return "词典暂时不可用。";
  }
}
