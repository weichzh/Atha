<script lang="ts">
  import { onMount } from "svelte";
  import type { OpeningCover, openingCoverFrame, openingPaperApproach } from "../data/open-transition.ts";

  let { cover, frame, approach, turning, onPoised, onFinish }: {
    cover: OpeningCover;
    frame: ReturnType<typeof openingCoverFrame>;
    approach: ReturnType<typeof openingPaperApproach>;
    turning: boolean;
    onPoised: () => void;
    onFinish: () => void;
  } = $props();

  let pause: ReturnType<typeof setTimeout>;
  onMount(() => () => clearTimeout(pause));

  function finishLift(event: AnimationEvent) {
    if (turning || event.target !== event.currentTarget) return;
    // Once the front has settled, give its artwork time to be read before turning.
    pause = setTimeout(onPoised, 420);
  }
</script>

<div class="opening-cover" aria-hidden="true">
  <div
    class="cover-plane"
    class:approaching={turning}
    style:left={`${frame.left}px`}
    style:top={`${frame.top}px`}
    style:width={`${cover.rect.width}px`}
    style:height={`${cover.rect.height}px`}
    style:--from-x={`${cover.rect.left - frame.left}px`}
    style:--from-y={`${cover.rect.top - frame.top}px`}
    style:--book-scale={frame.scale}
    style:--book-to-x={`${approach.coverToX}px`}
    style:--book-to-y={`${-frame.top}px`}
    style:--book-to-scale={approach.coverToScale}
    onanimationend={finishLift}
  >
    <div
      class="cover-hinge"
      class:turning
      onanimationend={(event) => {
        if (event.target === event.currentTarget) onFinish();
      }}
    >
      <div class="cover-front" {@attach (node) => { node.append(cover.art); }}></div>
      <div class="cover-back"></div>
    </div>
  </div>
</div>

<style>
  .opening-cover {
    position: fixed;
    inset: 0;
    z-index: 3;
    overflow: hidden;
    pointer-events: none;
  }

  .cover-plane {
    position: absolute;
    transform-origin: top left;
    transform: scale(var(--book-scale));
    perspective: calc(1100px / var(--book-scale));
    perspective-origin: 12% center;
  }

  .cover-hinge, .cover-front, .cover-back {
    position: absolute;
    inset: 0;
  }

  .cover-hinge {
    transform-origin: left center;
    transform-style: preserve-3d;
  }

  .cover-front, .cover-back {
    backface-visibility: hidden;
  }

  .cover-front :global(.cover-art) {
    border-radius: calc(7px / var(--book-scale));
  }

  .cover-back {
    border-radius: 8px 1px 1px 8px;
    transform: rotateY(180deg);
    background:
      linear-gradient(to left, var(--bg-scrim), transparent 16%),
      var(--opening-canvas);
  }

  @media (prefers-reduced-motion: no-preference) {
    .cover-plane {
      animation: lift-book 480ms cubic-bezier(0.22, 0.76, 0.2, 1) both;
    }

    .cover-plane.approaching {
      animation: approach-book var(--book-approach);
    }

    .cover-hinge.turning {
      animation: turn-cover var(--book-turn-time) cubic-bezier(0.44, 0.12, 0.23, 1) both;
    }

    .cover-hinge.turning .cover-back {
      animation: release-cover var(--book-turn-time) linear both;
    }
  }

  @keyframes lift-book {
    from { transform: translate(var(--from-x), var(--from-y)) scale(0.97); }
    to { transform: scale(var(--book-scale)); }
  }

  @keyframes turn-cover {
    from { transform: rotateY(0); }
    to { transform: rotateY(-164deg); }
  }

  @keyframes approach-book {
    from { transform: translate(0, 0) scale(var(--book-scale)); }
    to { transform: translate(var(--book-to-x), var(--book-to-y)) scale(var(--book-to-scale)); }
  }

  @keyframes release-cover {
    0%, 76% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
