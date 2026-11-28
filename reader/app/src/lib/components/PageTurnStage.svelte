<script lang="ts">
  import { onMount } from "svelte";
  import { rasterizeReaderPage, type PageTurnViewport } from "../engine/page-rasterizer.ts";
  import { spreadFold, type SpreadLeaf } from "../engine/spread-fold.ts";
  import { PageCurlRenderer, type CurlPose } from "../engine/webgl-page-curl.ts";

  type Effect = "cover" | "curl";
  type Direction = 1 | -1;
  let root: HTMLDivElement;
  let under: HTMLDivElement;
  let front: HTMLDivElement;
  let webgl: HTMLCanvasElement;
  let fallback: HTMLCanvasElement;
  let mode = $state<"cover" | "webgl" | "canvas2d" | "spread" | null>(null);
  let coverX = $state(0);
  let stationaryClip = $state("");
  let renderer: PageCurlRenderer | null = null;
  let motion: MediaQueryList;
  let turn: {
    effect: Effect;
    direction: Direction;
    from: number;
    width: number;
    height: number;
    y: number;
    progress: number;
    dragging: boolean;
    curled: boolean;
    leaf: SpreadLeaf | null;
  } | null = null;
  let frameId = 0;

  function cloneColumn(mount: HTMLElement, viewport: PageTurnViewport, index: number, paper: string) {
    const { reader, page, book, pageStep } = viewport;
    const sourceRoot = book.getRootNode() as ShadowRoot;
    const oldHost = sourceRoot.host;
    // Use the already-sanitized nodes and styles in a separate shadow tree. No HTML reparse
    // or loading path for books/resources is introduced by the visual overlay.
    const readerStyle = getComputedStyle(reader);
    const readerCopy = document.createElement("div");
    readerCopy.style.cssText = [
      "position:relative;display:flex;justify-content:center;box-sizing:border-box;overflow:hidden",
      `width:${readerStyle.width};height:${readerStyle.height}`,
      `padding:${readerStyle.padding};filter:${readerStyle.filter}`,
      `transform:${readerStyle.transform};transform-origin:top left`,
    ].join(";");
    // These are the local properties of the live .reader, including its layout variables.
    for (const name of reader.style) {
      if (name.startsWith("--")) readerCopy.style.setProperty(name, reader.style.getPropertyValue(name));
    }
    const pageCopy = document.createElement("div");
    pageCopy.style.cssText = "position:relative;overflow:hidden;flex:none";
    pageCopy.style.width = getComputedStyle(page).width;
    if (reader.hasAttribute("data-reader-spread")) {
      readerCopy.style.setProperty("--reader-col-w", readerStyle.getPropertyValue("--reader-col-w"));
      readerCopy.style.setProperty("--reader-spread-w", readerStyle.getPropertyValue("--reader-spread-w"));
    }
    const host = document.createElement("div");
    host.className = oldHost.className;
    for (const attr of oldHost.attributes) {
      if (attr.name.startsWith("data-")) host.setAttribute(attr.name, attr.value);
    }
    const shadow = host.attachShadow({ mode: "closed" });
    for (const node of sourceRoot.children) {
      if (node.localName === "style") shadow.append(node.cloneNode(true));
    }
    const article = book.cloneNode(true) as HTMLElement;
    article.removeAttribute("data-swipe-dragging");
    article.removeAttribute("data-swipe-settling");
    article.style.transition = "none";
    article.style.transform = `translateX(${-index * pageStep}px)`;
    // Cloning an image can reset its fetch state. Keep the known post-validation geometry
    // for the short-lived clone even if its cached, already-decoded bitmap paints a frame later.
    const originalImages = book.querySelectorAll<HTMLImageElement>("img[src]");
    article.querySelectorAll<HTMLImageElement>("img[src]").forEach((image, i) => {
      const original = originalImages[i];
      if (!original?.complete || !original.naturalWidth) return;
      const style = getComputedStyle(original);
      image.style.width = style.width;
      image.style.height = style.height;
    });
    shadow.append(article);
    pageCopy.append(host);
    readerCopy.append(pageCopy);
    mount.style.backgroundColor = paper;
    mount.replaceChildren(readerCopy);
    // cloneNode copies markup, not the visual position within a table or a code frame.
    const originalFrames = book.querySelectorAll<HTMLElement>(".atha-structured-overflow");
    article.querySelectorAll<HTMLElement>(".atha-structured-overflow").forEach((copy, index) => {
      copy.scrollLeft = originalFrames[index].scrollLeft;
      copy.scrollTop = originalFrames[index].scrollTop;
    });
  }

  function pose(p: number, y: number, w: number, h: number): CurlPose {
    const base = Math.max(28, Math.min(88, Math.min(w, h) * 0.13));
    const radius = Math.max(10, base * Math.sin(p * Math.PI));
    const normY = Math.max(0, Math.min(1, y / h));
    const tilt = (normY - 0.5) * 0.34;
    const len = Math.hypot(1, tilt);
    const pad = Math.abs(tilt) * h;
    return {
      originX: w + pad - p * (w + Math.PI * 10 * len + 2 * pad),
      originY: normY * h,
      axisX: 1 / len,
      axisY: tilt / len,
      radius,
      progress: p,
    };
  }

  function draw(progress: number) {
    if (!turn) return;
    turn.progress = progress;
    if (turn.leaf) {
      mode = renderer?.renderSpread(spreadFold(turn.leaf, progress, turn.y), turn.leaf) ? "spread" : "cover";
      coverX = -turn.direction * turn.width * progress;
      return;
    }
    const fold = turn.direction === 1 ? progress : 1 - progress;
    if (turn.curled) {
      mode = renderer?.render(pose(fold, turn.y, turn.width, turn.height)) ?? "cover";
    } else {
      mode = "cover";
    }
    // Backward turns bring the previous page back over the current one, not a mirrored book.
    coverX = -fold * turn.width;
  }

  function begin(view: PageTurnViewport, from: number, dir: Direction, effect: Effect, clientY?: number) {
    stop();
    const width = root.clientWidth;
    const height = root.clientHeight;
    if (!width || !height || motion?.matches) return false;
    const viewport = view.frame.getBoundingClientRect();
    const y = clientY === undefined ? height * 0.72 : (clientY - viewport.top) * height / viewport.height;
    const paper = getComputedStyle(view.frame).backgroundColor;
    const spread = effect === "curl" && view.reader.hasAttribute("data-reader-spread");
    if (spread) {
      const bounds = view.page.getBoundingClientRect();
      const scale = width / viewport.width;
      const spine = ((bounds.left + bounds.right) / 2 - viewport.left) * scale;
      const leafWidth = Math.min(bounds.width / 2 * scale + 24, spine, width - spine);
      const top = Math.max(0, bounds.top - viewport.top - 16);
      const leaf: SpreadLeaf = {
        left: dir === 1 ? spine : spine - leafWidth,
        top,
        width: leafWidth,
        height: Math.min(height - top, bounds.bottom - viewport.top + 16 - top),
        direction: dir,
      };
      stationaryClip = dir === 1 ? `inset(0 ${width - spine}px 0 0)` : `inset(0 0 0 ${spine}px)`;
      cloneColumn(under, view, from + dir, paper);
      cloneColumn(front, view, from, paper);
      const source = rasterizeReaderPage(view, 0, width, height, paper, dir === 1 ? "right" : "left");
      const back = source && rasterizeReaderPage(view, dir, width, height, paper, dir === 1 ? "left" : "right");
      let curled = false;
      if (source && back) {
        renderer ??= new PageCurlRenderer(webgl, fallback);
        curled = renderer.setSpreadSurface(source, back, width, height, paper);
        if (!curled) renderer.releaseSurface();
      }
      turn = { effect, direction: dir, from, width, height, y, progress: 0, dragging: false, curled, leaf };
      return true;
    }
    const top = dir === 1 ? from : from - 1;
    const bottom = dir === 1 ? from + 1 : from;
    cloneColumn(under, view, bottom, paper);
    cloneColumn(front, view, top, paper);
    let curled = false;
    if (effect === "curl") {
      const surface = rasterizeReaderPage(view, top - from, width, height, paper);
      if (surface) {
        renderer ??= new PageCurlRenderer(webgl, fallback);
        renderer.setPageSurface(surface, width, height, paper);
        curled = true;
      }
    }
    turn = { effect, direction: dir, from, width, height, y, progress: 0, dragging: false, curled, leaf: null };
    return true;
  }

  function settle(target: 0 | 1) {
    if (!turn) return;
    if (frameId) cancelAnimationFrame(frameId);
    const start = turn.progress;
    const duration = (turn.leaf && turn.curled ? 455 : turn.curled ? 380 : 250) * Math.max(0.4, Math.abs(target - start));
    const time = performance.now();
    turn.dragging = false;
    const tick = (now: number) => {
      if (motion?.matches || now - time >= duration) {
        stop();
        return;
      }
      draw(start + (target - start) * (1 - (1 - (now - time) / duration) ** 3));
      frameId = requestAnimationFrame(tick);
    };
    frameId = requestAnimationFrame(tick);
  }

  export function preview(view: PageTurnViewport, from: number, pages: number, dx: number, effect: Effect, clientY?: number) {
    const dir = dx < 0 ? 1 : -1;
    if (from + dir < 0 || from + dir >= pages) {
      stop();
      return;
    }
    if ((!turn?.dragging || turn.direction !== dir || turn.from !== from || turn.effect !== effect) &&
      !begin(view, from, dir, effect, clientY)) return;
    if (!turn) return;
    turn.dragging = true;
    draw(Math.min(1, Math.abs(dx) / (turn.leaf && turn.curled ? turn.leaf.width * 2 : turn.width)));
  }

  export function commit(view: PageTurnViewport, from: number, dir: Direction, effect: Effect) {
    if ((!turn?.dragging || turn.direction !== dir || turn.from !== from || turn.effect !== effect) &&
      !begin(view, from, dir, effect)) return;
    if (!turn) return;
    draw(turn.progress);
    settle(1);
  }

  export function cancel() {
    if (turn?.dragging) settle(0);
  }

  export function stop() {
    if (!turn) return;
    if (frameId) cancelAnimationFrame(frameId);
    frameId = 0;
    const hadSurface = turn.curled;
    turn = null;
    mode = null;
    if (hadSurface) renderer?.releaseSurface();
    under?.replaceChildren();
    front?.replaceChildren();
  }

  onMount(() => {
    motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    motion.addEventListener("change", stop);
    const redrawAfterLoss = () => {
      if (turn?.curled) draw(turn.progress);
    };
    webgl.addEventListener("webglcontextlost", redrawAfterLoss);
    return () => {
      motion.removeEventListener("change", stop);
      webgl.removeEventListener("webglcontextlost", redrawAfterLoss);
      stop();
      renderer?.destroy();
    };
  });
