import assert from "node:assert/strict";
import test from "node:test";
import { createLocator } from "./locator.mjs";
import { createNavigation } from "./navigation.mjs";

test("explicit note targets use the declared URL, including WebView custom schemes", async () => {
  for (const root of ["https://atha-book.localhost/", "atha-book://localhost/"]) {
    let index = 0;
    let shownOffset = null;
    const sections = ["one", "two"].map(id => ({
      id, href: `${id}.xhtml`, url: new URL(`${id}.xhtml`, root).href,
    }));
    const session = {
      describe: () => ({ sections, toc: [] }),
      snapshot: () => ({ currentIndex: index, currentSection: sections[index].id, sections: sections.length }),
      async open(nextIndex) { index = nextIndex; return true; },
    };
    const navigation = createNavigation({
      session,
      pagination: {
        snapshot: () => ({ page: 0, pages: 1 }),
        captureOffset: () => 0,
        offsetForFragment: (id) => id === "endnote" ? 42 : null,
        async showOffset(offset) { shownOffset = offset; return true; },
        async show() { assert.fail("Expected an exact offset, not a fallback"); },
      },
      toc: {},
      chapterLabel: {},
      progressChapter: {},
      progressBook: {},
      progressPosition: {},
      progressRange: new EventTarget(),
      previous: {},
      next: {},
      onFallback: assert.fail,
      onStable() {},
      assert: assert.ok,
      fail: assert.fail,
    });
    assert.equal(await navigation.goToHref(`${sections[1].url}#endnote`), true);
    assert.equal(index, 1);
    assert.equal(shownOffset, 42);
  }
});

test("workspace reflow keeps the exact jump until a later page, section, or scroll takes over", async () => {
  const sections = ["one", "two"].map(id => ({
    id, href: `${id}.xhtml`, url: `https://atha-book.localhost/${id}.xhtml`,
  }));
  const book = { contentVersion: "a".repeat(64), sections, toc: [] };
  let index = 0;
  let page = 0;
  let visible = true;
  let scrolling = false;
  let scrollTop = 0;
  const reflows = [];
  const locator = createLocator({ assert: assert.ok });
  const navigation = createNavigation({
    session: {
      describe: () => book,
      snapshot: () => ({ currentIndex: index, currentSection: sections[index].id, sections: sections.length }),
      async open(nextIndex) { index = nextIndex; return true; },
    },
    pagination: {
      snapshot: () => ({ page, pages: 10 }),
      captureOffset: () => page * 10 + scrollTop,
      isScrolled: () => scrolling,
      scrollPosition: () => scrollTop,
      async showOffset(offset) { page = Math.floor(offset / 10); return true; },
      async show(target) { page = target; },
      async move(delta) { page += delta; return true; },
      async resizeViewport(offset) { reflows.push(offset); },
    },
    locator,
    canResizeViewport: () => visible,
    toc: {},
    chapterLabel: {},
    progressChapter: {},
    progressBook: {},
    progressPosition: {},
    progressRange: new EventTarget(),
    previous: {},
    next: {},
    onFallback: assert.fail,
    onStable() {},
    assert: assert.ok,
    fail: assert.fail,
  });

  await navigation.goTo(locator.point(book, "one", 43));
  assert.equal(navigation.current().start.offset, 40, "progress still reports the live page's start");
  const gated = navigation.resize();
  visible = false; // The body disappears after enqueue, before the resize reads its geometry.
  assert.equal(await gated, null);
  assert.deepEqual(reflows, []);

  visible = true;
  await navigation.resize();
  await navigation.next();
  await navigation.resize();
  await navigation.goToHref(sections[1].url);
  await navigation.resize(43, 0, 4); // A late image from the previous section cannot resize this one.
  await navigation.resize();
  scrolling = true;
  await navigation.goTo(locator.point(book, "two", 28));
  await navigation.resize();
  scrollTop = 35;
  await navigation.resize();
  assert.deepEqual(reflows, [43, 50, 0, 28, 55]);
});
