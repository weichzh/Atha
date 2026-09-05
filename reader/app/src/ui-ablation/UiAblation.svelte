<script lang="ts">
  import {
    ArrowLeft,
    BatteryFull,
    Bookmark,
    BookOpenText,
    ChevronLeft,
    ChevronRight,
    Columns2,
    CornerUpLeft,
    CreditCard,
    Download,
    Ellipsis,
    Eye,
    FileImage,
    FlaskConical,
    Headphones,
    Highlighter,
    Languages,
    List,
    MessageCircle,
    Minus,
    PanelLeft,
    Paperclip,
    Quote,
    RotateCcw,
    Search,
    Send,
    Settings,
    Share2,
    SlidersHorizontal,
    Smile,
    Sparkles,
    SquarePen,
    Square,
    Signal,
    Trash2,
    Users,
    UserRound,
    Wifi,
    X,
  } from "@lucide/svelte";

  import mediaPreviewUrl from "./media-preview.png";

  type Stage = "weread" | "telegram" | "experiment";
  const variantOptions = ["C0", "A1", "A2", "A3", "A4", "A5", "A6", "A7"] as const;
  type Variant = (typeof variantOptions)[number];
  type ReferenceState = "clean" | "controls" | "desktop" | "list" | "thread";
  type ExperimentState = "list" | "thread";
  type ConversationRange = "当前" | "本章" | "本书";
  type SettingsScope = "阅读默认" | "本书样式" | "应用外观" | "CSS 专家";
  type Focus = "reader" | "notes";
  type MenuName = "settings" | "readerMore" | "threadMore" | null;
  const opaqueTreatmentTokens: Record<Variant, string> = {
    C0: "q0",
    A1: "q1",
    A2: "q2",
    A3: "q3",
    A4: "q4",
    A5: "q5",
    A6: "q6",
    A7: "q7",
  };

  const params = new URLSearchParams(globalThis.location.search);
  const runId = params.get("run");
  const blind = params.get("blind") === "1" || /^R-[A-Za-z0-9]+$/.test(runId ?? "");
  const capture = params.get("capture") === "1";

  function parseChoice<T extends string>(value: string | null, choices: readonly T[], fallback: T): T {
    return value && (choices as readonly string[]).includes(value) ? (value as T) : fallback;
  }

  function resolveBlindVariant(run: string | null): Variant {
    if (!run) return "C0";
    let hash = 0;
    for (const character of run) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
    return variantOptions[hash % variantOptions.length];
  }

  const requestedStage = parseChoice(params.get("stage"), ["weread", "telegram", "experiment"] as const, "experiment");
  const initialStage: Stage = blind ? "experiment" : requestedStage;
  const variant = blind ? resolveBlindVariant(runId) : parseChoice(params.get("variant"), variantOptions, "C0");
  const rawState = params.get("state");
  const initialFocus: Focus = parseChoice(params.get("focus"), ["reader", "notes"] as const, "reader");
  const notesCanOpenThread = globalThis.innerWidth >= 701;

  if (blind && typeof globalThis.history?.replaceState === "function") {
    const sanitizedUrl = new URL(globalThis.location.href);
    sanitizedUrl.searchParams.delete("variant");
    sanitizedUrl.searchParams.delete("stage");
    globalThis.history.replaceState(null, "", sanitizedUrl);
  }

  let stage: Stage = initialStage;
  let experimentVariant: Variant = variant;
  let referenceState: ReferenceState =
    initialStage === "weread"
      ? parseChoice(rawState, ["clean", "controls"] as const, "clean")
      : initialStage === "telegram"
        ? parseChoice(rawState, ["desktop", "list", "thread"] as const, "desktop")
        : "list";
  let experimentState: ExperimentState =
    initialFocus === "notes" && notesCanOpenThread
      ? "thread"
      : parseChoice(rawState, ["list", "thread"] as const, "list");
  let focus: Focus = initialFocus;
  let notesOpen = params.get("focus") === "reader" ? false : true;
  let selectedNoteId = "root";
  let conversationRange: ConversationRange = "当前";
  let settingsScope: SettingsScope = "本书样式";
  let query = "";
  let draft = "";
  let status = "";
  let sent = false;
  let bookmarked = false;
  let deleted = false;
  let deletePending = false;
  let openMenu: MenuName = null;
  let deleteDialog: HTMLDialogElement | null = null;
  let deleteTrigger: HTMLButtonElement | null = null;
  let deleteReturnSelector: string | null = null;

  const wereadLines = [
    { text: "当我们离开海港，风从远处吹来时潮声回响", accent: false },
    { text: "把漫长的海面推向黄昏，也推向远岸，向前", accent: false },
    { text: "《奥德赛》里的归途并不只是一条路，回声", accent: true },
    { text: "它更像一组不断回到原点的记忆，缓慢", accent: false },
    { text: "每一次靠岸，都让旧故事重新显形，再起", accent: false },
    { text: "船帆收起时，城市还没有睡去，灯火未眠", accent: false },
    { text: "我们听见水手谈论失去的名字，夜里低声", accent: false },
    { text: "也听见木板在脚下发出轻微回声，漂向海面", accent: false },
    { text: "远方的岛屿被雾气抹去边界，只剩微光", accent: false },
    { text: "只有星光仍然指出回家的方向，穿过雾向北", accent: false },
    { text: "英雄并不总是知道自己要去哪里，仍旧未定", accent: false },
    { text: "他只是继续读完眼前的下一行，再读缓慢", accent: false },
    { text: "潮水退去，留下盐和细小的贝壳，片刻不语", accent: false },
    { text: "夜色把每段叙述都变得更安静，一点低语", accent: false },
    { text: "于是我们保存这一页，等待明天再读", accent: false },
    { text: "等待另一个可以停下来的海岸，等潮水退", accent: false },
    { text: "回望时，旧风仍在书页之间，尚未停息", accent: false },
    { text: "而归途终于有了可以指认的位置，仍在脚下", accent: false },
  ];

  const telegramChats = [
    { initials: "LM", name: "Lena Morgan", preview: "The chapter is still on my mind", time: "09:42", color: "#d17976", unread: "2" },
    { initials: "NB", name: "Northbound", preview: "Shared a document", time: "08:17", color: "#6b9ac8", unread: "" },
    { initials: "RS", name: "Reading circle", preview: "A note was added", time: "Yesterday", color: "#8d7ec4", unread: "5" },
    { initials: "AK", name: "Archive club", preview: "See you after lunch", time: "Yesterday", color: "#c48a5b", unread: "" },
    { initials: "HC", name: "Harbor notes", preview: "A new note was added", time: "Mon", color: "#5d9b87", unread: "1" },
    { initials: "YI", name: "Yellow island", preview: "That quote is beautiful", time: "Sun", color: "#bf719a", unread: "" },
    { initials: "AF", name: "Afterglow", preview: "Saved this for later", time: "Sat", color: "#be8b68", unread: "" },
    { initials: "BM", name: "Blue margin", preview: "Can you read this page?", time: "Fri", color: "#548b9e", unread: "3" },
    { initials: "CN", name: "Common notes", preview: "A shared passage", time: "Thu", color: "#9a7d68", unread: "" },
    { initials: "DT", name: "Daybreak text", preview: "Forwarded from reading", time: "Wed", color: "#6d9c73", unread: "1" },
    { initials: "EM", name: "Evening margin", preview: "The next shore", time: "Tue", color: "#9b7292", unread: "" },
  ];

  const notes = [
    { id: "root", source: "归途并不只是一条路", preview: "把这一段留给下一次重读。", time: "刚刚", type: "note" },
    { id: "mark", source: "等待另一个可以停下来的海岸", preview: "仅有原文标注", time: "昨天", type: "mark" },
    { id: "reply", source: "旧风仍在书页之间", preview: "这个意象和开头呼应。", time: "周一", type: "note" },
  ];

  const threadMessages = [
    { id: "m1", kind: "reader", relation: "笔记", text: "把这一段留给下一次重读。", time: "09:41" },
    { id: "m2", kind: "reader", relation: "回复", text: "这个意象和开头呼应，像是把位置也保存下来了。", time: "09:42" },
    { id: "m3", kind: "reader", relation: "回复", text: "我更喜欢最后一句，语气突然安静。", time: "09:44" },
    { id: "m4", kind: "quote", relation: "引用", text: "就是这一句。", time: "09:47" },
  ];

  const scopeItems = [
    { name: "阅读默认", detail: "全书", scope: "阅读默认" as SettingsScope, icon: BookOpenText },
    { name: "本书样式", detail: "当前书", scope: "本书样式" as SettingsScope, icon: Bookmark },
    { name: "应用外观", detail: "应用", scope: "应用外观" as SettingsScope, icon: Eye },
    { name: "CSS 专家", detail: "本书", scope: "CSS 专家" as SettingsScope, icon: SlidersHorizontal },
  ];

  const stageOptions: { value: Stage; label: string }[] = [
    { value: "weread", label: "微信读书" },
    { value: "telegram", label: "Telegram" },
    { value: "experiment", label: "实验" },
  ];
  $: g0Off = experimentVariant === "A1";
  $: rootState = stage === "experiment" ? experimentState : referenceState;
  $: selectedNote = notes.find((note) => note.id === selectedNoteId) ?? notes[0];
  $: filteredNotes = notes.filter((note) => {
    const value = query.trim().toLocaleLowerCase();
    return !value || `${note.source} ${note.preview}`.toLocaleLowerCase().includes(value);
  });

  function resetFixture() {
    selectedNoteId = "root";
    conversationRange = "当前";
    settingsScope = "本书样式";
    query = "";
    draft = "";
    status = "";
    sent = false;
    bookmarked = false;
    deleted = false;
    deletePending = false;
    openMenu = null;
    deleteDialog = null;
    deleteTrigger = null;
    deleteReturnSelector = null;
    focus = "reader";
    notesOpen = true;
  }

  function setStage(next: Stage) {
    if (blind) return;
    resetFixture();
    stage = next;
    if (next === "weread") referenceState = "clean";
    if (next === "telegram") referenceState = "desktop";
    if (next === "experiment") experimentState = "list";
  }

  function setReferenceState(next: ReferenceState) {
    if (blind) return;
    referenceState = next;
  }

  function setFocus(next: Focus) {
    focus = next;
    notesOpen = next === "notes";
    if (next === "notes" && notesCanOpenThread) experimentState = "thread";
  }

  function setVariant(next: Variant) {
    if (blind) return;
    resetFixture();
    experimentVariant = next;
    stage = "experiment";
    experimentState = "list";
  }

  function chooseNote(id: string) {
    selectedNoteId = id;
    experimentState = "thread";
    status = "已打开当前线程";
  }

  function setConversationRange(range: ConversationRange) {
    conversationRange = range;
    openMenu = null;
    status = `对话范围：${range}`;
  }

  function setSettingsScope(scope: SettingsScope) {
    settingsScope = scope;
    openMenu = null;
    status = `设置范围：${scope}`;
  }

  function toggleMenu(name: Exclude<MenuName, null>) {
    openMenu = openMenu === name ? null : name;
  }

  function toggleBookmark() {
    bookmarked = !bookmarked;
    status = bookmarked ? "已标记当前位置" : "已取消当前位置标记";
  }

  function jumpToSource() {
    status = "已定位到原文锚点";
  }

  function sendMessage(event: SubmitEvent) {
    event.preventDefault();
    if (!draft.trim() || deleted) return;
    draft = "";
    sent = true;
    status = "已加入本次夹具的临时回复";
  }

  function requestDelete(event: MouseEvent) {
    const trigger = event.currentTarget;
    deleteTrigger = trigger instanceof HTMLButtonElement ? trigger : null;
    deleteReturnSelector = trigger instanceof Element && trigger.closest(".thread-context-menu")
      ? 'button[aria-label="线程更多操作"]'
      : 'button[aria-label="打开更多操作"]';
    openMenu = null;
    deletePending = true;
  }

  function closeDeleteDialog() {
    const returnFocus = deleteTrigger;
    const returnSelector = deleteReturnSelector;
    if (deleteDialog?.open) deleteDialog.close();
    deletePending = false;
    deleteDialog = null;
    deleteTrigger = null;
    deleteReturnSelector = null;
    queueMicrotask(() => {
      const target = returnFocus?.isConnected
        ? returnFocus
        : returnSelector
          ? document.querySelector<HTMLButtonElement>(returnSelector)
          : null;
      target?.focus();
    });
  }

  function cancelDelete() {
    const failedToCancel = g0Off;
    closeDeleteDialog();
    if (failedToCancel) {
      deleted = true;
      status = "已删除（取消没有阻止操作）";
    } else {
      status = "已取消删除";
    }
  }

  function confirmDelete() {
    closeDeleteDialog();
    deleted = true;
    status = "已确认删除（仅本次夹具）";
  }

  function enhanceDeleteDialog(node: HTMLDialogElement) {
    deleteDialog = node;
    if (!node.open) node.showModal();
    const initialFocus = node.querySelector<HTMLElement>("[data-initial-focus]");
    queueMicrotask(() => initialFocus?.focus());

    const handleCancel = (event: Event) => {
      event.preventDefault();
      cancelDelete();
    };
    node.addEventListener("cancel", handleCancel);

    return {
      destroy() {
        node.removeEventListener("cancel", handleCancel);
        if (node.open) node.close();
        if (deleteDialog === node) deleteDialog = null;
      },
    };
  }
