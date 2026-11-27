import type { TocEntry } from "../types.ts";

export interface TocNode {
  index: number;
  label: string;
  children: TocNode[];
}

/** Keep manifest indices for jumps; depth gaps do not create empty headings. */
export function buildTocTree(entries: readonly TocEntry[]): TocNode[] {
  const roots: TocNode[] = [];
  const ancestors: { level: number; node: TocNode }[] = [];
  for (const [index, entry] of entries.entries()) {
    const node: TocNode = { index, label: entry.label, children: [] };
    const level = entry.level ?? 1;
    while (ancestors.length && ancestors[ancestors.length - 1].level >= level) {
      ancestors.pop();
    }
    (ancestors[ancestors.length - 1]?.node.children ?? roots).push(node);
    ancestors.push({ level, node });
  }
  return roots;
}

/** Open nearby groups in short tables and always reveal the path to the current entry. */
export function initialTocExpansion(nodes: readonly TocNode[], activeIndex: number): Set<number> {
  const result = new Set<number>();
  function walk(branch: readonly TocNode[], top: boolean): boolean {
    let containsActive = false;
    for (const node of branch) {
      const activeChild = walk(node.children, false);
      if (activeChild || (top && nodes.length <= 24 && node.children.length > 0)) result.add(node.index);
      if (node.index === activeIndex || activeChild) containsActive = true;
    }
    return containsActive;
  }
  walk(nodes, true);
  return result;
}
