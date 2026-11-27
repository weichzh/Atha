import assert from "node:assert/strict";
import test from "node:test";
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