</script>

<svelte:head>
  <title>{blind ? "阅读夹具" : stage === "weread" ? "WeRead reference" : stage === "telegram" ? "Telegram reference" : "UI ablation"}</title>
</svelte:head>

<div
  class="ui-ablation"
  class:reference-root={stage !== "experiment"}
  class:experiment-root={stage === "experiment"}
  data-stage={blind ? "sandbox" : stage}
  data-treatment={capture || blind ? opaqueTreatmentTokens[experimentVariant] : experimentVariant}
  data-state={rootState}
  data-focus={focus}
  data-notes-open={notesOpen ? "true" : "false"}
  data-controls-hidden={capture || blind ? "true" : "false"}
  data-capture={capture ? "true" : "false"}
  data-blind={blind ? "true" : "false"}
>
  {#if !capture && !blind}
    <nav class="ablation-control-bar" aria-label="原型控制">
      <div class="control-brand">
        <FlaskConical aria-hidden="true" />
        <span>UI ablation</span>
      </div>
      <div class="control-group" aria-label="Stage">
        <span class="control-label">Stage</span>
        {#each stageOptions as option}
          <button
            class:active={stage === option.value}
            type="button"
            aria-pressed={stage === option.value}
            onclick={() => setStage(option.value)}
          >
            {#if option.value === "weread"}<BookOpenText aria-hidden="true" />{:else if option.value === "telegram"}<MessageCircle aria-hidden="true" />{:else}<FlaskConical aria-hidden="true" />{/if}
            <span>{option.label}</span>
          </button>
        {/each}
      </div>
      {#if stage === "experiment"}
        <div class="control-group variant-group" aria-label="实验变体">
          <span class="control-label">组</span>
          {#each variantOptions as option}
            <button
              class:active={experimentVariant === option}
              type="button"
              aria-pressed={experimentVariant === option}
              onclick={() => setVariant(option)}
            >{option}</button>
          {/each}
        </div>
        <div class="control-group state-group" aria-label="窗口状态">
          <span class="control-label">状态</span>
          <button class:active={experimentState === "list"} type="button" aria-pressed={experimentState === "list"} onclick={() => (experimentState = "list")}>列表</button>
          <button class:active={experimentState === "thread"} type="button" aria-pressed={experimentState === "thread"} onclick={() => (experimentState = "thread")}>线程</button>
        </div>
      {:else if stage === "weread"}
        <div class="control-group state-group" aria-label="阅读状态">
          <span class="control-label">状态</span>
          <button class:active={referenceState === "clean"} type="button" aria-pressed={referenceState === "clean"} onclick={() => setReferenceState("clean")}>清洁</button>
          <button class:active={referenceState === "controls"} type="button" aria-pressed={referenceState === "controls"} onclick={() => setReferenceState("controls")}>工具</button>
        </div>
      {:else}
        <div class="control-group state-group" aria-label="Telegram 状态">
          <span class="control-label">状态</span>
          <button class:active={referenceState === "desktop"} type="button" aria-pressed={referenceState === "desktop"} onclick={() => setReferenceState("desktop")}>桌面</button>
          <button class:active={referenceState === "list"} type="button" aria-pressed={referenceState === "list"} onclick={() => setReferenceState("list")}>列表</button>
          <button class:active={referenceState === "thread"} type="button" aria-pressed={referenceState === "thread"} onclick={() => setReferenceState("thread")}>会话</button>
        </div>
      {/if}
    </nav>
  {/if}

  {#if stage === "weread"}
    <section class="reference-stage weread-stage" aria-label="微信读书视觉参照">
      <div class="weread-surface" data-reference={referenceState}>
        <div class="weread-book-label">简介</div>
        <article class="weread-copy" aria-label="书籍正文">
          {#each wereadLines as line}
            <p class:accent={line.accent}>{line.text}</p>
          {/each}
        </article>
        <footer class="weread-page-footer">
          <span>↑上滑显示工具栏</span>
          <span>2 / 1775</span>
        </footer>
        {#if referenceState === "controls"}
          <header class="weread-topbar">
            <button type="button" aria-label="返回" title="返回"><ArrowLeft aria-hidden="true" /></button>
            <span class="weread-top-title">简介</span>
            <div class="weread-top-actions">
              <button type="button" aria-label="付费信息" title="付费信息"><CreditCard aria-hidden="true" /></button>
              <button type="button" aria-label="加入书架" title="加入书架"><Bookmark aria-hidden="true" /></button>
              <button type="button" aria-label="读者数量" title="读者数量"><Users aria-hidden="true" /></button>
              <button type="button" aria-label="分享" title="分享"><Share2 aria-hidden="true" /></button>
              <button type="button" aria-label="更多" title="更多"><Ellipsis aria-hidden="true" /></button>
            </div>
            <div class="weread-device-status" aria-label="设备状态">
              <span>09:41</span><Signal aria-hidden="true" /><Wifi aria-hidden="true" /><BatteryFull aria-hidden="true" />
            </div>
          </header>
          <div class="weread-floating-actions" role="toolbar" aria-label="阅读辅助动作">
            <button type="button" aria-label="AI" title="AI"><Sparkles aria-hidden="true" /></button>
            <button class="selected" type="button" aria-label="翻译" title="翻译"><Languages aria-hidden="true" /></button>
            <button type="button" aria-label="听书" title="听书"><Headphones aria-hidden="true" /></button>
          </div>
          <nav class="weread-bottom-bar" aria-label="阅读工具">
            <button type="button" aria-label="目录" title="目录"><List aria-hidden="true" /><span>目录</span></button>
            <button type="button" aria-label="搜索" title="搜索"><Search aria-hidden="true" /><span>搜索</span></button>
            <button type="button" aria-label="标注" title="标注"><Highlighter aria-hidden="true" /><span>标注</span></button>
            <button type="button" aria-label="进度" title="进度"><Eye aria-hidden="true" /><span>进度</span></button>
            <button type="button" aria-label="设置" title="设置"><Settings aria-hidden="true" /><span>设置</span></button>
          </nav>
        {/if}
      </div>
    </section>
  {:else if stage === "telegram"}
    <section class="reference-stage telegram-stage" aria-label="Telegram 视觉参照">
      {#if referenceState === "desktop"}
        <div class="telegram-desktop-reference">
          <header class="telegram-titlebar">
            <span class="window-dots" aria-hidden="true"><i></i><i></i><i></i></span>
            <span class="telegram-title">Telegram</span>
            <div class="window-title-actions" aria-label="窗口控件">
              <button type="button" aria-label="最小化窗口" title="最小化窗口"><Minus aria-hidden="true" /></button>
              <button type="button" aria-label="最大化窗口" title="最大化窗口"><Square aria-hidden="true" /></button>
              <button type="button" aria-label="关闭窗口" title="关闭窗口"><X aria-hidden="true" /></button>
            </div>
          </header>
          <div class="telegram-desktop-body">
            <nav class="telegram-nav" aria-label="主导航">
              <button type="button" class="active" aria-label="聊天" title="聊天"><PanelLeft aria-hidden="true" /></button>
              <button type="button" aria-label="联系人" title="联系人"><Users aria-hidden="true" /></button>
              <button type="button" aria-label="设置" title="设置"><Settings aria-hidden="true" /></button>
              <button type="button" aria-label="更多" title="更多"><Ellipsis aria-hidden="true" /></button>
            </nav>
            <aside class="telegram-chat-list" aria-label="聊天列表">
              <div class="telegram-search"><Search aria-hidden="true" /><span>Search</span></div>
              <div class="telegram-filter-row"><span class="active">All</span><span>Unread</span><span>Groups</span></div>
              <div class="telegram-chat-rows">
                {#each telegramChats as chat, index}
                  <button class:active={index === 0} type="button" aria-label={`打开 ${chat.name}`} title={chat.name}>
                    <span class="telegram-avatar" style={`--avatar:${chat.color}`}>{chat.initials}</span>
                    <span class="telegram-chat-copy"><strong>{chat.name}</strong><small>{chat.preview}</small></span>
                    <span class="telegram-chat-meta"><time>{chat.time}</time>{#if chat.unread}<b>{chat.unread}</b>{/if}</span>
                  </button>
                {/each}
              </div>
            </aside>
            <main class="telegram-conversation" aria-label="会话">
              <header class="telegram-conversation-head">
                <span class="telegram-avatar large" style="--avatar:#d17976">LM</span>
                <div><strong>Lena Morgan</strong><small>last seen recently</small></div>
                <div class="telegram-head-actions"><button type="button" aria-label="搜索消息" title="搜索消息"><Search aria-hidden="true" /></button><button type="button" aria-label="更多" title="更多"><Ellipsis aria-hidden="true" /></button></div>
              </header>
              <div class="telegram-pinned"><Bookmark aria-hidden="true" /><span><strong>Pinned message</strong><small>The chapter is still on my mind</small></span><ChevronRight aria-hidden="true" /></div>
              <div class="telegram-message-stream">
                <span class="telegram-day">TODAY</span>
                <div class="telegram-message incoming"><span class="telegram-avatar small" style="--avatar:#d17976">LM</span><div><p>That passage about the coast is beautiful.</p><time>09:41</time></div></div>
                <div class="telegram-message outgoing"><div><p>It keeps the whole chapter moving without rushing it.</p><time>09:42</time></div></div>
                <div class="telegram-message incoming media"><span class="telegram-avatar small" style="--avatar:#d17976">LM</span><div><figure class="telegram-media-preview"><img src={mediaPreviewUrl} alt="合成阅读摘录图片" /><figcaption><FileImage aria-hidden="true" /><span>reading-note.png · 图片附件</span></figcaption></figure><p>Saved this for later.</p><time>09:44</time></div></div>
                <div class="telegram-message outgoing quoted"><div><blockquote>the next shore</blockquote><p>Exactly this line.</p><time>09:47</time></div></div>
                <div class="telegram-message incoming"><span class="telegram-avatar small" style="--avatar:#d17976">LM</span><div><p>I kept the margin open for this page.</p><time>09:49</time></div></div>
                <div class="telegram-message outgoing"><div><p>Same here. The quiet ending gives it room.</p><time>09:51</time></div></div>
                <div class="telegram-message incoming"><span class="telegram-avatar small" style="--avatar:#d17976">LM</span><div><p>The last paragraph brings the coast back.</p><time>09:53</time></div></div>
              </div>
              <form class="telegram-input" onsubmit={(event) => event.preventDefault()}>
                <button type="button" aria-label="附件" title="附件"><Paperclip aria-hidden="true" /></button>
                <span>Write a message...</span>
                <button type="button" aria-label="表情" title="表情"><Smile aria-hidden="true" /></button>
                <button type="button" aria-label="发送" title="发送"><Send aria-hidden="true" /></button>
              </form>
            </main>
          </div>
        </div>
      {:else if referenceState === "list"}
        <div class="telegram-phone-reference telegram-phone-list">
          <header class="telegram-titlebar"><span class="window-dots" aria-hidden="true"><i></i><i></i><i></i></span><span class="telegram-title">Telegram</span><div class="window-title-actions" aria-label="窗口控件"><button type="button" aria-label="最小化窗口" title="最小化窗口"><Minus aria-hidden="true" /></button><button type="button" aria-label="最大化窗口" title="最大化窗口"><Square aria-hidden="true" /></button><button type="button" aria-label="关闭窗口" title="关闭窗口"><X aria-hidden="true" /></button></div></header>
          <section class="telegram-phone-list-head">
            <div class="telegram-search"><Search aria-hidden="true" /><span>Search</span></div>
            <div class="telegram-filter-row"><span class="active">All</span><span>Default</span><span>Groups</span></div>
          </section>
          <div class="telegram-phone-rows">
            {#each telegramChats as chat, index}
              <button class:active={index === 0} type="button" aria-label={`打开 ${chat.name}`} title={chat.name}>
                <span class="telegram-avatar" style={`--avatar:${chat.color}`}>{chat.initials}</span>
                <span class="telegram-chat-copy"><strong>{chat.name}</strong><small>{chat.preview}</small></span>
                <span class="telegram-chat-meta"><time>{chat.time}</time>{#if chat.unread}<b>{chat.unread}</b>{/if}</span>
              </button>
            {/each}
          </div>
        </div>
      {:else}
        <div class="telegram-phone-reference telegram-phone-thread">
          <header class="telegram-titlebar"><span class="window-dots" aria-hidden="true"><i></i><i></i><i></i></span><span class="telegram-title">Telegram</span><div class="window-title-actions" aria-label="窗口控件"><button type="button" aria-label="最小化窗口" title="最小化窗口"><Minus aria-hidden="true" /></button><button type="button" aria-label="最大化窗口" title="最大化窗口"><Square aria-hidden="true" /></button><button type="button" aria-label="关闭窗口" title="关闭窗口"><X aria-hidden="true" /></button></div></header>
          <header class="telegram-phone-conversation-head"><button type="button" aria-label="返回聊天列表" title="返回聊天列表"><ArrowLeft aria-hidden="true" /></button><span class="telegram-avatar" style="--avatar:#d17976">LM</span><div><strong>Lena Morgan</strong><small>last seen recently</small></div><button type="button" aria-label="更多" title="更多"><Ellipsis aria-hidden="true" /></button></header>
          <div class="telegram-phone-pinned"><Bookmark aria-hidden="true" /><span><strong>Pinned message</strong><small>The chapter is still on my mind</small></span></div>
          <div class="telegram-phone-message-stream">
            <div class="telegram-message incoming media"><span class="telegram-avatar small" style="--avatar:#d17976">LM</span><div><figure class="telegram-media-preview"><img src={mediaPreviewUrl} alt="合成阅读摘录图片" /><figcaption><FileImage aria-hidden="true" /><span>reading-note.png · 图片附件</span></figcaption></figure><p>Saved this for later.</p><time>09:44</time></div></div>
            <div class="telegram-message outgoing"><div><p>It keeps the whole chapter moving without rushing it.</p><time>09:45</time></div></div>
          </div>
          <form class="telegram-input" onsubmit={(event) => event.preventDefault()}><button type="button" aria-label="附件" title="附件"><Paperclip aria-hidden="true" /></button><span>Write a message...</span><button type="button" aria-label="表情" title="表情"><Smile aria-hidden="true" /></button><button type="button" aria-label="发送" title="发送"><Send aria-hidden="true" /></button></form>
        </div>
      {/if}
    </section>
  {:else}
    <section class="experiment-stage" aria-label={blind ? "阅读夹具" : "Atha 阅读夹具"}>
      <div class="experiment-layout">
        <section class="pane-a" aria-label="正文 A">
          <header class="experiment-reader-toolbar">
            <button class="icon-control" type="button" aria-label="返回书架" title="返回书架"><ArrowLeft aria-hidden="true" /></button>
            <span class="reader-title">海岸线上的归途</span>
            <div class="reader-toolbar-actions">
              <button class:pressed={bookmarked} class="icon-control" type="button" aria-label="添加或取消标记" title="添加或取消标记" aria-pressed={bookmarked} onclick={toggleBookmark}><Bookmark aria-hidden="true" /></button>
              <button class:pressed={focus === "notes"} class="icon-control" type="button" aria-label="打开笔记并专注对话" title="打开笔记并专注对话" aria-pressed={focus === "notes"} onclick={() => setFocus("notes")}><MessageCircle aria-hidden="true" /></button>
              <button class="icon-control" type="button" aria-label="打开阅读设置" title="打开阅读设置" aria-expanded={openMenu === "settings"} onclick={() => toggleMenu("settings")}><Settings aria-hidden="true" /></button>
              <button class="icon-control" type="button" aria-label="打开更多操作" title="打开更多操作" aria-expanded={openMenu === "readerMore"} onclick={() => toggleMenu("readerMore")}><Ellipsis aria-hidden="true" /></button>
            </div>
          </header>
          <article class="book-page" aria-label="书籍内容">
            <div class="book-page-meta"><span>第 12 节</span><span>阅读中</span></div>
            <div class="book-copy">
              <p>当我们离开海港，风从远处吹来，把漫长的海面推向黄昏。</p>
              <p>归途并不只是一条路，它更像一组不断回到原点的记忆。</p>
              <p>每一次靠岸，都让旧故事重新显形。船帆收起时，城市还没有睡去。</p>
              <p>我们听见水手谈论失去的名字，也听见木板在脚下发出轻微回声。</p>
              <p>等待另一个可以停下来的海岸，等待一句话在下一次重读时重新抵达。</p>
            </div>
            <footer class="book-page-footer"><span>原文位置 · 第 12 节</span><span>12 / 48</span></footer>
          </article>
          <div class="high-frequency-controls" aria-label="常用阅读控制">
            <button class="text-control" type="button" aria-label="减小字号" title="减小字号"><span>A</span><small>-</small></button>
            <button class="text-control" type="button" aria-label="增大字号" title="增大字号"><span>A</span><small>+</small></button>
            <button class="text-control" type="button" aria-label="切换主题" title="切换主题"><Eye aria-hidden="true" /></button>
            <button class="text-control" type="button" aria-label="调整布局" title="调整布局"><Columns2 aria-hidden="true" /></button>
          </div>
          {#if openMenu === "settings"}
            <div class="context-menu settings-menu" role="menu" aria-label="阅读设置菜单">
              <div class="menu-heading"><span>阅读设置</span><button class="menu-close" type="button" aria-label="关闭阅读设置" title="关闭阅读设置" onclick={() => (openMenu = null)}><X aria-hidden="true" /></button></div>
              {#each scopeItems as item}
                <button class:active={settingsScope === item.scope} class="menu-item scope-item" type="button" role="menuitemradio" aria-checked={settingsScope === item.scope} onclick={() => setSettingsScope(item.scope)}>
                  <svelte:component this={item.icon} aria-hidden="true" />
                  <span class="scope-copy"><strong class="scope-name">{item.name}</strong><strong class="scope-confused">设置</strong><small>{item.detail}</small></span>
                  <ChevronRight aria-hidden="true" />
                </button>
              {/each}
            </div>
          {:else if openMenu === "readerMore"}
            <div class="context-menu more-menu" role="menu" aria-label="更多操作菜单">
              <button class="menu-item" type="button" role="menuitem" onclick={jumpToSource}><BookOpenText aria-hidden="true" /><span>跳回原文</span></button>
              <button class="menu-item" type="button" role="menuitem"><Download aria-hidden="true" /><span>导出当前引用</span></button>
              <button class="menu-item danger" type="button" role="menuitem" onclick={requestDelete}><Trash2 aria-hidden="true" /><span>删除笔记</span></button>
            </div>
          {/if}
        </section>

        <div class="note-window" aria-label="笔记窗口">
          <aside class="pane-b" aria-label="笔记列表 B">
            <header class="pane-header notes-header">
              <div><span class="eyebrow">阅读记忆</span><h1>笔记</h1></div>
              <button class="icon-control notes-close" type="button" aria-label="关闭笔记窗口" title="关闭笔记窗口" onclick={() => setFocus("reader")}><BookOpenText aria-hidden="true" /></button>
              <button class="icon-control" type="button" aria-label="新建笔记" title="新建笔记"><SquarePen aria-hidden="true" /></button>
            </header>
            <div class="scope-tabs" role="tablist" aria-label="笔记范围">
              {#each ["当前", "本章", "本书"] as scope}
                <button class:active={conversationRange === scope} type="button" role="tab" aria-selected={conversationRange === scope} onclick={() => setConversationRange(scope as ConversationRange)}>{scope}</button>
              {/each}
            </div>
            <form class="notes-search" onsubmit={(event) => event.preventDefault()}>
              <Search aria-hidden="true" />
              <label class="visually-hidden" for="notes-query">搜索笔记</label>
              <input id="notes-query" bind:value={query} type="search" placeholder="搜索笔记" autocomplete="off" />
              <button class="icon-control" type="button" aria-label="清除搜索" title="清除搜索" onclick={() => (query = "")}><X aria-hidden="true" /></button>
            </form>
            <div class="notes-list" aria-live="polite">
              {#each filteredNotes as note}
                <button class:selected={selectedNoteId === note.id} class="note-row" type="button" aria-label={`打开笔记：${note.source}`} onclick={() => chooseNote(note.id)}>
                  <span class="note-row-mark" aria-hidden="true">{note.type === "mark" ? "" : "·"}</span>
                  <span class="note-row-copy"><strong>{note.source}</strong><span>{note.preview}</span><small>{note.time}</small></span>
                  <ChevronRight aria-hidden="true" />
                </button>
              {/each}
            </div>
            <footer class="pane-footer"><span>{filteredNotes.length} 条阅读记忆</span><button type="button" aria-label="刷新笔记列表" title="刷新笔记列表" onclick={resetFixture}><RotateCcw aria-hidden="true" /></button></footer>
          </aside>

          <aside class="pane-c" aria-label="当前线程 C">
            <header class="pane-header thread-header">
              <button class="icon-control pane-back" type="button" aria-label="返回笔记列表" title="返回笔记列表" onclick={() => (experimentState = "list")}><ChevronLeft aria-hidden="true" /></button>
              <div><span class="eyebrow">当前线程</span><h2>{selectedNote.source}</h2></div>
              <button class="icon-control" type="button" aria-label="线程更多操作" title="线程更多操作" aria-expanded={openMenu === "threadMore"} onclick={() => toggleMenu("threadMore")}><Ellipsis aria-hidden="true" /></button>
            </header>
            {#if openMenu === "threadMore"}
              <div class="context-menu thread-context-menu" role="menu" aria-label="线程更多操作菜单">
                <button class="menu-item" type="button" role="menuitem" onclick={jumpToSource}><BookOpenText aria-hidden="true" /><span>跳回原文</span></button>
                <button class="menu-item" type="button" role="menuitem"><Download aria-hidden="true" /><span>导出当前引用</span></button>
                <button class="menu-item danger" type="button" role="menuitem" onclick={requestDelete}><Trash2 aria-hidden="true" /><span>删除笔记</span></button>
              </div>
            {/if}
            <div class="thread-scroll">
              <div class="message-flow" aria-label="消息流">
                <article class="thread-message source-message" data-message-kind="source">
                  <div class="message-marker" aria-hidden="true"><span></span></div>
                  <div class="message-body source-message-body">
                    <div class="source-message-head"><span><BookOpenText aria-hidden="true" /><strong>原文</strong></span><button class="source-message-jump" type="button" aria-label="跳回原文" title="跳回原文" onclick={jumpToSource}><BookOpenText aria-hidden="true" /></button></div>
                    <p>“归途并不只是一条路，它更像一组不断回到原点的记忆。”</p>
                    <small>第 12 节 · 历史引用快照</small>
                  </div>
                </article>
                {#each threadMessages as message}
                  <article class="thread-message" data-message-kind={message.kind}>
                    <div class="message-marker" aria-hidden="true"><span></span></div>
                    <div class="message-body">
                      <div class="message-meta"><span class="message-author"><span class="message-avatar" aria-hidden="true"><UserRound /></span><strong>我</strong></span><span class="message-relation">{message.relation}</span><time>{message.time}</time></div>
                      <p>{message.text}</p>
                      {#if message.kind === "quote"}<blockquote>“等待另一个可以停下来的海岸”</blockquote>{/if}
                      <div class="message-actions"><button type="button" aria-label="回复消息" title="回复消息" onclick={() => (status = "已选择回复目标")}><CornerUpLeft aria-hidden="true" /></button><button type="button" aria-label="引用消息" title="引用消息" onclick={() => (status = "已选择引用目标")}><Quote aria-hidden="true" /></button></div>
                    </div>
                  </article>
                {/each}
                {#if sent}<p class="sent-message" role="status"><span class="message-avatar" aria-hidden="true"><UserRound /></span><span>本次夹具中的临时回复已加入线程。</span></p>{/if}
                {#if deleted}<p class="deleted-message" role="status">这条笔记已被标记为删除，历史快照仍保留。</p>{/if}
              </div>
            </div>
            <form class="message-composer" onsubmit={sendMessage}>
              <label class="visually-hidden" for="message-draft">回复当前线程</label>
              <textarea id="message-draft" bind:value={draft} rows="1" placeholder="写下回应" disabled={deleted}></textarea>
              <button class="composer-tool" type="button" aria-label="添加引用" title="添加引用" onclick={() => (draft = "引用这一段：")}><Quote aria-hidden="true" /></button>
              <button class="composer-send" type="submit" aria-label="发送回复" title="发送回复" disabled={!draft.trim() || deleted}><Send aria-hidden="true" /></button>
            </form>
          </aside>
        </div>
      </div>
      <p class="experiment-status" aria-live="polite">{status}</p>
      {#if deletePending}
        <div class="confirm-backdrop">
          <dialog bind:this={deleteDialog} use:enhanceDeleteDialog class="confirm-dialog" aria-modal="true" aria-labelledby="delete-heading" aria-describedby="delete-description">
            <button class="dialog-close" type="button" aria-label="关闭删除确认" title="关闭删除确认" onclick={cancelDelete}><X aria-hidden="true" /></button>
            <div class="confirm-icon" aria-hidden="true"><Trash2 /></div>
            <h2 id="delete-heading">删除这条笔记？</h2>
            <p id="delete-description">确认后只改变本次离线夹具，历史引用不会被抹掉。</p>
            <div class="confirm-actions"><button data-initial-focus type="button" onclick={cancelDelete}>取消</button><button class="danger-button" type="button" onclick={confirmDelete}>确认删除</button></div>
          </dialog>
        </div>
      {/if}
    </section>
  {/if}
</div>
