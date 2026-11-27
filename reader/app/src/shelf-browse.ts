import type { LibraryBook } from "./library";
import type { readShelfReading } from "./shelf-reading";

export const shelfSorts = [
  ["title", "书名 A–Z"],
  ["title-desc", "书名 Z–A"],
  ["author", "作者"],
  ["recent", "最近阅读"],
  ["added", "最近添加"],
  ["progress", "阅读进度"],
] as const;
export type ShelfSort = (typeof shelfSorts)[number][0];
export type ShelfLayout = "grid" | "list";

export const shelfFacets = {
  status: "阅读状态",
  subjects: "主题",
  tags: "标签",
  series: "系列",
  authors: "作者",
} as const;
export type Facet = keyof typeof shelfFacets;
export const facetKeys = Object.keys(shelfFacets) as Facet[];

export const readingLabels = {
  unread: "未读",
  reading: "在读",
  finished: "已读完",
  unknown: "进度未知",
} as const;
export type ShelfFilters = Record<Facet, string[]> & { favorite: boolean };

export function emptyFilters(): ShelfFilters {
  return { favorite: false, status: [], subjects: [], tags: [], series: [], authors: [] };
}

export function filterCount(filters: ShelfFilters): number {
  return Number(filters.favorite) + facetKeys.reduce((count, key) => count + filters[key].length, 0);
}

export function toggleFacet(filters: ShelfFilters, facet: Facet, value: string): ShelfFilters {
  const values = filters[facet];
  return {
    ...filters,
    [facet]: values.includes(value) ? values.filter((item) => item !== value) : [...values, value],
  };
}

const collator = new Intl.Collator("zh-Hans-CN-u-co-pinyin", { numeric: true, sensitivity: "base" });
export const shelfLetters = [..."ABCDEFGHIJKLMNOPQRSTUVWXYZ", "0–9", "#"];

const hanIndex = "ABCDEFGHJKLMNOPQRSTWXYZ";
const marker = (initial: string) => `\uFDD0${initial}`;
// UTS #35, CJK Index Markers: the pinyin collation has built-in A–Z boundaries.
// Engines without those boundaries keep Chinese collation within the "other" bucket.
const hasHanIndex = collator.compare(marker("A"), "阿") < 0 && collator.compare("阿", marker("B")) < 0;

export function shelfInitial(title: string): string {
  const [first] = title.normalize("NFKC").replace(/^[\s\p{P}\p{S}]+/u, "");
  if (!first) return "#";
  if (/\p{Nd}/u.test(first)) return "0–9";
  if (/\p{Script=Han}/u.test(first)) {
    // 咗 is the final tailored pinyin character in CLDR's zh collation;
    // unclassified Han sort after it, not among the Z titles.
    if (!hasHanIndex || collator.compare(first, "咗") > 0) return "#";
    for (let index = hanIndex.length - 1; index >= 0; index--) {
      if (collator.compare(first, marker(hanIndex[index])) >= 0) return hanIndex[index];
    }
    return "#";
  }
  const latin = first.normalize("NFD")[0].toUpperCase();
  return /^[A-Z]$/.test(latin) ? latin : "#";
}

type ShelfReading = ReturnType<typeof readShelfReading>;
export type ReadingFact = NonNullable<ReturnType<ShelfReading["get"]>>;

export function indexShelf(books: LibraryBook[], reading: ShelfReading) {
  return books.map((book) => {
    const fact = reading.get(book.id);
    const values: Record<Facet, readonly string[]> = {
      status: [fact?.status ?? "unknown"],
      subjects: book.subjects ?? [],
      tags: book.tags ?? [],
      series: book.series ?? [],
      authors: book.authors,
    };
    const title = book.title.normalize("NFKC").replace(/^[\s\p{P}\p{S}]+/u, "") || book.title;
    return {
      book, fact, values, title,
      initial: shelfInitial(book.title),
      author: values.authors.join(" / "),
      searchable: [book.title, ...values.authors, ...values.tags, ...values.subjects, ...values.series]
        .join("\n").normalize("NFKC").toLocaleLowerCase(),
    };
  });
}
export type ShelfEntry = ReturnType<typeof indexShelf>[number];

function titleOrder(left: ShelfEntry, right: ShelfEntry): number {
  return shelfLetters.indexOf(left.initial) - shelfLetters.indexOf(right.initial)
    || collator.compare(left.title, right.title)
    || left.book.id.localeCompare(right.book.id);
}

