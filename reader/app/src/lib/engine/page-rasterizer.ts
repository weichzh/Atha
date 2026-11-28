export interface PageTurnViewport {
  frame: HTMLElement;
  reader: HTMLElement;
  page: HTMLElement;
  book: HTMLElement;
  pageStep: number;
}

interface PageProjection {
  frameRect: DOMRect;
  pageRect: DOMRect;
  shift: number;
  frameScale: number;
  readerScale: number;
}

/**
 * Paint only the turning column, from the live, sanitized DOM and its decoded images.
 * The transient surface is never serialized, read back or used as reading content.
 */
export function rasterizeReaderPage(
  viewport: PageTurnViewport,
  columnDelta: number,
  width: number,
  height: number,
  paper: string,
): HTMLCanvasElement | null {
  const { frame, reader, page, book, pageStep } = viewport;
  const frameRect = frame.getBoundingClientRect();
  const pageRect = page.getBoundingClientRect();
  const frameScale = frameRect.width / width;
  const readerScale = reader.getBoundingClientRect().width / reader.offsetWidth;
  if (!frameScale || !readerScale || !pageRect.width || !pageRect.height) return null;

  const shift = columnDelta * pageStep * readerScale;
  const projection = { frameRect, pageRect, shift, frameScale, readerScale };
  const visible = (rect: DOMRect) =>
    rect.width > 0 && rect.height > 0 && rect.left < pageRect.right + shift - 0.25 &&
    rect.right > pageRect.left + shift + 0.25 && rect.top < pageRect.bottom && rect.bottom > pageRect.top;
  const canvas = document.createElement("canvas");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.ceil(width * dpr);
  canvas.height = Math.ceil(height * dpr);
  const ctx = canvas.getContext("2d", { alpha: false });
  if (!ctx) return null;
  ctx.setTransform(canvas.width / width, 0, 0, canvas.height / height, 0, 0);
  ctx.fillStyle = paper;
  ctx.fillRect(0, 0, width, height);

  const { x, y, w, h } = project(pageRect, { ...projection, shift: 0 });
  ctx.beginPath();
  ctx.rect(x, y, w, h);
  ctx.clip();
  const filter = getComputedStyle(reader).filter;
  if (filter !== "none" && filter !== "brightness(1)") {
    if (!("filter" in ctx)) return null;
    ctx.filter = filter;
  }

  const opacityCache = new WeakMap<Element, number>();
  function effectiveOpacity(element: Element): number {
    const cached = opacityCache.get(element);
    if (cached !== undefined) return cached;
    const parent = element === book ? null : element.parentElement;
    const opacity = Number(getComputedStyle(element).opacity) * (parent ? effectiveOpacity(parent) : 1);
    opacityCache.set(element, opacity);
    return opacity;
  }

  // The DOM cover preserves material a Canvas2D text/image pass cannot faithfully paint.
  for (const element of book.querySelectorAll("svg, math, ruby, .atha-structured-overflow, blockquote, hr, li, img")) {
    if (![...element.getClientRects()].some(visible)) continue;
    const style = getComputedStyle(element);
    const opacity = effectiveOpacity(element);
    if (style.visibility !== "visible" || opacity === 0) continue;
    if (element.localName === "li" && style.display !== "list-item") continue;
    if (!(element instanceof HTMLImageElement) || !element.complete || !element.naturalWidth ||
      opacity !== 1 || style.objectFit !== "fill" || style.transform !== "none") return null;
    const area = project(element.getBoundingClientRect(), projection);
    try {
      ctx.drawImage(element, area.x, area.y, area.w, area.h);
    } catch (error) {
      if (!(error instanceof DOMException)) throw error;
      return null;
    }
  }

  const walker = document.createTreeWalker(book, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  while (walker.nextNode()) {
    const node = walker.currentNode as Text;
    if (!node.data.trim() || !node.parentElement) continue;
    range.selectNodeContents(node);
    const rects = range.getClientRects();
    if (![...rects].some(visible)) continue;
    const style = getComputedStyle(node.parentElement);
    const opacity = effectiveOpacity(node.parentElement);
    if (style.visibility !== "visible" || opacity === 0) continue;
    if (opacity !== 1 || style.writingMode !== "horizontal-tb" || style.direction === "rtl") return null;
    const scale = readerScale / frameScale;
    ctx.font = `${style.fontStyle} ${style.fontWeight} ${parseFloat(style.fontSize) * scale}px ${style.fontFamily}`;
    if ("letterSpacing" in ctx) ctx.letterSpacing = `${(parseFloat(style.letterSpacing) || 0) * scale}px`;
    ctx.fillStyle = style.color;
    ctx.textBaseline = "middle";
    const paint = (text: string, rect: DOMRect) => {
      const area = project(rect, projection);
      if (!style.whiteSpace.startsWith("pre")) text = text.replace(/[\t\n\r ]+/g, " ").trim();
      if (text) ctx.fillText(text, area.x, area.y + area.h * 0.52, area.w);
    };
    if (rects.length === 1) {
      paint(node.data, rects[0]);
      continue;
    }
    // Layout decides the lines; binary search each line's extent rather than splitting at guessed widths.
    for (let start = 0; start < node.length;) {
      let end = start + 1;
      let lo = end;
      let hi = node.length;
      while (lo <= hi) {
        const mid = (lo + hi) >>> 1;
        range.setStart(node, start);
        range.setEnd(node, mid);
        if (range.getClientRects().length <= 1) {
          end = mid;
          lo = mid + 1;
        } else {
          hi = mid - 1;
        }
      }
      range.setStart(node, start);
      range.setEnd(node, end);
      const rect = range.getClientRects()[0];
      if (rect && visible(rect)) paint(node.data.slice(start, end), rect);
      start = end;
    }
  }
  return canvas;
}

function project(rect: DOMRect, { frameRect, shift, frameScale }: PageProjection) {
  return {
    x: (rect.left - shift - frameRect.left) / frameScale,
    y: (rect.top - frameRect.top) / frameScale,
    w: rect.width / frameScale,
    h: rect.height / frameScale,
  };
}
