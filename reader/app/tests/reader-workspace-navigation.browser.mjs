import assert from "node:assert/strict";

const quote = "石桥下的第九盏灯只在退潮时亮起。";
const before = Array.from({ length: 64 }, (_, i) =>
  `手记 ${i + 1}：清晨河风把树影推上石阶，渡口的人们按旧例在薄雾里等候。`);
const after = Array.from({ length: 64 }, (_, i) =>
  `归程 ${i + 1}：有人记得船帆的颜色，有人只记得抵岸时湿透的鞋袜。`);
const note = "回跳应恢复石桥下的灯";
const offset = before.join("").length;
const progressKey = "atha.reader.progress.a1b2c3d4e5f60718.v1";

async function waitForOriginal(page) {
  await page.waitForFunction((quote) => {
    const paper = document.querySelector(".page").getBoundingClientRect();
    const frame = document.querySelector(".reader-frame").getBoundingClientRect();
    const ranges = ["atha-annotations", "atha-active-annotation"].flatMap(name =>
      [...CSS.highlights.get(name) ?? []]).filter(range => range.toString() === quote);
    return ranges.length === 1 && [...ranges[0].getClientRects()].some(rect => {
      const left = Math.max(rect.left, paper.left, frame.left);
      const right = Math.min(rect.right, paper.right, frame.right);
      const top = Math.max(rect.top, paper.top, frame.top);
      const bottom = Math.min(rect.bottom, paper.bottom, frame.bottom);
      return right > left && bottom > top &&
        document.elementFromPoint((left + right) / 2, (top + bottom) / 2)?.matches(".reader-content-host");
    });
  }, quote);
}

async function progress(page) {
  return page.evaluate(key => JSON.parse(JSON.parse(localStorage.getItem(key)).locator).start, progressKey);
}

async function verifyAtWidth(browser, url, width) {
  // Keep the real pane outros: a read just after a Svelte update may see a transitioning flex box.
  const page = await browser.newPage({ viewport: { width, height: 713 } });
  const regionA = page.locator(".region-a");
  const thread = page.locator(".pane-c");
  const close = async () => {
    await thread.getByRole("button", { name: "关闭笔记工作区" }).click();
    await thread.waitFor({ state: "hidden" });
    await page.waitForFunction(() => {
      const frame = document.querySelector(".reader-frame");
      const reader = frame?.querySelector(".reader");
      return Math.abs(reader?.getBoundingClientRect().width - frame?.clientWidth) <= 1 &&
        frame?.clientWidth === window.innerWidth;
    });
  };
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  try {
    await page.route("**/demo/ch1.xhtml", route => route.fulfill({
      contentType: "application/xhtml+xml",
      body: `<html xmlns="http://www.w3.org/1999/xhtml"><head><title>渡口手记</title></head><body>${[...before, quote, ...after].map(text => `<p>${text}</p>`).join("")}</body></html>`,
    }));
    await page.goto(url);
    await page.getByRole("tab", { name: "阅读记忆" }).click();
    await page.getByRole("heading", { name: "最近记忆片段" }).waitFor();
    const sourceLocator = await page.evaluate(({ quote, note, offset, progressKey }) => {
      const key = "atha.demo.messages.v1";
      const store = JSON.parse(localStorage.getItem(key));
      const { messages } = store.conversations[0];
      const root = messages[0];
      const locator = JSON.parse(root.source.canonicalLocator);
      locator.start.offset = offset;
      locator.end.offset = offset + quote.length;
      root.source.originalLocator = root.source.canonicalLocator = JSON.stringify(locator);
      root.source.selectedText = quote;
      root.source.prefixText = root.source.suffixText = "";
      messages[1].text = note;
      localStorage.setItem(key, JSON.stringify(store));
      localStorage.removeItem(progressKey);
      return root.source.canonicalLocator;
    }, { quote, note, offset, progressKey });

    await page.getByRole("searchbox", { name: "搜索笔记与回复" }).fill(note);
    await page.locator(".hit-row").filter({ hasText: note }).getByRole("button", { name: "跳回原书" }).click();
    await page.waitForSelector(".reader-root[data-screen-ready=true] .pane-c");
    await thread.getByText(quote, { exact: true }).waitFor();

    const medium = width === 1000;
    if (medium) {
      await regionA.waitFor({ state: "hidden" });
      // Let the kernel's window-resize debounce finish while only the workspace is visible.
      await page.setViewportSize({ width: 1010, height: 713 });
      await page.locator("html[data-viewport-stable]").waitFor({ state: "attached" });
    } else if (width >= 1440) {
      await waitForOriginal(page);
      await thread.getByRole("button", { name: "专注对话" }).click();
      await regionA.waitFor({ state: "hidden" });
      await thread.getByRole("button", { name: "退出专注" }).click();
      await waitForOriginal(page);
    }

    await close();
    await waitForOriginal(page);

    if (medium) {
      // Later reading actions supersede the memory jump, even in the same chapter.
      const next = async () => {
        const previous = await progress(page);
        await page.locator(".reader-shell").focus();
        await page.keyboard.press("ArrowRight");
        await page.waitForFunction(({ progressKey, previous }) => {
          const { start } = JSON.parse(JSON.parse(localStorage.getItem(progressKey)).locator);
          return start.section === previous.section && start.offset > previous.offset;
        }, { progressKey, previous });
      };
      await next();
      await next();
      const later = await progress(page);
      assert.ok(later.offset > offset + quote.length);
      const openThread = async () => {
        await page.keyboard.press("n");
        // Opening the notes list really reflows A before B+C hides it.
        await page.waitForFunction(() => {
          const frame = document.querySelector(".reader-frame");
          const reader = frame?.querySelector(".reader");
          return frame?.clientWidth > 0 && frame.clientWidth < innerWidth - 100 &&
            Math.abs(reader?.getBoundingClientRect().width - frame.clientWidth) <= 1;
        });
        await page.locator(".note-index-row", { hasText: quote }).click();
        await regionA.waitFor({ state: "hidden" });
      };
      await openThread();
      await close();
      assert.deepEqual(await progress(page), later);

      // An explicit jump from inside B+C replaces that later position.
      await openThread();
      await thread.getByRole("button", { name: "跳回原文" }).click();
      await page.waitForFunction(({ progressKey, later }) =>
        JSON.parse(JSON.parse(localStorage.getItem(progressKey)).locator).start.offset < later.offset,
      { progressKey, later });
      await close();
      await waitForOriginal(page);
    }

    assert.equal(await page.evaluate(() =>
      JSON.parse(localStorage.getItem("atha.demo.messages.v1")).conversations[0].messages[0].source.canonicalLocator
    ), sourceLocator);
    assert.deepEqual(errors, []);
    assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
    await page.locator(".memory-shell").getByText(note, { exact: true }).waitFor();
  } finally {
    await page.close();
  }
}

export async function verifyReaderWorkspaceNavigation(browser, url) {
  for (const width of [1000, 360, 1500]) await verifyAtWidth(browser, url, width);
}
