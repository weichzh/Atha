const MAX_QUERY_LENGTH = 128;
const MAX_RESULTS = 2000;
const MAX_SECTION_LENGTH = 16 * 1024 * 1024;
const HIDDEN_SENTINEL = "\0";

export function createSearch({ session, navigation, pagination, locator, controls, assert }) {
  let active = null;
  let results = [];
  let status = "idle";
  let query = "";
  let truncated = false;
  let lastError = null;
  let lastJump = null;

  function sectionLabel(description, section) {
    return (
      description.toc.find((item) => item.href.split("#", 1)[0] === section.href)?.label ||
      section.id
    );
  }

  function sectionText(source) {
    if (source.length > MAX_SECTION_LENGTH) throw new Error("search-section-size");
    const documentNode = new DOMParser().parseFromString(source, "application/xhtml+xml");
    if (
      documentNode.querySelector("parsererror") ||
      documentNode.doctype ||
      !documentNode.body ||
      documentNode.querySelector(
        "script, iframe, frame, object, embed, form, input, button, select, textarea, video, audio, source, track, base, meta[http-equiv], foreignObject",
      )
    ) {
      throw new Error("search-section");
    }
    for (const element of documentNode.querySelectorAll("*")) {
      if ([...element.attributes].some((attribute) => attribute.name.toLowerCase().startsWith("on"))) {
        throw new Error("search-section");
      }
    }
    for (const element of documentNode.querySelectorAll("style, link[rel='stylesheet']")) {
      element.remove();
    }
    let text = "";
    const walker = documentNode.createTreeWalker(documentNode.body, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const value = walker.currentNode.textContent || "";
      let hidden = false;
      for (
        let element = walker.currentNode.parentElement;
        element && element !== documentNode.body;
        element = element.parentElement
      ) {
        const inlineStyle = element.style;
        if (
          element.localName === "noscript" ||
          element.hasAttribute("hidden") ||
          element.hasAttribute("inert") ||
          element.getAttribute("aria-hidden")?.toLowerCase() === "true" ||
          inlineStyle?.display === "none" ||
          inlineStyle?.visibility === "hidden" ||
          inlineStyle?.contentVisibility === "hidden"
        ) {
          hidden = true;
          break;
        }
      }
      text += hidden ? HIDDEN_SENTINEL.repeat(value.length) : value;
    }
    return text;
  }

  function excerpt(text, start, end) {
    return text
      .slice(Math.max(0, start - 32), Math.min(text.length, end + 48))
      .replaceAll(HIDDEN_SENTINEL, " ")
      .replace(/\s+/gu, " ")
      .trim();
  }

  function sync() {
    controls.results.replaceChildren(
      ...results.map((result) => {
        const option = document.createElement("option");
        option.value = result.id;
        option.textContent = `${result.sectionLabel} · ${result.excerpt}`;
        return option;
      }),
    );
    controls.go.disabled = results.length === 0;
    controls.cancel.disabled = status !== "searching";
    controls.status.dataset.error = String(status === "error");
    controls.status.textContent = {
      idle: "",
      searching: "正在搜索…",
      canceled: "已取消",
      complete: truncated ? `找到 ${results.length} 条，已达上限` : `找到 ${results.length} 条`,
      error: lastError || "搜索失败",
    }[status];
  }

  function cancel() {
    if (!active) return false;
    active.abort();
    active = null;
    results = [];
    truncated = false;
    status = "canceled";
    sync();
    return true;
  }

  async function search(value) {
    cancel();
    query = String(value).trim();
    results = [];
    truncated = false;
    lastError = null;
    lastJump = null;
    if (!query || query.length > MAX_QUERY_LENGTH || query.includes(HIDDEN_SENTINEL)) {
      status = "error";
      lastError = "请输入 1 至 128 个字符";
      sync();
      return snapshot();
    }

    const controller = new AbortController();
    active = controller;
    status = "searching";
    sync();
    const description = session.describe();
    const pattern = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&"), "giu");
    try {
      sections: for (const section of description.sections) {
        const response = await fetch(section.url, { signal: controller.signal });
        if (!response.ok) throw new Error("search-section");
        const text = sectionText(await response.text());
        for (const match of text.matchAll(pattern)) {
          if (controller.signal.aborted) throw new DOMException("aborted", "AbortError");
          const start = match.index;
          const end = start + match[0].length;
          results.push(
            Object.freeze({
              id: String(results.length),
              section: section.id,
              sectionLabel: sectionLabel(description, section),
              excerpt: excerpt(text, start, end),
              locator: locator.serialize(
                description,
                locator.range(
                  description,
                  { section: section.id, offset: start },
                  { section: section.id, offset: end },
                ),
              ),
            }),
          );
          if (results.length === MAX_RESULTS) {
            truncated = true;
            break sections;
          }
        }
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
      if (active !== controller) return Object.freeze({ status: "canceled" });
      active = null;
      status = "complete";
      sync();
      return snapshot();
    } catch (error) {
      if (active !== controller || (error instanceof DOMException && error.name === "AbortError")) {
        return Object.freeze({ status: "canceled" });
      }
      if (active === controller) {
        active = null;
        results = [];
        status = "error";
        lastError = "无法搜索此章节";
        sync();
      }
      return snapshot();
    }
  }

  async function go() {
    const result = results.find((item) => item.id === controls.results.value);
    if (!result || !(await navigation.goTo(result.locator))) {
      lastError = "搜索结果位置已失效";
      status = "error";
      sync();
      return false;
    }
    const target = locator.parse(session.describe(), result.locator);
    lastJump = Object.freeze({
      section: target.start.section,
      offset: target.start.offset,
      visible:
        session.snapshot().currentSection === target.start.section &&
        pagination.isOffsetVisible(target.start.offset),
    });
    if (!lastJump.visible) {
      lastError = "搜索结果位置已失效";
      status = "error";
      sync();
      return false;
    }
    return true;
  }

  function bind() {
    controls.form.addEventListener("submit", (event) => {
      event.preventDefault();
      void search(controls.query.value);
    });
    controls.cancel.addEventListener("click", cancel);
    controls.go.addEventListener("click", () => void go());
    controls.results.addEventListener("dblclick", () => void go());
    sync();
  }

  async function verify() {
    const before = locator.serialize(session.describe(), navigation.current());
    assert(
      sectionText("<html xmlns='http://www.w3.org/1999/xhtml'><body>safe</body></html>") ===
        "safe",
      "sample-boundary",
    );
    const hiddenText = sectionText(
      "<html xmlns='http://www.w3.org/1999/xhtml'><body><p>before</p><span hidden='hidden'>xxxxxx</span><p>after</p></body></html>",
    );
    assert(
      hiddenText.length === "beforexxxxxxafter".length &&
        hiddenText.indexOf("after") === "beforexxxxxx".length &&
        !hiddenText.includes("before      after"),
      "sample-boundary",
    );
    let activeRejected = false;
    try {
      sectionText(
        "<html xmlns='http://www.w3.org/1999/xhtml'><body><script>unsafe</script></body></html>",
      );
    } catch {
      activeRejected = true;
    }
    assert(activeRejected, "sample-boundary");
    const replaced = search("atha-search-replaced");
    const replacement = search("atha-search-final");
    assert((await replaced).status === "canceled", "sample-boundary");
    assert((await replacement).status === "complete", "sample-boundary");
    const explicit = search("atha-search-cancel");
    assert(cancel() && (await explicit).status === "canceled", "sample-boundary");
    assert((await search("x".repeat(MAX_QUERY_LENGTH + 1))).status === "error", "sample-boundary");
    assert(
      session.snapshot().state === "layout-stable" &&
        locator.serialize(session.describe(), navigation.current()) === before,
      "sample-boundary",
    );
    return Object.freeze({
      replaced: true,
      canceled: true,
      errorIsolated: true,
      activeContentRejected: true,
    });
  }

  function snapshot() {
    return Object.freeze({
      status,
      query,
      truncated,
      lastError,
      lastJump,
      count: results.length,
      sections: Object.freeze([...new Set(results.map((result) => result.section))]),
      results: Object.freeze(results.map((result) => Object.freeze({ ...result }))),
    });
  }

  return Object.freeze({ bind, cancel, go, search, snapshot, verify });
}
