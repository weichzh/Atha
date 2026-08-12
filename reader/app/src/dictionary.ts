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