function recentOrder(left: ShelfEntry, right: ShelfEntry): number {
  // A calendar day from history is not a timestamp. On the same date show the
  // precise records first, then use title order for entries without a time.
  return (right.fact?.lastReadDate ?? "").localeCompare(left.fact?.lastReadDate ?? "")
    || (right.fact?.lastReadAt ?? 0) - (left.fact?.lastReadAt ?? 0);
}

export function recentShelf(entries: ShelfEntry[]): ShelfEntry[] {
  return entries.filter(({ fact }) => fact?.lastReadAt != null || fact?.lastReadDate != null)
    .sort((left, right) => recentOrder(left, right) || titleOrder(left, right)).slice(0, 12);
}

const terms = (query: string) => query.trim().normalize("NFKC").toLocaleLowerCase().split(/\s+/u).filter(Boolean);
const matchesText = (entry: ShelfEntry, words: string[]) => words.every((word) => entry.searchable.includes(word));
const matchesFacet = (entry: ShelfEntry, values: string[], key: Facet) =>
  values.length === 0 || values.some((value) => entry.values[key].includes(value));

export function browseShelf(entries: ShelfEntry[], query: string, filters: ShelfFilters, sort: ShelfSort) {
  const words = terms(query);
  return entries.filter((entry) => matchesText(entry, words)
    && (!filters.favorite || entry.book.favorite)
    && facetKeys.every((key) => matchesFacet(entry, filters[key], key)))
    .sort((left, right) => {
      switch (sort) {
        case "title-desc": return -titleOrder(left, right);
        case "author": return (left.author && right.author
          ? collator.compare(left.author, right.author)
          : Number(!left.author) - Number(!right.author)) || titleOrder(left, right);
        case "recent": return recentOrder(left, right) || titleOrder(left, right);
        case "progress":
        case "added": return right.book.importedAt - left.book.importedAt || titleOrder(left, right);
        default: return titleOrder(left, right);
      }
    });
}

export function quickShelfFacets(books: LibraryBook[]) {
  const result: { key: "tags" | "subjects"; value: string }[] = [];
  const seen = new Set<string>();
  for (const key of ["tags", "subjects"] as const) {
    const counts = new Map<string, number>();
    for (const book of books) {
      for (const value of new Set(book[key])) counts.set(value, (counts.get(value) ?? 0) + 1);
    }
    const ordered = [...counts.keys()]
      .sort((a, b) => counts.get(b)! - counts.get(a)! || collator.compare(a, b) || a.localeCompare(b));
    for (const value of ordered) {
      if (seen.has(value)) continue;
      result.push({ key, value });
      seen.add(value);
      if (result.length === 7) return result;
    }
  }
  return result;
}

export function shelfFacetOptions(entries: ShelfEntry[], query: string, filters: ShelfFilters) {
  const words = terms(query);
  const counters = Object.fromEntries(facetKeys.map((key) => [key, new Map<string, number>()])) as Record<Facet, Map<string, number>>;
  for (const value of Object.keys(readingLabels)) counters.status.set(value, 0);
  let matches = 0;
  for (const entry of entries) {
    const eligible = matchesText(entry, words) && (!filters.favorite || entry.book.favorite);
    const excluded = eligible ? facetKeys.filter((key) => !matchesFacet(entry, filters[key], key)) : [];
    if (eligible && excluded.length === 0) matches++;
    for (const key of facetKeys) {
      const include = eligible && (excluded.length === 0 || (excluded.length === 1 && excluded[0] === key));
      for (const value of new Set(entry.values[key])) {
        const group = counters[key];
        group.set(value, (group.get(value) ?? 0) + Number(include));
      }
    }
  }
  const groups = facetKeys.map((key) => {
    const group = counters[key];
    for (const value of filters[key]) if (!group.has(value)) group.set(value, 0);
    const values = [...group.keys()];
    if (key !== "status") values.sort(collator.compare);
    return { key, options: values.map((value) => ({ value, count: group.get(value) ?? 0 })) };
  });
  return { groups, matches };
}

export function facetLabel(key: Facet, value: string): string {
  return key === "status" ? readingLabels[value as keyof typeof readingLabels] : value;
}

export function progressShelf(entries: ShelfEntry[]) {
  const result = { reading: [] as ShelfEntry[], unread: [] as ShelfEntry[], unknown: [] as ShelfEntry[] };
  for (const entry of entries) {
    const status = entry.fact?.status ?? "unknown";
    result[status === "finished" ? "reading" : status].push(entry);
  }
  return result;
}

export function parseOrganizationField(value: string): string[] {
  return [...new Set(value.split(/\r\n?|\n/u).map((item) => item.trim()).filter(Boolean))];
}
