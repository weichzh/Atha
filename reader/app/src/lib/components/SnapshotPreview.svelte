<script lang="ts">
  import { renderSnapshotElement } from "../../../../web/conversations.mjs";
  import { projectShadowCounterCss } from "../../../../web/content.mjs";
  import type { MessageClient, SourceCaptureView } from "../../messages.ts";
  import { isWebKitGtkRuntime } from "../data/runtime.ts";

  interface Props {
    capture: SourceCaptureView;
    resourceClient: Pick<MessageClient, "snapshotResource">;
  }

  let { capture, resourceClient }: Props = $props();
  let hostEl = $state<HTMLDivElement | null>(null);
  let failed = $state(false);

  $effect(() => {
    const element = hostEl;
    const current = capture;
    const client = resourceClient;
    if (!element) return;
    let disposed = false;
    const urls: string[] = [];
    element.replaceChildren();
    failed = false;
    void renderSnapshotElement(current, client, async (resource: Awaited<ReturnType<MessageClient["snapshotResource"]>>) => {
      if (disposed) throw new Error("snapshot-closed");
      const url = URL.createObjectURL(new Blob([Uint8Array.from(resource.bytes)], { type: resource.mediaType }));
      urls.push(url);
      return url;
    }, isWebKitGtkRuntime() ? projectShadowCounterCss : undefined).then((snapshot) => {
      if (!disposed) element.replaceChildren(snapshot);
    }).catch(() => {
      if (!disposed) failed = true;
    });
    return () => {
      disposed = true;
      for (const url of urls) URL.revokeObjectURL(url);
    };
  });
</script>

<div bind:this={hostEl} class="snapshot-shadow-host"></div>
{#if failed}
  <p role="status">无法显示此版本的原文快照。</p>
{/if}

<style>
  .snapshot-shadow-host {
    display: block;
    width: 100%;
    border-radius: var(--radius-md);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
  }
</style>
