export function readerWorkspaceLayout(width: number, height: number) {
  if (width < 800 || width < height) return "compact";
  if (width >= 1360 && width >= height * 1.4) return "three";
  return "split";
}

// Decide from the space the reader still owns, not the space behind its side panes.
export function usesFacingPages(bodyWidth: number, width: number, height: number) {
  return bodyWidth >= 1160 && height >= 560 && width >= height * 1.3;
}