</script>

<div class="page-turn-stage" data-page-turn-renderer={mode} bind:this={root} aria-hidden="true" inert>
  <div class="turn-layer" bind:this={under}></div>
  <div
    class="turn-layer cover"
    class:visible={mode === "cover" || mode === "spread"}
    class:stationary={mode === "spread"}
    style:transform={mode === "spread" ? "none" : `translate3d(${coverX}px, 0, 0)`}
    style:clip-path={mode === "spread" ? stationaryClip : undefined}
  >
    <div class="turn-layer" bind:this={front}></div>
  </div>
  <canvas class="turn-layer" class:visible={mode === "webgl" || mode === "spread"} bind:this={webgl}></canvas>
  <canvas class="turn-layer" class:visible={mode === "canvas2d"} bind:this={fallback}></canvas>
</div>

<style>
  .page-turn-stage,
  .turn-layer {
    position: absolute;
    inset: 0;
  }

  .page-turn-stage {
    overflow: hidden;
    pointer-events: none;
    visibility: hidden;
  }

  .page-turn-stage[data-page-turn-renderer] {
    visibility: visible;
  }

  canvas {
    display: none;
    width: 100%;
    height: 100%;
  }

  canvas.visible {
    display: block;
  }

  .cover {
    opacity: 0;
  }

  .cover.visible {
    opacity: 1;
    will-change: transform;
    box-shadow: 8px 0 22px var(--shadow-ink, rgb(0 0 0 / 18%));
  }

  .cover.stationary {
    will-change: auto;
    box-shadow: none;
  }
</style>
