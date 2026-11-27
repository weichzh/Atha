<script lang="ts">
  import { Heart } from "@lucide/svelte";
  import { coverGlyph, coverTone, type LibraryBook } from "../../library.ts";
  import { resolveBookCoverUrl } from "../data/library-adapter.ts";

  interface Props {
    book: LibraryBook;
    index: number;
    percent?: number | null;
    compact?: boolean;
    coverRevision?: number;
  }

  let { book, index, percent = null, compact = false, coverRevision = 0 }: Props = $props();

  let imageLoaded = $state(false);
  let imageFailed = $state(false);

  const src = $derived(book.hasCover ? resolveBookCoverUrl(book, index, coverRevision) : "");
  const tone = $derived(coverTone(book.id, 4));
  const glyph = $derived(coverGlyph(book.title));

  $effect(() => {
    src;
    imageLoaded = false;
    imageFailed = false;
  });
</script>

<div class="cover-stage" class:compact data-tone={tone}>
  <div class="cover-art">
    {#if src && !imageFailed}
      <img
        {src}
        alt={book.title}
        loading="lazy"
        decoding="async"
        class:loaded={imageLoaded}
        onload={() => (imageLoaded = true)}
        onerror={() => (imageFailed = true)}
      />
    {/if}
    {#if !src || imageFailed || !imageLoaded}
      <div class="cover-fallback" aria-hidden="true">
        <span class="cover-glyph">{glyph}</span>
        <span class="cover-mini-title">{book.title}</span>
      </div>
    {/if}

    {#if book.favorite}
      <span class="favorite-pip" aria-label="已收藏">
        <Heart size={11} fill="currentColor" strokeWidth={2} />
      </span>
    {/if}
  </div>

  {#if !compact}
    <div
      class="progress-sliver"
      class:placeholder={!percent || percent <= 0}
      aria-label={percent && percent > 0 ? `阅读进度 ${percent}%` : undefined}
    >
      {#if percent !== null && percent > 0}
        <div class="progress-fill" style:width={`${Math.min(100, Math.max(4, percent))}%`}></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .cover-stage {
    position: relative;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .cover-art {
    position: relative;
    width: 100%;
    aspect-ratio: 5 / 7;
    border-radius: 10px;
    overflow: hidden;
    background: var(--bg-elevated);
    box-shadow: var(--shadow-book);
    transition: transform 240ms var(--ease-spring), box-shadow 240ms var(--ease-spring);
  }

  .cover-stage.compact .cover-art {
    border-radius: 7px;
  }

  .cover-art::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.14) 0%,
      rgba(0, 0, 0, 0.05) 3.5%,
      transparent 8%,
      transparent 96%,
      rgba(0, 0, 0, 0.04) 100%
    );
    pointer-events: none;
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 260ms var(--ease-smooth);
  }

  img.loaded {
    opacity: 1;
  }

  .cover-fallback {
    position: absolute;
    inset: 0;
    padding: 16px 14px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 10px;
    background: linear-gradient(155deg, #ece6da 0%, #dfd7c8 100%);
    color: #2c2620;
  }

  [data-tone="1"] .cover-fallback {
    background: linear-gradient(155deg, #dce2dd 0%, #cbd4cd 100%);
    color: #1f2a24;
  }

  [data-tone="2"] .cover-fallback {
    background: linear-gradient(155deg, #e5ddd5 0%, #d6cbc0 100%);
    color: #2f231c;
  }

  [data-tone="3"] .cover-fallback {
    background: linear-gradient(155deg, #d9dfe6 0%, #c8d0da 100%);
    color: #1c242e;
  }

  .cover-glyph {
    font-family: var(--font-serif);
    font-size: 38px;
    font-weight: 600;
    line-height: 1;
    color: var(--accent);
  }

  .cover-mini-title {
    font-size: 12px;
    line-height: 1.4;
    font-weight: 500;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    opacity: 0.78;
  }

  .favorite-pip {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 24px;
    height: 24px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 253, 249, 0.88);
    color: var(--accent);
    box-shadow: var(--shadow-xs);
    backdrop-filter: blur(8px);
  }

  .progress-sliver {
    width: 100%;
    height: 3px;
    border-radius: 999px;
    background: var(--bg-well);
    overflow: hidden;
  }

  .progress-sliver.placeholder {
    opacity: 0;
  }

  .progress-fill {
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
    transition: width 260ms var(--ease-smooth);
  }
</style>
