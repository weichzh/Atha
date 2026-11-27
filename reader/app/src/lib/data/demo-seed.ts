import type { LocalDictionary, DictionaryLookup } from "../../dictionary.ts";
import type { LibraryBook } from "../../library.ts";
import type { ConversationView, EditionInput, RevisionView, SourceCaptureView } from "../../messages.ts";

export const DEMO_PRIMARY_BOOK_ID =
  "a1b2c3d4e5f60718293a4b5c6d7e8f9011223344556677889900aabbccddeeff";

const BOOK_PALETTES = [
  { bg: "#ECE6DA", fg: "#2C2620", accent: "#A8442C" },
  { bg: "#DCE2DD", fg: "#1F2A24", accent: "#3B6251" },
  { bg: "#E5DDD5", fg: "#2F231C", accent: "#8C4A32" },
  { bg: "#D9DFE6", fg: "#1C242E", accent: "#3B5978" },
  { bg: "#E7DFD1", fg: "#2D251B", accent: "#916328" },
  { bg: "#272523", fg: "#EFECE6", accent: "#D9775F" },
];

function splitCoverTitleLines(title: string): [string, string] {
  const clean = title.trim();
  const hasSpaces = /\s/.test(clean) && !/[\u4e00-\u9fff]/.test(clean);
  if (hasSpaces) {
    const words = clean.split(/\s+/);
    let line1 = "";
    let line2 = "";
    for (const w of words) {
      if ((line1 + " " + w).trim().length <= 18 && !line2) {
        line1 = (line1 + " " + w).trim();
      } else {
        line2 = (line2 + " " + w).trim();
      }
    }
    if (line2.length > 18) line2 = line2.slice(0, 17) + "…";
    return [line1, line2];
  }
  const chars = Array.from(clean);
  if (chars.length <= 10) return [clean, ""];
  const line1 = chars.slice(0, 10).join("");
  const rest = chars.slice(10);
  const line2 = rest.length > 10 ? rest.slice(0, 9).join("") + "…" : rest.join("");
  return [line1, line2];
}

