import { emptyFilters, facetKeys, readingLabels, shelfSorts, type ShelfFilters, type ShelfLayout, type ShelfSort } from "./shelf-browse.ts";

const shelfSessionKey = "atha.app.shelf.session.v1";
export type ShelfOrigin = {
  id: string;
  area: "catalog" | "recent";
  offset: number;
  horizontalOffset?: number;
  control?: "actions";
};
export type ShelfPosition = {
  y: number;
  recentX: number;
  tagsX: number;
  conditionsY: number;
  anchor?: ShelfOrigin;
  focus?: ShelfOrigin;
};
export type ShelfSession = {
  query: string;
  filters: ShelfFilters;
  sort: ShelfSort;
  layout: ShelfLayout;
  position: ShelfPosition;
};

export function emptyShelfPosition(): ShelfPosition {
  return { y: 0, recentX: 0, tagsX: 0, conditionsY: 0 };
}

const coordinate = (value: unknown) => typeof value === "number" && Number.isFinite(value) ? value : 0;
function readOrigin(value: unknown): ShelfOrigin | undefined {
  if (!value || typeof value !== "object") return;
  const { id, area, offset, horizontalOffset, control } = value as Partial<ShelfOrigin>;
  if (typeof id !== "string" || (area !== "catalog" && area !== "recent")) return;
  return {
    id, area, offset: coordinate(offset), horizontalOffset: coordinate(horizontalOffset),
    control: control === "actions" ? control : undefined,
  };
}

export function readShelfSession(): ShelfSession {
  const fallback = { query: "", filters: emptyFilters(), sort: "title", layout: "grid", position: emptyShelfPosition() } satisfies ShelfSession;
  try {
    const saved = JSON.parse(sessionStorage.getItem(shelfSessionKey) ?? "null") as Partial<ShelfSession> | null;
    if (!saved) return fallback;
    const filters = emptyFilters();
    filters.favorite = saved.filters?.favorite === true;
    for (const key of facetKeys) {
      const values = saved.filters?.[key];
      if (Array.isArray(values)) filters[key] = [...new Set(values.filter((value) => typeof value === "string" && value.length > 0
        && (key !== "status" || Object.hasOwn(readingLabels, value))))];
    }
    const position = saved.position;
    return {
      query: typeof saved.query === "string" ? saved.query : "",
      filters,
      sort: shelfSorts.some(([value]) => value === saved.sort) ? saved.sort! : "title",
      layout: saved.layout === "list" ? "list" : "grid",
      position: {
        y: coordinate(position?.y),
        recentX: coordinate(position?.recentX),
        tagsX: coordinate(position?.tagsX),
        conditionsY: coordinate(position?.conditionsY),
        anchor: readOrigin(position?.anchor),
        focus: readOrigin(position?.focus),
      },
    };
  } catch {
    return fallback;
  }
}

export function writeShelfSession(session: ShelfSession) {
  try {
    sessionStorage.setItem(shelfSessionKey, JSON.stringify(session));
  } catch {
    // The live shelf is still usable when document storage is unavailable.
  }
}
