<script lang="ts">
  import {
    ArrowLeft,
    ArrowUpRight,
    Check,
    ChevronDown,
    Columns2,
    Download,
    History,
    MessageSquare,
    MoreHorizontal,
    Search,
    X,
  } from "@lucide/svelte";
  import { Dialog, Popover, Select } from "bits-ui";
  import { tick } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import { fade, fly, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type {
    ConversationView,
    MessageView,
    RevisionView,
    SourceCaptureView,
  } from "../../messages.ts";
  import { renderSafeMessageHtml } from "../data/message-doc.ts";
  import { getMessageClient } from "../data/message-adapter.ts";
  import type { ReaderEngine } from "../engine/reader-engine.svelte.ts";
  import type { ComposerSubmitPayload, RootNoteItem } from "../types.ts";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import MessageActionMenu from "./MessageActionMenu.svelte";
  import MessageComposer from "./MessageComposer.svelte";
  import SnapshotPreview from "./SnapshotPreview.svelte";

  interface Props {
    engine: ReaderEngine;
    mode: "closed" | "list" | "thread";
    selectedRootId: string | null;
    pendingDraft: Awaited<ReturnType<ReaderEngine["captureSelectionDraft"]>>;
    viewportWidth: number;
    viewportHeight: number;
    focusThread: boolean;
    onSelectRoot: (rootId: string) => void;
    onBackToList: () => void;
    onClose: () => void;
    onToggleFocusThread: () => void;
    onCancelDraft: () => void;
  }

  let {
    engine,
    mode,
    selectedRootId,
    pendingDraft,
    viewportWidth,
    viewportHeight,
    focusThread,
    onSelectRoot,
    onBackToList,
    onClose,
    onToggleFocusThread,
    onCancelDraft,
  }: Props = $props();

  let notesQuery = $state("");
  let chapterFilter = $state("all");
  let threadScope = $state<"current" | "chapter" | "book">("current");
  let threadSort = $state<"time" | "position">("time");

  let paneBEl = $state<HTMLElement | null>(null);
  let threadScrollEl = $state<HTMLDivElement | null>(null);
  const reducedMotion = new MediaQuery("(prefers-reduced-motion: reduce)");
  let savedListScrollTop = 0;

  // Thread interactive state
  let replyTarget = $state<{ id: string; preview: string; rootId: string } | null>(null);
  let referenceItems = $state<{ id: string; label: string }[]>([]);
  let editingTarget = $state<{ rootId: string; message: MessageView } | null>(null);

  // Action menu state
  let activeMenu = $state<{ message: MessageView; anchor: HTMLElement; rootId: string } | null>(null);

  // Modals: Snapshot, Revisions, Confirm Delete
  let snapshotOpen = $state(false);
  let snapshotCaptures = $state<SourceCaptureView[]>([]);
  let snapshotLoading = $state(false);

  let revisionsOpen = $state(false);
  let revisionsList = $state<RevisionView[]>([]);
  let revisionsLoading = $state(false);

  let confirmDelete = $state<{
    open: boolean;
    message: MessageView | null;
    isRoot: boolean;
  }>({ open: false, message: null, isRoot: false });

  // C1 exact layout rules: B and C are NEVER separated by A!
  const isCompact = $derived(viewportWidth < 800 || viewportHeight > viewportWidth);
  const isMediumLandscape = $derived(!isCompact && viewportWidth < 1440);
  const isWideLandscape = $derived(!isCompact && viewportWidth >= 1440);

  const showPaneB = $derived(
    mode !== "closed" &&
      (isCompact ? mode === "list" : true),
  );
  const showPaneC = $derived(
    mode === "thread" &&
      (isCompact || isMediumLandscape || isWideLandscape),
  );

  // Sync active highlight in book whenever thread root changes
  $effect(() => {
    engine.setActiveRootHighlight(mode === "thread" ? selectedRootId : null);
  });

  // Restore Pane B scroll position when returning B <- C on compact viewports
  $effect(() => {
    if (showPaneB && paneBEl && savedListScrollTop > 0) {
      const targetTop = savedListScrollTop;
      void tick().then(() => {
        if (paneBEl) paneBEl.scrollTop = targetTop;
      });
    }
  });

  const chapterOptions = $derived.by(() => {
    const items: { value: string; label: string }[] = [{ value: "all", label: "全部章节" }];
    const seen = new Set<string>();
    for (const root of engine.roots) {
      const sec = root.sourceAnchor.canonicalLocator
        ? (() => {
            try {
              return JSON.parse(root.sourceAnchor.canonicalLocator).start.section as string;
            } catch {
              return "";
            }
          })()
        : "";
      if (sec && !seen.has(sec)) {
        seen.add(sec);
        const tocMatch = engine.toc.find((t) => t.href.startsWith(sec));
        items.push({ value: sec, label: tocMatch?.label ?? sec });
      }
    }
    return items;
  });

  const selectedChapterLabel = $derived(
    chapterOptions.find((o) => o.value === chapterFilter)?.label ?? "全部章节",
  );

  function rootSection(root: RootNoteItem): string {
    try {
      return JSON.parse(root.sourceAnchor.canonicalLocator).start.section as string;
    } catch {
      return "";
    }
  }

  const filteredRoots = $derived.by(() => {
    const q = notesQuery.trim().toLocaleLowerCase();
    return engine.roots
      .filter((r) => chapterFilter === "all" || rootSection(r) === chapterFilter)
      .filter((r) => {
        if (!q) return true;
        return (
          r.sourceAnchor.selectedText.toLocaleLowerCase().includes(q) ||
          r.note.toLocaleLowerCase().includes(q) ||
          r.latestText.toLocaleLowerCase().includes(q)
        );
      })
      .sort((a, b) => b.updatedAt - a.updatedAt);
  });

  const activeRoot = $derived(
    selectedRootId
      ? engine.roots.find((r) => r.id === selectedRootId) ?? null
      : filteredRoots[0] ?? null,
  );
  const activeConversation = $derived(
    engine.conversations.find((c) => c.id === activeRoot?.conversationId &&
      c.messages[0]?.id === activeRoot?.id && !c.messages[0]?.deleted) ?? null,
  );
  const missingSelectedThread = $derived(selectedRootId !== null && !activeConversation);
  const editingMessage = $derived(
    !pendingDraft && editingTarget?.rootId === activeRoot?.id ? editingTarget?.message ?? null : null,
  );
  const activeReply = $derived(
    !pendingDraft && replyTarget?.rootId === activeRoot?.id ? replyTarget : null,
  );
  let refreshingThread = $state(false);

  async function reloadSelectedThread() {
    const rootId = selectedRootId;
    refreshingThread = true;
    try {
      await engine.refreshMessages();
      if (selectedRootId === rootId && activeConversation) threadScope = "current";
    } catch {
      engine.showToast("笔记暂时无法读取");
    } finally {
      refreshingThread = false;
    }
  }

  const visibleConversations = $derived.by((): ConversationView[] => {
    if (pendingDraft || missingSelectedThread) return [];
    if (threadScope === "current") return activeConversation ? [activeConversation] : [];
    const list = engine.conversations.filter((conv) => {
      const root = conv.messages[0];
      if (!root || root.deleted || !root.source) return false;
      if (threadScope === "chapter") {
        return root.source.section === (activeRoot ? rootSection(activeRoot) : engine.currentSectionId);
      }
      return true;
    });
    return [...list].sort((a, b) => {
      if (threadSort === "position") {
        const sa = a.messages[0]?.source?.section ?? "";
        const sb = b.messages[0]?.source?.section ?? "";
        if (sa !== sb) return sa.localeCompare(sb);
        const oa = a.messages[0]?.source?.canonicalLocator ?? "";
        const ob = b.messages[0]?.source?.canonicalLocator ?? "";
        return oa.localeCompare(ob);
      }
      const ua = a.messages.at(-1)?.updatedAt ?? 0;
      const ub = b.messages.at(-1)?.updatedAt ?? 0;
      return ub - ua;
    });
  });

  function openRootFromList(rootId: string) {
    if (paneBEl) savedListScrollTop = paneBEl.scrollTop;
    threadScope = "current";
    replyTarget = null;
    editingTarget = null;
    onSelectRoot(rootId);
  }

  function handleBackToB() {
    editingTarget = null;
    replyTarget = null;
    onBackToList();
  }

  async function handleComposerSubmit(payload: ComposerSubmitPayload): Promise<boolean> {
    const client = getMessageClient();
    const sourceDraft = pendingDraft;
    if (sourceDraft) {
      const id = await engine.createHighlightFromSelection(payload.text, payload.richText, sourceDraft);
      if (!id) return false;
      if (pendingDraft === sourceDraft) {
        threadScope = "current";
        editingTarget = null;
        replyTarget = null;
        referenceItems = [];
        onSelectRoot(id);
      }
      return true;
    }
    const edit = editingMessage;
    const targetRoot = activeRoot;
    const scrollAtSubmit = threadScrollEl;
    if (!targetRoot || !activeConversation) return false;
    if (edit) {
      await client.revise(edit.id, edit.revisionId, payload.text, payload.richText);
      if (editingMessage === edit) editingTarget = null;
    } else {
      const reply = activeReply;
      const references = referenceItems;
      await client.reply({
        conversationId: targetRoot.conversationId,
        replyToMessageId: reply?.id ?? targetRoot.id,
        text: payload.text,
        richText: payload.richText,
        referenceIds: references.map((ref) => ref.id),
      });
      if (replyTarget === reply) replyTarget = null;
      if (referenceItems === references) referenceItems = [];
    }
    try {
      await engine.refreshMessages();
      if (!edit) {
        await tick();
        if (mode === "thread" && !pendingDraft && threadScope === "current" &&
          activeRoot?.id === targetRoot.id && scrollAtSubmit && threadScrollEl === scrollAtSubmit) {
          threadScrollEl.scrollTop = threadScrollEl.scrollHeight;
        }
      }
    } catch {
      // The write committed; retrying it after a projection error would duplicate the message.
      engine.showToast("已保存，重新打开笔记可查看");
    }
    return true;
  }

  // Reset on navigation, not when a refresh replaces the same projected messages.
  const threadViewKey = $derived(JSON.stringify([
    pendingDraft?.anchor.canonicalLocator ?? activeRoot?.id,
    threadScope,
    threadScope === "current" ? null : threadSort,
  ]));
  $effect(() => {
    if (mode === "thread" && threadScrollEl && threadViewKey) threadScrollEl.scrollTop = 0;
  });

  function activateMessageThread(rootId: string) {
    if (rootId !== activeRoot?.id || threadScope !== "current") openRootFromList(rootId);
  }

  async function openSnapshotForRoot(rootMessageId: string) {
    snapshotOpen = true;
    snapshotLoading = true;
    try {
      snapshotCaptures = await getMessageClient().sourceCaptures(rootMessageId);
    } finally {
      snapshotLoading = false;
    }
  }

  async function openRevisionsForMessage(messageId: string) {
    revisionsOpen = true;
    revisionsLoading = true;
    try {
      revisionsList = await getMessageClient().revisions(messageId);
    } finally {
      revisionsLoading = false;
    }
  }

  async function executeDeleteMessage() {
    if (!confirmDelete.message) return;
    if (confirmDelete.isRoot) {
      await engine.deleteRootNote(confirmDelete.message.id);
      if (isCompact || !engine.roots.length) {
        onBackToList();
      } else if (engine.roots[0]) {
        onSelectRoot(engine.roots[0].id);
      }
    } else {
      await getMessageClient().remove(confirmDelete.message.id, confirmDelete.message.revisionId);
      await engine.refreshMessages();
    }
  }

  async function handleExport(conversationId: string | null = null) {
    await getMessageClient().export(engine.getEditionId(), conversationId);
  }

  function formatTime(ms: number): string {
    return new Date(ms).toLocaleString("zh-CN", {
      month: "numeric",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function findParentSnippet(conv: ConversationView, replyToId: string | null): string | null {
    if (!replyToId) return null;
    const rootId = conv.messages[0]?.id;
    if (replyToId === rootId) return null; // Root quote is already shown above
    const target = conv.messages.find((m) => m.id === replyToId);
    if (!target) return null;
    return target.deleted ? "（原消息已删除）" : target.text.slice(0, 42);
  }

</script>

{#if mode !== "closed"}
  <div
    class="workspace-overlay"
    class:compact={isCompact}
    class:medium={isMediumLandscape}
    class:wide={isWideLandscape}
  >
    {#if showPaneB}
      <aside
        class="pane-b glass-popover"
        aria-label="笔记列表"
        transition:fly={{ x: isCompact ? -24 : 24, duration: reducedMotion.current ? 0 : 220, easing: cubicOut }}
      >
        <div class="pane-head">
          <div class="pane-title-group">
            <h2>笔记</h2>
            <span class="pane-count tabular">{filteredRoots.length}</span>
          </div>
          <div class="pane-actions">
            <Popover.Root>
              <Popover.Trigger class="icon-btn" aria-label="导出笔记">
                <Download size={16} />
              </Popover.Trigger>
              <Popover.Portal>
                <Popover.Content sideOffset={8} align="end" class="glass-popover export-popover">
                  <button type="button" class="menu-item" onclick={() => handleExport(null)}>
                    <Download size={14} />
                    <span>导出全书笔记</span>
                  </button>
                  {#if activeRoot}
                    <button
                      type="button"
                      class="menu-item"
                      onclick={() => handleExport(activeRoot.conversationId)}
                    >
                      <MessageSquare size={14} />
                      <span>导出当前对话</span>
                    </button>
                  {/if}
                </Popover.Content>
              </Popover.Portal>
            </Popover.Root>
            <button type="button" class="icon-btn" aria-label="关闭笔记工作区" onclick={onClose}>
              <X size={17} />
            </button>
          </div>
        </div>

        <div class="pane-b-controls">
          <div class="well-input pane-search">
            <Search size={14} class="search-icon" />
            <input
              type="search"
              aria-label="筛选笔记"
              placeholder="搜索摘录与对话…"
              bind:value={notesQuery}
            />
          </div>

          <Select.Root
            type="single"
            value={chapterFilter}
            onValueChange={(v) => v && (chapterFilter = v)}
            items={chapterOptions}
          >
            <Select.Trigger class="pill-btn chapter-select-trigger" aria-label="按章节筛选">
              <span>{selectedChapterLabel}</span>
              <ChevronDown size={14} />
            </Select.Trigger>
            <Select.Portal>
              <Select.Content sideOffset={6} class="glass-popover select-popover">
                <Select.Viewport>
                  {#each chapterOptions as opt (opt.value)}
                    <Select.Item value={opt.value} label={opt.label} class="select-item">
                      {#snippet children({ selected })}
                        <span>{opt.label}</span>
                        {#if selected}
                          <Check size={14} />
                        {/if}
                      {/snippet}
                    </Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
        </div>

        <div class="pane-b-list" bind:this={paneBEl}>
          {#if filteredRoots.length === 0}
            <div class="pane-empty">
              <p>还没有笔记或标注。</p>
              <span>选中正文任意句子即可开始标注与对话。</span>
            </div>
          {:else}
            {#each filteredRoots as root (root.id)}
              {@const active = mode === "thread" && activeRoot?.id === root.id}
              <button
                type="button"
                class="note-index-row"
                class:active
                onclick={() => openRootFromList(root.id)}
              >
                <div class="row-quote">
                  <span class="row-bar" class:note-type={root.type === "note"} aria-hidden="true"></span>
                  <p>{root.sourceAnchor.selectedText}</p>
                </div>
                {#if root.latestText || root.note}
                  <div class="row-latest">{root.latestText || root.note}</div>
                {:else}
                  <div class="row-latest empty">还没有回应</div>
                {/if}
                <div class="row-foot tabular">
                  <span>{root.type === "note" ? "对话" : "标注"}</span>
                  <span>{formatTime(root.updatedAt)}</span>
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </aside>
    {/if}

    {#if showPaneC}
      <section
        id="message-conversation"
        class="pane-c glass-popover"
        aria-label="当前消息线程"
        transition:fly={{ x: 24, duration: reducedMotion.current ? 0 : 220, easing: cubicOut }}
      >
        <div class="pane-head">
          <div class="pane-title-group">
            <button
              type="button"
              class="icon-btn"
              aria-label={isCompact ? "返回笔记列表" : "收起对话线程"}
              onclick={handleBackToB}
            >
              <ArrowLeft size={17} />
            </button>
            <h2>对话</h2>
          </div>

          <div class="pane-actions">
            {#if isWideLandscape}
              <button
                type="button"
                class="pill-btn focus-toggle"
                class:active={focusThread}
                aria-pressed={focusThread}
                onclick={onToggleFocusThread}
              >
                <Columns2 size={14} />
                <span>{focusThread ? "退出专注" : "专注对话"}</span>
              </button>
            {/if}
            <button type="button" class="icon-btn" aria-label="关闭笔记工作区" onclick={onClose}>
              <X size={17} />
            </button>
          </div>
        </div>

        {#if !pendingDraft}
          <div class="scope-bar">
            <div class="scope-pills" role="tablist" aria-label="对话范围">
              {#each [
                { value: "current", label: "当前" },
                { value: "chapter", label: "本章" },
                { value: "book", label: "本书" },
              ] as opt (opt.value)}
                <button
                  type="button"
                  role="tab"
                  aria-selected={threadScope === opt.value}
                  class="scope-pill"
                  class:active={threadScope === opt.value}
                  onclick={() => (threadScope = opt.value as typeof threadScope)}
                >
                  {opt.label}
                </button>
              {/each}
            </div>

            {#if threadScope !== "current"}
              <button
                type="button"
                class="sort-pill"
                onclick={() => (threadSort = threadSort === "time" ? "position" : "time")}
              >
                排序：{threadSort === "time" ? "时间" : "位置"}
              </button>
            {/if}
          </div>
        {/if}

        <div class="thread-scroll" bind:this={threadScrollEl}>
          {#if pendingDraft}
            <div class="editorial-quote-box">
              <div class="quote-head">
                <span class="quote-badge">新建对话 · 原文</span>
                <button type="button" class="clear-draft-link" onclick={onCancelDraft}>放弃草稿</button>
              </div>
              <blockquote class="source-quote">
                <span class="quote-accent-bar" aria-hidden="true"></span>
                <p>{pendingDraft.text}</p>
              </blockquote>
            </div>
          {:else if missingSelectedThread}
            <div class="thread-empty-hint">
              <p role="status">这条对话暂时无法读取。</p>
              <button type="button" class="pill-btn" disabled={refreshingThread} onclick={reloadSelectedThread}>
                {refreshingThread ? "正在读取…" : "重新读取"}
              </button>
            </div>
          {:else if visibleConversations.length === 0}
            <div class="thread-empty-hint">选择一条摘录，围绕原文展开对话。</div>
          {:else}
            {#each visibleConversations as conv (conv.id)}
              {@const rootMsg = conv.messages[0]}
              {@const activeMessages = conv.messages.filter((m) => !m.deleted)}
              {#if rootMsg && rootMsg.source}
                <div class="conversation-block">
                  <div class="editorial-quote-box">
                    <div class="quote-head">
                      <span class="quote-badge">原文</span>
                      <div class="quote-actions">
                        <button
                          type="button"
                          class="mini-action"
                          onclick={() => void engine.jumpToLocator(rootMsg.source!.canonicalLocator)}
                        >
                          <ArrowUpRight size={13} />
                          <span>跳回原文</span>
                        </button>
                        <button
                          type="button"
                          class="mini-action"
                          onclick={() => void openSnapshotForRoot(rootMsg.id)}
                        >
                          <History size={13} />
                          <span>历史快照</span>
                        </button>
                      </div>
                    </div>
                    <blockquote class="source-quote">
                      <span class="quote-accent-bar" aria-hidden="true"></span>
                      <p>{rootMsg.source.selectedText}</p>
                    </blockquote>
                  </div>

                  {#if activeMessages.every((m) => m.kind === "source-only")}
                    <div class="highlight-only-invite">
                      这是一条纯原文高亮标注，还没有回应。在下方输入想法，随时开始对话。
                    </div>
                  {:else}
                    <div class="messages-timeline">
                      {#each activeMessages as msg (msg.id)}
                        {#if msg.kind !== "source-only"}
                          {@const parentSnippet = findParentSnippet(conv, msg.replyToMessageId)}
                          <article class="message-node">
                            <div class="message-bubble">
                              {#if parentSnippet}
                                <div class="provenance-chip">回复：{parentSnippet}</div>
                              {/if}

                              {#each msg.referencePreviews as ref (ref.id)}
                                <div class="provenance-chip ref">
                                  引用：{ref.deleted ? "（已删除消息）" : ref.text.slice(0, 48)}
                                </div>
                              {/each}

                              <div class="msg-content atha-tiptap-body">
                                {@html renderSafeMessageHtml(msg.contentJson, msg.text)}
                              </div>

                              <div class="msg-footer">
                                <span class="msg-meta tabular">
                                  {formatTime(msg.updatedAt)}
                                  {#if msg.updatedAt > msg.createdAt}
                                    <span> · 已编辑</span>
                                  {/if}
                                </span>
                                <button
                                  type="button"
                                  class="msg-more-btn"
                                  aria-label="消息操作"
                                  onclick={(e) => (activeMenu = { message: msg, anchor: e.currentTarget, rootId: rootMsg.id })}
                                >
                                  <MoreHorizontal size={16} />
                                </button>
                              </div>
                            </div>
                          </article>
                        {/if}
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}
            {/each}
          {/if}
        </div>

        {#if pendingDraft || activeConversation}
          <div class="composer-dock">
            <MessageComposer
              draftKey={pendingDraft?.anchor.canonicalLocator ?? editingMessage?.id ?? activeRoot?.id ?? ""}
              sealDraftOnSubmit={Boolean(pendingDraft || editingMessage)}
              initialText={editingMessage?.text ?? ""}
              initialContentJson={editingMessage?.contentJson ?? ""}
              editingLabel={editingMessage ? editingMessage.text.slice(0, 18) : null}
              replyChipLabel={editingMessage ? null : activeReply?.preview ?? null}
              referenceChipLabels={pendingDraft || editingMessage ? [] : referenceItems}
              submitLabel={editingMessage ? "保存修订" : pendingDraft ? "创建对话" : "发送"}
              onClearReply={() => (replyTarget = null)}
              onRemoveReference={(id) => (referenceItems = referenceItems.filter((r) => r.id !== id))}
              onCancelEdit={() => (editingTarget = null)}
              onSubmit={handleComposerSubmit}
            />
          </div>
        {/if}
      </section>
    {/if}
  </div>
{/if}

{#if activeMenu}
  {@const menuState = activeMenu}
  <MessageActionMenu
    anchorEl={menuState.anchor}
    canEdit={menuState.message.kind === "text"}
    onReply={() => {
      activateMessageThread(menuState.rootId);
      editingTarget = null;
      replyTarget = {
        id: menuState.message.id,
        rootId: menuState.rootId,
        preview: menuState.message.text.slice(0, 20) || "原文",
      };
    }}
    onReference={() => {
      if (!referenceItems.some((r) => r.id === menuState.message.id)) {
        referenceItems = [
          ...referenceItems,
          { id: menuState.message.id, label: menuState.message.text.slice(0, 18) || "消息" },
        ];
      }
    }}
    onEdit={() => {
      activateMessageThread(menuState.rootId);
      editingTarget = { rootId: menuState.rootId, message: menuState.message };
    }}
    onRevisions={() => void openRevisionsForMessage(menuState.message.id)}
    onDelete={() => {
      confirmDelete = {
        open: true,
        message: menuState.message,
        isRoot: menuState.message.id === menuState.rootId,
      };
    }}
    onClose={() => (activeMenu = null)}
  />
{/if}

<ConfirmDialog
  bind:open={confirmDelete.open}
  title={confirmDelete.isRoot ? "删除这段标注与对话？" : "删除这条消息？"}
  description={confirmDelete.isRoot
    ? "将隐藏此段原文高亮及整条对话线程（数据以墓碑形式保留快照完整性）。"
    : "删除后此消息将不再显示在默认阅读对话流中。"}
  confirmLabel="确认删除"
  destructive
  onConfirm={executeDeleteMessage}
/>

<Dialog.Root bind:open={snapshotOpen}>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div {...props} class="atha-dialog-overlay" transition:fade={{ duration: 150 }}></div>
        {/if}
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-content wide modal-centered"
            transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}
          >
            <div class="modal-head">
              <div>
                <Dialog.Title class="dialog-heading">原文历史快照</Dialog.Title>
                <Dialog.Description class="dialog-sub">
                  冻结创建时刻的 HTML、CSS 与呈现参数，在隔离 Shadow Root 中只读呈现。
                </Dialog.Description>
              </div>
              <button type="button" class="icon-btn" aria-label="关闭" onclick={() => (snapshotOpen = false)}>
                <X size={18} />
              </button>
            </div>
            {#if snapshotLoading}
              <div class="modal-empty">正在读取快照…</div>
            {:else if snapshotCaptures.length === 0}
              <div class="modal-empty">暂无已捕获快照。</div>
            {:else}
              <div class="modal-stack">
                {#each snapshotCaptures as cap (cap.source.id)}
                  <SnapshotPreview capture={cap} resourceClient={getMessageClient()} />
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<Dialog.Root bind:open={revisionsOpen}>
  <Dialog.Portal>
    <Dialog.Overlay forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div {...props} class="atha-dialog-overlay" transition:fade={{ duration: 150 }}></div>
        {/if}
      {/snippet}
    </Dialog.Overlay>
    <Dialog.Content forceMount>
      {#snippet child({ props, open: isOpen })}
        {#if isOpen}
          <div
            {...props}
            class="atha-dialog-content modal-centered"
            transition:scale={{ start: 0.96, duration: 180, easing: cubicOut }}
          >
            <div class="modal-head">
              <Dialog.Title class="dialog-heading">消息修订历史</Dialog.Title>
              <button type="button" class="icon-btn" aria-label="关闭" onclick={() => (revisionsOpen = false)}>
                <X size={18} />
              </button>
            </div>
            {#if revisionsLoading}
              <div class="modal-empty">正在读取修订历史…</div>
            {:else}
              <div class="modal-stack">
                {#each revisionsList as rev (rev.id)}
                  <div class="rev-item">
                    <div class="rev-time tabular">{formatTime(rev.createdAt)}</div>
                    <div class="rev-text">{rev.text || "（纯标注修订）"}</div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/snippet}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  .workspace-overlay {
    display: flex;
    height: 100%;
    gap: 12px;
    padding: 14px 14px 14px 0;
    z-index: 40;
  }

  .workspace-overlay.compact {
    position: fixed;
    inset: 0;
    padding: 0;
    z-index: 50;
    background: var(--bg-scrim);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }

  .workspace-overlay.medium {
    padding: 14px;
    flex: none;
  }

  .pane-b,
  .pane-c {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-radius: var(--radius-xl);
    overflow: hidden;
  }

  .pane-b {
    width: 320px;
    flex: none;
    padding: 18px 16px;
  }

  .pane-c {
    width: 400px;
    flex: none;
    padding: 18px 18px 16px;
  }

  .workspace-overlay.medium .pane-b {
    width: 310px;
  }

  .workspace-overlay.medium .pane-c {
    flex: 1;
    width: auto;
    min-width: 340px;
  }

  .workspace-overlay.wide:has(.focus-toggle.active) .pane-b {
    width: 360px;
  }

  .workspace-overlay.wide:has(.focus-toggle.active) .pane-c {
    flex: 1;
    width: auto;
  }

  .workspace-overlay.compact .pane-b,
  .workspace-overlay.compact .pane-c {
    position: absolute;
    inset: auto 0 0;
    width: 100%;
    height: min(var(--reader-mobile-sheet-h, 78dvh), calc(100% - env(safe-area-inset-top, 0px)));
    border-radius: var(--radius-lg) var(--radius-lg) 0 0;
    padding: 12px 16px calc(12px + env(safe-area-inset-bottom, 0px));
    background: var(--bg-elevated);
  }

  .workspace-overlay.compact .pane-head :global(.icon-btn) {
    width: 44px;
    min-height: 44px;
  }

  @media (max-height: 530px) {
    .workspace-overlay.compact .pane-b,
    .workspace-overlay.compact .pane-c {
      height: calc(100% - env(safe-area-inset-top, 0px));
    }
  }

  .pane-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 14px;
  }

  .pane-title-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pane-head h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.015em;
  }

  .pane-count {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-well);
    color: var(--ink-secondary);
  }

  .pane-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .pane-b-controls {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 14px;
  }

  .pane-search {
    height: 38px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pane-search :global(.search-icon) {
    color: var(--ink-muted);
    flex: none;
  }

  .pane-search input {
    width: 100%;
    font-size: 13.5px;
  }

  :global(.chapter-select-trigger) {
    width: 100%;
    justify-content: space-between !important;
  }

  .pane-b-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-right: 2px;
  }

  .pane-empty {
    padding: 40px 16px;
    text-align: center;
    color: var(--ink-muted);
    font-size: 13px;
    line-height: 1.6;
  }

  .note-index-row {
    width: 100%;
    text-align: left;
    padding: 13px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-surface);
    box-shadow: var(--shadow-xs);
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: background-color 160ms var(--ease-smooth), box-shadow 160ms var(--ease-smooth), transform 160ms var(--ease-spring);
  }

  .note-index-row:hover {
    background: var(--bg-elevated);
    box-shadow: var(--shadow-sm);
  }

  .note-index-row.active {
    background: var(--accent-soft);
    box-shadow: none;
  }

  .row-quote {
    position: relative;
    padding-left: 11px;
    font-family: var(--font-serif);
    font-size: 13px;
    line-height: 1.55;
    color: var(--ink-primary);
  }

  .row-quote p {
    margin: 0;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .row-bar {
    position: absolute;
    left: 0;
    top: 2px;
    bottom: 2px;
    width: 2.5px;
    border-radius: 999px;
    background: #d99b43;
  }

  .row-bar.note-type {
    background: var(--accent);
  }

  .row-latest {
    font-size: 13px;
    line-height: 1.5;
    color: var(--ink-secondary);
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .row-latest.empty {
    font-size: 12px;
    color: var(--ink-muted);
  }

  .row-foot {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    color: var(--ink-muted);
  }

  /* Pane C Thread */
  .scope-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 14px;
  }

  .scope-pills {
    display: inline-flex;
    padding: 3px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    gap: 2px;
  }

  .scope-pill {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--radius-pill);
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-secondary);
  }

  .scope-pill.active {
    background: var(--bg-pill-active);
    color: var(--ink-primary);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }

  .sort-pill {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius-pill);
    background: var(--bg-well);
    font-size: 12px;
    color: var(--ink-secondary);
  }

  .thread-scroll {
    flex: 1;
    overscroll-behavior: contain;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding-right: 2px;
  }

  .conversation-block {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .editorial-quote-box {
    padding: 14px 16px;
    border-radius: var(--radius-lg);
    background: var(--quote-tint);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .quote-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .quote-badge {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--accent);
  }

  .quote-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 6px;
  }

  .mini-action,
  .clear-draft-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius-pill);
    background: var(--bg-surface);
    color: var(--ink-secondary);
    font-size: 11.5px;
    font-weight: 500;
    box-shadow: var(--shadow-xs);
  }

  .mini-action:hover {
    color: var(--accent);
  }

  .source-quote {
    position: relative;
    margin: 0;
    padding-left: 12px;
    font-family: var(--font-serif);
    font-size: 14px;
    line-height: 1.72;
    color: var(--ink-primary);
  }

  .source-quote p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .quote-accent-bar {
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    width: 3px;
    border-radius: 999px;
    background: var(--accent);
    opacity: 0.78;
  }

  .highlight-only-invite,
  .thread-empty-hint {
    padding: 20px 16px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
    font-size: 13px;
    line-height: 1.6;
    color: var(--ink-secondary);
    text-align: center;
  }

  .thread-empty-hint p {
    margin: 0 0 8px;
  }

  .messages-timeline {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .message-node {
    display: flex;
    justify-content: flex-end;
  }

  .message-bubble {
    max-width: 90%;
    min-width: 0;
    padding: 10px 12px 0;
    border-radius: var(--radius-md) var(--radius-md) 4px var(--radius-md);
    background: var(--accent-soft);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .msg-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .msg-meta {
    font-size: 11.5px;
    color: var(--ink-secondary);
  }

  .msg-more-btn {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-pill);
    display: grid;
    place-items: center;
    color: var(--ink-secondary);
  }

  .msg-more-btn:hover,
  .msg-more-btn:focus-visible {
    background: var(--bg-well-hover);
    color: var(--ink-primary);
  }

  @media (pointer: coarse) {
    .msg-more-btn {
      width: 44px;
      height: 44px;
    }

    .scope-pill,
    .sort-pill,
    .mini-action,
    .clear-draft-link {
      min-height: 44px;
    }
  }

  .provenance-chip {
    padding: 4px 10px;
    border-radius: var(--radius-xs);
    background: var(--bg-well);
    font-size: 12px;
    color: var(--ink-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .provenance-chip.ref {
    background: var(--accent-soft-hover);
    color: var(--ink-primary);
  }

  .msg-content {
    font-size: 14px;
    line-height: 1.65;
    color: var(--ink-primary);
    overflow-wrap: anywhere;
  }

  .composer-dock {
    margin-top: 12px;
  }

  :global(.export-popover) {
    z-index: 72;
    padding: 6px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 164px;
  }

  :global(.export-popover .menu-item) {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 12px;
    border-radius: var(--radius-xs);
    font-size: 13px;
  }

  :global(.export-popover .menu-item:hover) {
    background: var(--bg-well);
  }

  .modal-centered {
    left: 50%;
    top: 50%;
    transform: none;
    translate: -50% -50%;
  }

  .modal-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 14px;
  }

  .modal-stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .modal-empty {
    padding: 24px 0;
    text-align: center;
    color: var(--ink-muted);
    font-size: 13.5px;
  }

  .rev-item {
    padding: 12px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-well);
  }

  .rev-time {
    font-size: 11.5px;
    color: var(--ink-muted);
    margin-bottom: 4px;
  }

  .rev-text {
    font-size: 13.5px;
    line-height: 1.6;
  }
</style>
