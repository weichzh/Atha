const MAX_MANIFEST_SECTIONS = 2000;
const MAX_RESOURCES = 10000;
const MAX_TOC_ITEMS = 2000;
const BOOK_ROOT_URL =
  typeof location !== "undefined" && location.protocol === "tauri:"
    ? "atha-book://localhost/"
    : "https://atha-book.localhost/";

export function createReadingSession({ params, content, render, onState, assert, fail }) {
  let manifest;
  let currentIndex = -1;
  let state = "closed";
  let contentLoads = 0;
  let stableLayouts = 0;
  let closes = 0;

  function invalidManifest() {
    throw new Error("invalid-manifest");
  }

  function manifestAssert(condition) {
    if (!condition) invalidManifest();
  }

  function exactKeys(value, keys) {
    const actual = Object.keys(value).sort();
    const expected = [...keys].sort();
    manifestAssert(
      actual.length === expected.length && actual.every((key, index) => key === expected[index]),
    );
  }

  function safePath(value, extensions = null) {
    manifestAssert(
      typeof value === "string" &&
        value.length > 0 &&
        value.length <= 512 &&
        !/[\\%:?#\u0000-\u001f\u007f]/.test(value) &&
        !value.startsWith("/") &&
        value.split("/").every((part) => part && part !== "." && part !== "..") &&
        (!extensions || extensions.some((extension) => value.toLowerCase().endsWith(extension))),
    );
    return value;
  }

  function validateManifest(value, sourceUrl) {
    manifestAssert(value && typeof value === "object" && !Array.isArray(value));
    const keys = ["schema", "contentVersion", "sections", "resources"];
    if (Object.hasOwn(value, "toc")) keys.push("toc");
    exactKeys(value, keys);
    manifestAssert(value.schema === 1 && /^[a-f0-9]{64}$/.test(value.contentVersion));
    manifestAssert(
      Array.isArray(value.sections) &&
        value.sections.length > 0 &&
        value.sections.length <= MAX_MANIFEST_SECTIONS,
    );
    manifestAssert(Array.isArray(value.resources) && value.resources.length <= MAX_RESOURCES);

    const rootUrl = new URL(".", sourceUrl);
    const sectionIds = new Set();
    const sectionPaths = new Set();
    const sections = value.sections.map((section) => {
      manifestAssert(section && typeof section === "object" && !Array.isArray(section));
      exactKeys(section, ["id", "href"]);
      manifestAssert(typeof section.id === "string" && /^[a-z0-9][a-z0-9._-]{0,63}$/.test(section.id));
      const href = safePath(section.href);
      manifestAssert(!sectionIds.has(section.id) && !sectionPaths.has(href));
      sectionIds.add(section.id);
      sectionPaths.add(href);
      return Object.freeze({ id: section.id, href, url: new URL(href, rootUrl) });
    });
    const resourcePaths = new Set();
    const resourceUrls = new Set();
    for (const resource of value.resources) {
      const path = safePath(resource, [".css", ".svg", ".png", ".jpg", ".jpeg", ".gif", ".webp"]);
      manifestAssert(!resourcePaths.has(path));
      resourcePaths.add(path);
      resourceUrls.add(new URL(path, rootUrl).href);
    }

    if (Object.hasOwn(value, "toc")) {
      manifestAssert(Array.isArray(value.toc) && value.toc.length <= MAX_TOC_ITEMS);
    }
    const tocItems = new Set();
    const toc = value.toc?.map((item) => {
      manifestAssert(item && typeof item === "object" && !Array.isArray(item));
      exactKeys(item, ["label", "href"]);
      manifestAssert(
        typeof item.label === "string" && item.label.trim().length > 0 && item.label.length <= 256,
      );
      manifestAssert(typeof item.href === "string" && item.href.length <= 768);
      const [path, fragment, ...extra] = item.href.split("#");
      manifestAssert(extra.length === 0 && sectionPaths.has(safePath(path)));
      if (fragment !== undefined) {
        manifestAssert(
          fragment.length > 0 &&
            fragment.length <= 256 &&
            !/[\\%?#\u0000-\u001f\u007f]/.test(fragment),
        );
      }
      manifestAssert(!tocItems.has(item.href));
      tocItems.add(item.href);
      return Object.freeze({ label: item.label, href: item.href });
    });
    const frozenToc = Object.freeze(toc || []);
    return Object.freeze({
      contentVersion: value.contentVersion,
      sections: Object.freeze(sections),
      resources: resourceUrls,
      toc: frozenToc,
      strictResources: true,
      description: Object.freeze({
        contentVersion: value.contentVersion,
        sections: Object.freeze(
          sections.map(({ id, href, url }) => Object.freeze({ id, href, url: url.href })),
        ),
        toc: frozenToc,
      }),
    });
  }

  function rejected(value) {
    try {
      validateManifest(value, new URL("https://atha-book.localhost/.atha-reader.json"));
      return false;
    } catch {
      return true;
    }
  }

  function validatorSelfCheck() {
    assert(MAX_MANIFEST_SECTIONS === 2000, "invalid-manifest-section-limit");
    const valid = {
      schema: 1,
      contentVersion: "a".repeat(64),
      sections: [{ id: "one", href: "text/one.xhtml" }],
      resources: ["styles/book.css"],
      toc: [{ label: "One", href: "text/one.xhtml#start" }],
    };
    validateManifest(valid, new URL("https://atha-book.localhost/.atha-reader.json"));
    validateManifest(
      {
        ...valid,
        sections: [{ id: "one", href: "text/one" }],
        toc: [{ label: "One", href: "text/one#start" }],
      },
      new URL("https://atha-book.localhost/.atha-reader.json"),
    );
    for (const invalid of [
      { ...valid, schema: 2 },
      { ...valid, unknown: true },
      { ...valid, sections: [{ id: "one", href: "../one.xhtml" }] },
      { ...valid, sections: [...valid.sections, ...valid.sections] },
      { ...valid, resources: ["https://example.com/image.png"] },
      { ...valid, toc: [{ label: "Elsewhere", href: "text/two.xhtml" }] },
      { ...valid, toc: [...valid.toc, ...valid.toc] },
    ]) {
      assert(rejected(invalid), "invalid-manifest");
    }
  }

  function configuredBookUrl() {
    const override = params.get("book");
    if (override) return new URL(override, location.href);
    const entry = params.get("entry");
    assert(entry, "missing-book-url");
    return new URL(entry.replace(/^\/+/, ""), BOOK_ROOT_URL);
  }

  async function loadManifest() {
    if (manifest) return manifest;
    validatorSelfCheck();
    const path = params.get("manifest");
    if (!path) {
      const url = configuredBookUrl();
      const contentVersion = params.get("version");
      assert(/^[a-f0-9]{64}$/.test(contentVersion || ""), "missing-content-version");
      manifest = Object.freeze({
        contentVersion,
        sections: Object.freeze([Object.freeze({ id: "entry", href: url.pathname, url })]),
        resources: null,
        toc: Object.freeze([]),
        strictResources: false,
        description: Object.freeze({
          contentVersion,
          sections: Object.freeze([
            Object.freeze({ id: "entry", href: url.pathname, url: url.href }),
          ]),
          toc: Object.freeze([]),
        }),
      });
      return manifest;
    }
    if (params.has("book") || params.has("entry")) fail("invalid-manifest");
    try {
      safePath(path.startsWith("/") ? path.slice(1) : path, [".json"]);
    } catch {
      fail("invalid-manifest");
    }
    const sourceUrl = new URL(path.replace(/^\/+/, ""), BOOK_ROOT_URL);
    let value;
    try {
      const response = await fetch(sourceUrl);
      if (!response.ok) fail("manifest-load");
      value = await response.json();
    } catch (error) {
      if (error instanceof Error && error.message === "manifest-load") throw error;
      fail("invalid-manifest");
    }
    try {
      manifest = validateManifest(value, sourceUrl);
    } catch {
      fail("invalid-manifest");
    }
    return manifest;
  }

  function release(nextState) {
    content.close();
    delete document.documentElement.dataset.sectionPosition;
    currentIndex = -1;
    state = nextState;
    onState(state);
  }

  async function open(index = 0) {
    if (state !== "closed") {
      release("closed");
      closes += 1;
    }
    state = "opening";
    onState(state);
    try {
      const book = await loadManifest();
      if (!Number.isInteger(index) || index < 0 || index >= book.sections.length) {
        fail("section-index");
      }
      const section = book.sections[index];
      document.documentElement.dataset.sectionPosition = `${index + 1} / ${book.sections.length}`;
      await content.loadSection(
        section.url,
        book.strictResources ? book.resources : null,
        book.strictResources ? "section-load" : "book-load",
      );
      currentIndex = index;
      contentLoads += 1;
      state = "content-loaded";
      onState(state);
      await render();
      stableLayouts += 1;
      state = "layout-stable";
      onState(state);
    } catch (error) {
      release("failed");
      throw error;
    }
  }

  function close() {
    if (state === "closed") return;
    release("closed");
    closes += 1;
  }

  function describe() {
    assert(manifest, "invalid-manifest");
    return manifest.description;
  }

  function snapshot() {
    return Object.freeze({
      state,
      contentVersion: manifest?.contentVersion ?? null,
      sections: manifest?.sections.length ?? 0,
      tocItems: manifest?.toc.length ?? 0,
      currentIndex,
      currentSection: currentIndex >= 0 ? manifest.sections[currentIndex].id : null,
      contentLoads,
      stableLayouts,
      closes,
    });
  }

  return Object.freeze({ close, describe, open, snapshot });
}
