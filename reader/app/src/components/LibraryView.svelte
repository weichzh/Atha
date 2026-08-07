<script lang="ts">
  import { Archive, ArchiveRestore, BookOpen, Plus, Trash2 } from "@lucide/svelte";
  import { onMount } from "svelte";

  import {
    backupMessages,
    coverUrl,
    importBooks,
    importFailureMessage,
    libraryAvailable,
    listBooks,
    openBook,
    removeBook,
    restoreMessages,
    type LibraryBook,
  } from "../library";

  let books: LibraryBook[] = [];
  let loading = true;
  let busy = false;
  let status = "";
  let failedCovers = new Set<string>();

  onMount(async () => {
    try {
      books = await listBooks();
    } catch {
      status = "无法读取本地书架。";
    } finally {
      loading = false;
    }
  });

  async function chooseBooks() {
    if (busy) return;
    if (!libraryAvailable) {
      status = "请在 Atha 桌面应用中选择 EPUB。";
      return;
    }
    busy = true;
    status = "正在导入…";
    try {
      const report = await importBooks();
      if (!report) {
        status = "";
        return;
      }
      books = report.books;
      if (report.failures.length === 0) {
        status = "已加入书架。";
      } else {
        status = report.failures
          .map((failure) => `${failure.name}：${importFailureMessage(failure.code)}`)
          .join("；");
      }
    } catch {
      status = "无法导入所选书籍。";
    } finally {
      busy = false;
    }
  }

  async function backup() {
    if (busy) return;
    if (!libraryAvailable) {
      status = "请在 Atha 桌面应用中备份消息。";
      return;
    }
    busy = true;
    status = "正在创建消息备份…";
    try {
      status = (await backupMessages()) ? "消息备份已保存。" : "";
    } catch {
      status = "无法创建消息备份。";
    } finally {
      busy = false;
    }
  }

  async function restore() {
    if (busy) return;
    if (!libraryAvailable) {
      status = "请在 Atha 桌面应用中恢复消息。";
      return;
    }
    if (!confirm("恢复会替换全部标注、笔记和对话。请先关闭其他 Atha 窗口。继续？")) return;
    busy = true;
    status = "正在验证并恢复消息备份…";
    try {
      status = (await restoreMessages()) ? "消息已从备份恢复。" : "";
    } catch {
      status = "无法恢复：备份无效、数据库正被占用，或文件不可读取。";
    } finally {
      busy = false;
    }
  }

  async function read(book: LibraryBook) {
    if (busy) return;
    busy = true;
    status = "";
    try {
      await openBook(book.id);
    } catch {
      status = `无法打开《${book.title}》，请重新导入。`;
      busy = false;
    }
  }

  async function remove(book: LibraryBook) {
    if (busy || !confirm(`从书架移除《${book.title}》？阅读进度和导入内容会保留。`)) return;
    busy = true;
    status = "";
    try {
      books = await removeBook(book.id);
      status = "已从书架移除。";
    } catch {
      status = "无法从书架移除这本书。";
    } finally {
      busy = false;
    }
  }

  function markCoverFailed(id: string) {
    failedCovers = new Set(failedCovers).add(id);
  }
</script>

<main class="library-shell" aria-label="Atha 书架" aria-busy={loading || busy}>
  <header class="library-header">
    <div>
      <span class="library-brand">Atha</span>
      <h1>书架</h1>
    </div>
    <div class="library-actions">
      <button
        class="library-header-button library-maintenance-button"
        type="button"
        aria-label="备份全部消息"
        title="备份全部消息"
        onclick={backup}
        disabled={busy}
      >
        <Archive aria-hidden="true" />
        <span>备份</span>
      </button>
      <button
        class="library-header-button library-maintenance-button"
        type="button"
        aria-label="恢复全部消息"
        title="恢复全部消息"
        onclick={restore}
        disabled={busy}
      >
        <ArchiveRestore aria-hidden="true" />
        <span>恢复</span>
      </button>
      <button
        class="library-header-button library-import-button"
        type="button"
        onclick={chooseBooks}
        disabled={busy}
      >
        <Plus aria-hidden="true" />
        <span>导入</span>
      </button>
    </div>
  </header>

  {#if loading}
    <section class="library-empty" aria-label="正在读取书架">
      <div class="library-loading" aria-hidden="true"></div>
      <p>正在读取书架…</p>
    </section>
  {:else if books.length === 0}
    <section class="library-empty">
      <BookOpen aria-hidden="true" />
      <h2>开始你的书架</h2>
      <p>从电脑中选择 EPUB，导入后即可随时继续阅读。</p>
      <button type="button" onclick={chooseBooks} disabled={busy}>
        选择 EPUB
      </button>
    </section>
  {:else}
    <section class="library-grid" aria-label="书籍">
      {#each books as book (book.id)}
        <article class="library-book">
          <button class="library-book-open" type="button" onclick={() => read(book)} disabled={busy}>
            <span class="library-cover">
              {#if book.hasCover && !failedCovers.has(book.id)}
                <img
                  src={coverUrl(book.id)}
                  alt=""
                  draggable="false"
                  onerror={() => markCoverFailed(book.id)}
                />
              {:else}
                <span aria-hidden="true">{book.title.slice(0, 1)}</span>
              {/if}
            </span>
            <span class="library-book-title">{book.title}</span>
            <span class="library-book-author">{book.authors.join(" / ") || "未知作者"}</span>
          </button>
          <button
            class="library-book-remove"
            type="button"
            aria-label={`从书架移除《${book.title}》`}
            title="从书架移除"
            onclick={() => remove(book)}
            disabled={busy}
          >
            <Trash2 aria-hidden="true" />
          </button>
        </article>
      {/each}
    </section>
  {/if}

  {#if status}
    <p class="library-status" role="status">{status}</p>
  {/if}
</main>