export function generateDemoCoverSvg(title: string, author: string, seedIndex: number): string {
  const palette = BOOK_PALETTES[seedIndex % BOOK_PALETTES.length];
  const glyph = Array.from(title.trim())[0] ?? "书";
  const [line1, line2] = splitCoverTitleLines(title);
  const esc = (s: string) => s.replace(/[<>&"']/g, "");
  const escapedAuthor = esc(author.slice(0, 22));
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 420" width="300" height="420">
    <rect width="300" height="420" fill="${palette.bg}"/>
    <circle cx="150" cy="162" r="62" fill="${palette.accent}" fill-opacity="0.12"/>
    <circle cx="150" cy="162" r="42" fill="${palette.accent}" fill-opacity="0.16"/>
    <text x="150" y="180" text-anchor="middle" font-family="Songti SC, Georgia, serif" font-size="50" font-weight="600" fill="${palette.accent}">${esc(glyph)}</text>
    <text x="150" y="274" text-anchor="middle" font-family="Songti SC, Georgia, serif" font-size="14.5" font-weight="600" fill="${palette.fg}">${esc(line1)}</text>
    ${line2 ? `<text x="150" y="296" text-anchor="middle" font-family="Songti SC, Georgia, serif" font-size="14.5" font-weight="600" fill="${palette.fg}">${esc(line2)}</text>` : ""}
    <text x="150" y="${line2 ? 332 : 314}" text-anchor="middle" font-family="PingFang SC, sans-serif" font-size="11.5" fill="${palette.fg}" fill-opacity="0.62">${escapedAuthor}</text>
    <rect x="0" y="0" width="8" height="420" fill="${palette.fg}" fill-opacity="0.06"/>
  </svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function makeHex64(prefix: string): string {
  return prefix.padEnd(64, "0").slice(0, 64);
}

function makeHex32(prefix: string): string {
  return prefix.padEnd(32, "0").slice(0, 32);
}

export const DEMO_BOOKS: LibraryBook[] = [
  {
    id: DEMO_PRIMARY_BOOK_ID,
    title: "潮汐与纸页：数字阅读中的对话形态",
    authors: ["林澄", "Arthur Vance"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1764096000000,
    prepared: true,
    favorite: true,
    tags: ["深度共读", "排版美学", "精读"],
    subjects: ["数字人文", "出版设计"],
    series: ["现代书房论集"],
  },
  {
    id: makeHex64("b202"),
    title: "The Elements of Typographic Rhythm",
    authors: ["Eleanor Marsh"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1764009600000,
    prepared: true,
    favorite: true,
    tags: ["Typographic", "经典必读"],
    subjects: ["出版设计", "平面构成"],
    series: ["Atelier Monographs"],
  },
  {
    id: makeHex64("c303"),
    title: "宋版书刻与江南纸坊考",
    authors: ["沈怀瑾"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763923200000,
    prepared: true,
    favorite: false,
    tags: ["文献学", "精读"],
    subjects: ["古典文献", "历史"],
    series: ["金陵丛书"],
  },
  {
    id: makeHex64("d404"),
    title: "建筑的现象学与安静空间",
    authors: ["周砚秋"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763836800000,
    prepared: true,
    favorite: true,
    tags: ["空间笔记", "建筑"],
    subjects: ["建筑美学", "哲学"],
    series: ["营造译丛"],
  },
  {
    id: makeHex64("e505"),
    title: "Margins of the Renaissance",
    authors: ["Marco Bellini", "Clara Lind"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763750400000,
    prepared: true,
    favorite: false,
    tags: ["边注史", "外文原版"],
    subjects: ["历史", "数字人文"],
    series: ["Atelier Monographs"],
  },
  {
    id: makeHex64("f606"),
    title: "软件工具的克制与直觉",
    authors: ["陈默"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763664000000,
    prepared: true,
    favorite: false,
    tags: ["人机交互", "设计系统"],
    subjects: ["计算机科学", "出版设计"],
    series: ["现代书房论集"],
  },
  {
    id: makeHex64("0707"),
    title: "海风集：二十四节气散文选",
    authors: ["江小舟"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763577600000,
    prepared: true,
    favorite: true,
    tags: ["睡前随笔", "精读"],
    subjects: ["散文诗歌"],
    series: [],
  },
  {
    id: makeHex64("1808"),
    title: "Designing Calm Interfaces",
    authors: ["Jonas Lindqvist"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763491200000,
    prepared: true,
    favorite: false,
    tags: ["人机交互", "Typographic"],
    subjects: ["计算机科学", "设计系统"],
    series: [],
  },
  {
    id: makeHex64("2909"),
    title: "长安春深：唐诗地理志",
    authors: ["陆知微"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763404800000,
    prepared: true,
    favorite: false,
    tags: ["诗词考据"],
    subjects: ["古典文献", "散文诗歌"],
    series: ["金陵丛书"],
  },
  {
    id: makeHex64("3a10"),
    title: "Notes on Craft & Paper",
    authors: ["Kenji Takahashi"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763318400000,
    prepared: true,
    favorite: true,
    tags: ["纸张工艺", "睡前随笔"],
    subjects: ["出版设计"],
    series: [],
  },
  {
    id: makeHex64("4b11"),
    title: "城市步行者的制图术",
    authors: ["苏未"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763232000000,
    prepared: true,
    favorite: false,
    tags: ["城市观察"],
    subjects: ["建筑美学", "数字人文"],
    series: ["营造译丛"],
  },
  {
    id: makeHex64("5c12"),
    title: "Local-First Memory Systems",
    authors: ["Hannah Sato"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763145600000,
    prepared: true,
    favorite: false,
    tags: ["软件架构", "深度共读"],
    subjects: ["计算机科学"],
    series: [],
  },
  {
    id: makeHex64("6d13"),
    title: "晚明小品文选注",
    authors: ["张岱 原著", "顾明远 校注"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1763059200000,
    prepared: true,
    favorite: false,
    tags: ["文献学", "睡前随笔"],
    subjects: ["古典文献", "散文诗歌"],
    series: ["金陵丛书"],
  },
  {
    id: makeHex64("7e14"),
    title: "色彩与微光：暗色界面声学",
    authors: ["叶安"],
    hasCover: true,
    hasCustomCover: false,
    importedAt: 1762972800000,
    prepared: true,
    favorite: false,
    tags: ["设计系统"],
    subjects: ["平面构成", "出版设计"],
    series: ["现代书房论集"],
  },
];

export const DEMO_DICTIONARIES: LocalDictionary[] = [
  {
    id: makeHex64("d101"),
    title: "牛津高阶英汉双解词典（离线索引版）",
    format: "mdict",
    entryCount: 142800,
    resourceCount: 64,
    importedAt: 1763800000000,
  },
  {
    id: makeHex64("d202"),
    title: "现代汉语规范词典",
    format: "kindle-mobi6",
    entryCount: 78500,
    resourceCount: 0,
    importedAt: 1763900000000,
  },
];

export const DEMO_DICTIONARY_ENTRIES: Record<string, DictionaryLookup> = {
  palimpsest: {
    dictionaryId: makeHex64("d101"),
    headword: "palimpsest",
    definition: "n. 刮去原字重写之羊皮纸；重写本；留有历史痕迹的多重文本。",
    definitionHtml:
      "<p><strong>pal·imp·sest</strong> <em>/ˈpælɪmpsest/</em> · n.</p><p>1. <strong>重写羊皮纸</strong>：刮去或洗去原有文字后再次书写的古抄本，底层字迹往往若隐若现。</p><p>2. <strong>（喻）多重时间层</strong>：表面覆盖着新层却仍可辨认旧迹的器物、文献或空间。</p>",
  },
  margins: {
    dictionaryId: makeHex64("d101"),
    headword: "margin",
    definition: "n. 页边空白；余地；边缘地带。",
    definitionHtml:
      "<p><strong>mar·gin</strong> <em>/ˈmɑːdʒɪn/</em> · n.</p><p>1. <strong>页边距，留白</strong>：书页正文四周保留的空白区域，用于承载手写评注与视觉呼吸。</p><p>2. <strong>余地，缓冲</strong>：在时间、空间或判断中所留出的从容边界。</p>",
  },
  estuary: {
    dictionaryId: makeHex64("d101"),
    headword: "estuary",
    definition: "n. 河口湾；江海交汇处的潮汐入口。",
    definitionHtml:
      "<p><strong>es·tu·ary</strong> <em>/ˈestʃueri/</em> · n.</p><p>淡水河流与咸水海洋交汇并受潮汐往复作用的宽阔河口。</p>",
  },
  潮汐: {
    dictionaryId: makeHex64("d202"),
    headword: "潮汐",
    definition: "【名】由于月球和太阳引潮力引起的大海周期性涨落现象；比喻往复呼应的节律。",
    definitionHtml:
      "<p><strong>潮汐</strong> <em>cháo xī</em> · 名词</p><p>1. 海水受天体引力影响产生的周期性涨落，白昼为潮，夜间为汐。</p><p>2. 比喻思绪、历史或阅读体验中来回往复的节奏与呼应。</p>",
  },
  留白: {
    dictionaryId: makeHex64("d202"),
    headword: "留白",
    definition: "【动/名】书画、版式与文字中刻意保留的空白之处，以无声衬托有声。",
    definitionHtml:
      "<p><strong>留白</strong> <em>liú bái</em></p><p>中国传统书画及现代版式设计的重要手法：通过克制的空白营造空间呼吸感，使核心笔墨更显从容。</p>",
  },
  书房: {
    dictionaryId: makeHex64("d202"),
    headword: "书房",
    definition: "【名】专门用于藏书、阅读、思考与写作的安静房间。",
    definitionHtml:
      "<p><strong>书房</strong> <em>shū fáng</em> · 名词</p><p>置放典籍、伏案阅读与沉思写字的居室。</p>",
  },
};

export interface DemoMessageStoreSeed {
  editions: Record<string, EditionInput>;
  conversations: ConversationView[];
  revisions: Record<string, RevisionView[]>;
  captures: Record<string, SourceCaptureView[]>;
}

function makeRichDoc(paragraphs: string[]) {
  return JSON.stringify({
    type: "doc",
    content: paragraphs.map((text) => ({
      type: "paragraph",
      content: [{ type: "text", text }],
    })),
  });
}

export function buildDemoMessageSeed(): DemoMessageStoreSeed {
  const conv1 = makeHex32("aa01");
  const root1 = makeHex32("1101");
  const reply1 = makeHex32("1102");
  const reply2 = makeHex32("1103");
  const src1 = makeHex32("5501");

  const conv2 = makeHex32("aa02");
  const root2 = makeHex32("2201");
  const src2 = makeHex32("5502");

  const conv3 = makeHex32("aa03");
  const root3 = makeHex32("3301");
  const reply3 = makeHex32("3302");
  const src3 = makeHex32("5503");

  const conv4 = makeHex32("aa04");
  const root4 = makeHex32("4401");
  const reply4 = makeHex32("4402");
  const src4 = makeHex32("5504");

  const quote1 = "真正的阅读并非单向接收，而是把陌生句子牵引到自己生活的潮汐线旁边，听见两种时间彼此回应。";
  const quote2 = "A well-proportioned page holds silence around every line so the eye can rest between returns, just as an estuary absorbs the surge of incoming water.";
  const quote3 = "历史的归历史，现场的归现场。即使原书已被从本地设备移除，当年那段文字在暖米纸色下的模样依然被封存在只读影子文档之中。";
  const quote4 = "Good typography is an act of hospitality toward the reader's wandering attention.";

  const locator1 = JSON.stringify({
    schema: 1,
    contentVersion: DEMO_PRIMARY_BOOK_ID,
    start: { section: "ch1", offset: 48 },
    end: { section: "ch1", offset: 48 + quote1.length },
  });
  const locator2 = JSON.stringify({
    schema: 1,
    contentVersion: DEMO_PRIMARY_BOOK_ID,
    start: { section: "ch1", offset: 375 },
    end: { section: "ch1", offset: 375 + quote2.length },
  });
  const locator3 = JSON.stringify({
    schema: 1,
    contentVersion: DEMO_PRIMARY_BOOK_ID,
    start: { section: "ch2", offset: 300 },
    end: { section: "ch2", offset: 300 + quote3.length },
  });
  const locator4 = JSON.stringify({
    schema: 1,
    contentVersion: makeHex64("b202"),
    start: { section: "ch1", offset: 12 },
    end: { section: "ch1", offset: 12 + quote4.length },
  });

  const t0 = 1764097200000;
  const frozenReaderCss = ".book { font-family: Georgia, 'Noto Serif SC', serif; line-height: 1.78; }";

  return {
    editions: {
      [DEMO_PRIMARY_BOOK_ID]: {
        contentVersion: DEMO_PRIMARY_BOOK_ID,
        title: "潮汐与纸页：数字阅读中的对话形态",
        authors: ["林澄", "Arthur Vance"],
      },
      [makeHex64("b202")]: {
        contentVersion: makeHex64("b202"),
        title: "The Elements of Typographic Rhythm",
        authors: ["Eleanor Marsh"],
      },
    },
    conversations: [
      {
        id: conv1,
        editionId: DEMO_PRIMARY_BOOK_ID,
        messages: [
          {
            id: root1,
            revisionId: makeHex32("9101"),
            kind: "text",
            text: "把批注从孤立表单解放出来，才真正有“与书同行”的感觉。这里说的“两种时间”很打动人：作者写下的时间与读者重访的时间。",
            contentJson: makeRichDoc([
              "把批注从孤立表单解放出来，才真正有“与书同行”的感觉。这里说的“两种时间”很打动人：作者写下的时间与读者重访的时间。",
            ]),
            replyToMessageId: null,
            referenceIds: [],
            referencePreviews: [],
            source: {
              id: src1,
              originalLocator: locator1,
              canonicalLocator: locator1,
              section: "ch1",
              selectedText: quote1,
              prefixText: "CHAPTER I · MARGINS & TIDES\n第一章 · ",
              suffixText: "\n在漫长的写本与印本历史中",
              contentHash: makeHex64("6101"),
            },
            deleted: false,
            createdAt: t0,
            updatedAt: t0,
          },
          {
            id: reply1,
            revisionId: makeHex32("9102"),
            kind: "text",
            text: "过了一周再重读这一段，依然觉得开篇比喻精准。中世纪写本的朱砂边注，本质上就是早期的异步讨论线程。",
            contentJson: makeRichDoc([
              "过了一周再重读这一段，依然觉得开篇比喻精准。中世纪写本的朱砂边注，本质上就是早期的异步讨论线程。",
            ]),
            replyToMessageId: root1,
            referenceIds: [],
            referencePreviews: [],
            source: null,
            deleted: false,
            createdAt: t0 + 3600_000,
            updatedAt: t0 + 3600_000,
          },
          {
            id: reply2,
            revisionId: makeHex32("9103"),
            kind: "text",
            text: "可以和后面第二章“锚点与快照”的段落对照着看：时间差带来语境变化，而快照保留了当年的光线与排版。",
            contentJson: makeRichDoc([
              "可以和后面第二章“锚点与快照”的段落对照着看：时间差带来语境变化，而快照保留了当年的光线与排版。",
            ]),
            replyToMessageId: reply1,
            referenceIds: [root3],
            referencePreviews: [
              {
                id: root3,
                text: "正因为快照与定位解耦，重导不同版本书籍时老批注才不会全部坍塌。",
                deleted: false,
              },
            ],
            source: null,
            deleted: false,
            createdAt: t0 + 7200_000,
            updatedAt: t0 + 7200_000,
          },
        ],
      },
      {
        id: conv2,
        editionId: DEMO_PRIMARY_BOOK_ID,
        messages: [
          {
            id: root2,
            revisionId: makeHex32("9201"),
            kind: "source-only",
            text: "",
            contentJson: "",
            replyToMessageId: null,
            referenceIds: [],
            referencePreviews: [],
            source: {
              id: src2,
              originalLocator: locator2,
              canonicalLocator: locator2,
              section: "ch1",
              selectedText: quote2,
              prefixText: "more than blank paper surrounding a column of type. ",
              suffixText: "\n设想一种阅读现场：",
              contentHash: makeHex64("6201"),
            },
            deleted: false,
            createdAt: t0 + 1800_000,
            updatedAt: t0 + 1800_000,
          },
        ],
      },
      {
        id: conv3,
        editionId: DEMO_PRIMARY_BOOK_ID,
        messages: [
          {
            id: root3,
            revisionId: makeHex32("9301"),
            kind: "text",
            text: "正因为快照与定位解耦，重导不同版本书籍时老批注才不会全部坍塌。",
            contentJson: makeRichDoc([
              "正因为快照与定位解耦，重导不同版本书籍时老批注才不会全部坍塌。",
            ]),
            replyToMessageId: null,
            referenceIds: [],
            referencePreviews: [],
            source: {
              id: src3,
              originalLocator: locator3,
              canonicalLocator: locator3,
              section: "ch2",
              selectedText: quote3,
              prefixText: "帮助读者一键跳回今天的正文上下文；",
              suffixText: "\n下面是一个简明对比表",
              contentHash: makeHex64("6301"),
            },
            deleted: false,
            createdAt: t0 + 5400_000,
            updatedAt: t0 + 5400_000,
          },
          {
            id: reply3,
            revisionId: makeHex32("9302"),
            kind: "text",
            text: "在独立影子文档中渲染快照，也能防止旧书里的恶意 CSS 污染主阅读界面。",
            contentJson: makeRichDoc([
              "在独立影子文档中渲染快照，也能防止旧书里的恶意 CSS 污染主阅读界面。",
            ]),
            replyToMessageId: root3,
            referenceIds: [],
            referencePreviews: [],
            source: null,
            deleted: false,
            createdAt: t0 + 9000_000,
            updatedAt: t0 + 9000_000,
          },
        ],
      },
      {
        id: conv4,
        editionId: makeHex64("b202"),
        messages: [
          {
            id: root4,
            revisionId: makeHex32("9401"),
            kind: "text",
            text: "“款待读者的注意力”——这正是无框线排版的最高准则：减少一切界面的自我炫耀。",
            contentJson: makeRichDoc([
              "“款待读者的注意力”——这正是无框线排版的最高准则：减少一切界面的自我炫耀。",
            ]),
            replyToMessageId: null,
            referenceIds: [],
            referencePreviews: [],
            source: {
              id: src4,
              originalLocator: locator4,
              canonicalLocator: locator4,
              section: "ch1",
              selectedText: quote4,
              prefixText: "Prologue: ",
              suffixText: " When lines breathe...",
              contentHash: makeHex64("6401"),
            },
            deleted: false,
            createdAt: t0 - 86400_000,
            updatedAt: t0 - 86400_000,
          },
          {
            id: reply4,
            revisionId: makeHex32("9402"),
            kind: "text",
            text: "和林澄在《潮汐与纸页》里谈的“安静的页边距”呼应得很好。",
            contentJson: makeRichDoc([
              "和林澄在《潮汐与纸页》里谈的“安静的页边距”呼应得很好。",
            ]),
            replyToMessageId: root4,
            referenceIds: [],
            referencePreviews: [],
            source: null,
            deleted: false,
            createdAt: t0 - 80000_000,
            updatedAt: t0 - 80000_000,
          },
        ],
      },
    ],
    revisions: {
      [root1]: [
        {
          id: makeHex32("9100"),
          kind: "text",
          text: "把批注从孤立表单解放出来，才有“与书对话”的感觉。",
          contentJson: makeRichDoc(["把批注从孤立表单解放出来，才有“与书对话”的感觉。"]),
          createdAt: t0 - 600_000,
        },
        {
          id: makeHex32("9101"),
          kind: "text",
          text: "把批注从孤立表单解放出来，才真正有“与书同行”的感觉。这里说的“两种时间”很打动人：作者写下的时间与读者重访的时间。",
          contentJson: makeRichDoc([
            "把批注从孤立表单解放出来，才真正有“与书同行”的感觉。这里说的“两种时间”很打动人：作者写下的时间与读者重访的时间。",
          ]),
          createdAt: t0,
        },
      ],
    },
    captures: {
      [root1]: [
        {
          source: {
            id: src1,
            originalLocator: locator1,
            canonicalLocator: locator1,
            section: "ch1",
            selectedText: quote1,
            prefixText: "CHAPTER I · MARGINS & TIDES\n第一章 · ",
            suffixText: "\n在漫长的写本与印本历史中",
            contentHash: makeHex64("6101"),
          },
          snapshot: {
            fragmentHtml: `<blockquote class="epigraph"><p>“${quote1}”</p></blockquote>`,
            readerCss: frozenReaderCss,
            bookCss: ".epigraph { font-style: italic; opacity: 0.9; }",
            userCss: "",
            presentationJson: JSON.stringify({
              schema: 1,
              theme: "paper",
              brightness: 100,
              fontSize: 19,
              fontFamily: "book",
              density: "standard",
            }),
            resources: [],
          },
          current: true,
          createdAt: t0,
        },
      ],
      [root3]: [
        {
          source: {
            id: src3,
            originalLocator: locator3,
            canonicalLocator: locator3,
            section: "ch2",
            selectedText: quote3,
            prefixText: "帮助读者一键跳回今天的正文上下文；",
            suffixText: "\n下面是一个简明对比表",
            contentHash: makeHex64("6301"),
          },
          snapshot: {
            fragmentHtml: `<blockquote><p>“${quote3}”</p></blockquote>`,
            readerCss: frozenReaderCss,
            bookCss: "",
            userCss: "",
            presentationJson: JSON.stringify({
              schema: 1,
              theme: "light",
              brightness: 100,
              fontSize: 19,
              fontFamily: "book",
              density: "standard",
            }),
            resources: [],
          },
          current: true,
          createdAt: t0 + 5400_000,
        },
      ],
      [root4]: [
        {
          source: {
            id: src4,
            originalLocator: locator4,
            canonicalLocator: locator4,
            section: "ch1",
            selectedText: quote4,
            prefixText: "Prologue: ",
            suffixText: " When lines breathe...",
            contentHash: makeHex64("6401"),
          },
          snapshot: {
            fragmentHtml: `<p>${quote4}</p>`,
            readerCss: frozenReaderCss,
            bookCss: "",
            userCss: "",
            presentationJson: JSON.stringify({
              schema: 1,
              theme: "paper",
              brightness: 100,
              fontSize: 20,
              fontFamily: "serif",
              density: "comfortable",
            }),
            resources: [],
          },
          current: true,
          createdAt: t0 - 86400_000,
        },
      ],
    },
  };
}

export function seedDemoLocalStorageIfEmpty(): void {
  if (typeof window === "undefined") return;
  const marker = "atha.demo.seeded.v1";
  if (window.localStorage.getItem(marker)) return;
  window.localStorage.setItem(marker, "1");

  const now = new Date();
  const formatDate = (date: Date) => {
    const y = date.getFullYear();
    const m = String(date.getMonth() + 1).padStart(2, "0");
    const d = String(date.getDate()).padStart(2, "0");
    return `${y}-${m}-${d}`;
  };
  const todayStr = formatDate(now);
  const yesterday = new Date(now.getTime() - 86_400_000);
  const yesterdayStr = formatDate(yesterday);
  const twoDaysAgo = new Date(now.getTime() - 2 * 86_400_000);
  const twoDaysAgoStr = formatDate(twoDaysAgo);

  const recentSeeds: { id: string; section: string; offset: number; percent: number; completed: boolean; durationMs: number; date: string; at: number }[] = [
    { id: DEMO_PRIMARY_BOOK_ID, section: "ch1", offset: 0, percent: 26, completed: false, durationMs: 2_280_000, date: todayStr, at: now.getTime() - 1_200_000 },
    { id: makeHex64("b202"), section: "ch1", offset: 48, percent: 64, completed: false, durationMs: 4_120_000, date: todayStr, at: now.getTime() - 7_200_000 },
    { id: makeHex64("d404"), section: "ch1", offset: 120, percent: 42, completed: false, durationMs: 1_950_000, date: yesterdayStr, at: yesterday.getTime() },
    { id: makeHex64("0707"), section: "ch1", offset: 310, percent: 100, completed: true, durationMs: 5_400_000, date: twoDaysAgoStr, at: twoDaysAgo.getTime() },
    { id: makeHex64("3a10"), section: "ch1", offset: 80, percent: 18, completed: false, durationMs: 1_140_000, date: twoDaysAgoStr, at: twoDaysAgo.getTime() - 3_600_000 },
  ];

  for (const seed of recentSeeds) {
    const locator = JSON.stringify({
      schema: 1,
      contentVersion: seed.id,
      start: { section: seed.section, offset: seed.offset },
    });
    const pKey = `atha.reader.progress.${seed.id.slice(0, 16)}.v1`;
    if (!window.localStorage.getItem(pKey)) {
      window.localStorage.setItem(
        pKey,
        JSON.stringify({ schema: 1, contentVersion: seed.id, locator }),
      );
    }
    const rKey = `atha.reader.reading.${seed.id}.v1`;
    if (!window.localStorage.getItem(rKey)) {
      window.localStorage.setItem(
        rKey,
        JSON.stringify({
          schema: 1,
          locator,
          percent: seed.percent,
          completed: seed.completed,
          lastReadAt: seed.at,
          lastReadDate: seed.date,
        }),
      );
    }
  }

  const statsKey = "atha.reader.statistics.v1";
  if (!window.localStorage.getItem(statsKey)) {
    window.localStorage.setItem(
      statsKey,
      JSON.stringify({
        schema: 1,
        days: [
          { date: twoDaysAgoStr, durationMs: 2_400_000 },
          { date: yesterdayStr, durationMs: 1_950_000 },
          { date: todayStr, durationMs: 1_680_000 },
        ],
        books: recentSeeds.map((seed) => ({
          contentVersion: seed.id,
          durationMs: seed.durationMs,
          lastReadDate: seed.date,
        })),
      }),
    );
  }
}
