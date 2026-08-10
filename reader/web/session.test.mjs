import assert from "node:assert/strict";
import test from "node:test";

import { createReadingSession } from "./session.mjs";

test("section changes keep the live content and prefetch adjacent sections", async (context) => {
  const previousDocument = globalThis.document;
  const previousFetch = globalThis.fetch;
  globalThis.document = { documentElement: { dataset: {} } };
  globalThis.fetch = async () => ({
    ok: true,
    json: async () => ({
      schema: 1,
      contentVersion: "a".repeat(64),
      sections: [
        { id: "one", href: "one.xhtml" },
        { id: "two", href: "two.xhtml" },
        { id: "three", href: "three.xhtml" },
      ],
      resources: [],
    }),
  });
  context.after(() => {
    globalThis.document = previousDocument;
    globalThis.fetch = previousFetch;
  });

  const loads = [];
  const prefetched = [];
  let closes = 0;
  let active = null;
  let failingPath = null;
  let failingRenderPath = null;
  let blockedPath = null;
  let blockedStarted;
  let unblock;
  let terminalFailures = 0;
  const session = createReadingSession({
    params: new URLSearchParams("manifest=.atha-reader.json"),
    content: {
      close() {
        closes += 1;
        active = null;
      },
      async prepareSection(url) {
        loads.push(url.pathname);
        if (url.pathname === failingPath) throw new Error("section-load");
        if (url.pathname === blockedPath) {
          blockedStarted();
          await new Promise((resolve) => {
            unblock = resolve;
          });
        }
        return { url };
      },
      activateSection(section) {
        active = section.url.pathname;
      },
      prefetchSection(url) {
        prefetched.push(url.pathname);
      },
    },
    render: async () => {
      if (active === failingRenderPath) throw new Error("unstable-layout");
    },
    onState: () => {},
    assert(condition, code) {
      if (!condition) throw new Error(code);
    },
    fail(code) {
      terminalFailures += 1;
      throw new Error(code);
    },
  });

  assert.equal(await session.open(0), true);
  assert.equal(await session.open(1), true);
  assert.deepEqual(loads, ["/one.xhtml", "/two.xhtml"]);
  assert.deepEqual(prefetched, ["/two.xhtml", "/one.xhtml", "/three.xhtml"]);
  assert.equal(closes, 0);
  assert.equal(session.snapshot().currentIndex, 1);
  assert.equal(active, "/two.xhtml");
  assert.equal(globalThis.document.documentElement.dataset.error, undefined);
  assert.equal(terminalFailures, 0);

  failingPath = "/three.xhtml";
  assert.equal(await session.open(2), false);
  assert.deepEqual(loads, ["/one.xhtml", "/two.xhtml", "/three.xhtml"]);
  assert.equal(session.snapshot().state, "layout-stable");
  assert.equal(session.snapshot().currentIndex, 1);
  assert.equal(globalThis.document.documentElement.dataset.sectionPosition, "2 / 3");
  assert.equal(active, "/two.xhtml");
  assert.equal(closes, 0);

  failingPath = null;
  failingRenderPath = "/three.xhtml";
  assert.equal(await session.open(2), false);
  assert.deepEqual(loads, [
    "/one.xhtml",
    "/two.xhtml",
    "/three.xhtml",
    "/three.xhtml",
    "/two.xhtml",
  ]);
  assert.equal(session.snapshot().state, "layout-stable");
  assert.equal(session.snapshot().currentIndex, 1);
  assert.equal(active, "/two.xhtml");
  assert.equal(globalThis.document.documentElement.dataset.error, undefined);
  assert.equal(terminalFailures, 0);

  assert.equal(await session.open(99), false);
  assert.equal(session.snapshot().state, "layout-stable");
  assert.equal(session.snapshot().currentIndex, 1);
  assert.equal(active, "/two.xhtml");
  assert.equal(globalThis.document.documentElement.dataset.error, undefined);
  assert.equal(terminalFailures, 0);

  failingRenderPath = null;
  blockedPath = "/one.xhtml";
  const started = new Promise((resolve) => {
    blockedStarted = resolve;
  });
  const opening = session.open(0);
  await started;
  session.close();
  unblock();
  assert.equal(await opening, false);
  assert.equal(closes, 1);
  assert.equal(active, null);
  assert.equal(session.snapshot().state, "closed");
  assert.equal(session.snapshot().currentIndex, -1);
});
