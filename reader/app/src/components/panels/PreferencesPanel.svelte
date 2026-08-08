<script lang="ts">
  import {
    ArrowLeft,
    ArrowDown,
    ArrowUp,
    BookOpenText,
    Braces,
    CheckCheck,
    ChevronRight,
    CircleOff,
    Columns3,
    Copy,
    FileJson,
    Hand,
    Palette,
    Plus,
    Save,
    Search,
    SunMedium,
    Trash2,
    Type,
  } from "@lucide/svelte";

  import CssEditor from "../CssEditor.svelte";
</script>

<div class="tool-panel preferences-panel" data-settings-root data-settings-view="menu">
  <section class="settings-view settings-menu" data-settings-page="menu">
    <h2 tabindex="-1">阅读设置</h2>
    <div class="settings-list">
      <button type="button" data-settings-target="font">
        <Type aria-hidden="true" />
        <span>字体</span>
        <ChevronRight aria-hidden="true" />
      </button>
      <button type="button" data-settings-target="layout">
        <Columns3 aria-hidden="true" />
        <span>布局</span>
        <ChevronRight aria-hidden="true" />
      </button>
      <button type="button" data-settings-target="theme">
        <Palette aria-hidden="true" />
        <span>主题与亮度</span>
        <ChevronRight aria-hidden="true" />
      </button>
      <button type="button" data-settings-target="behavior">
        <Hand aria-hidden="true" />
        <span>阅读行为</span>
        <ChevronRight aria-hidden="true" />
      </button>
      <button type="button" data-settings-target="book">
        <BookOpenText aria-hidden="true" />
        <span>本书样式</span>
        <ChevronRight aria-hidden="true" />
      </button>
      <button type="button" data-settings-target="modules">
        <Braces aria-hidden="true" />
        <span>CSS 模块</span>
        <ChevronRight aria-hidden="true" />
      </button>
    </div>
    <button id="reset-application-preferences" class="quiet-button" type="button">
      恢复应用默认
    </button>
  </section>

  <section class="settings-view" data-settings-page="font" hidden>
    <header class="panel-heading">
      <button
        class="icon-button panel-back"
        type="button"
        data-settings-back
        aria-label="返回设置"
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 tabindex="-1">字体</h2>
    </header>
    <div class="setting-group">
      <div class="setting-row">
        <span class="setting-label">字号</span>
        <div class="segmented-control" role="radiogroup" aria-label="正文字号">
          <button type="button" role="radio" aria-checked="false" data-preference-for="font-size" data-preference-value="24"><span class="font-choice-small">A</span></button>
          <button type="button" role="radio" aria-checked="true" data-preference-for="font-size" data-preference-value="32">A</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="font-size" data-preference-value="40"><span class="font-choice-large">A</span></button>
        </div>
      </div>
      <select id="font-size" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="24">24px</option>
        <option value="32" selected>32px</option>
        <option value="40">40px</option>
      </select>
      <div class="setting-row setting-row-stacked">
        <span class="setting-label">字体</span>
        <div class="segmented-control segmented-control-wide" role="radiogroup" aria-label="正文字体">
          <button type="button" role="radio" aria-checked="true" data-preference-for="font-family" data-preference-value="book">书源</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="font-family" data-preference-value="serif">衬线</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="font-family" data-preference-value="sans">无衬线</button>
        </div>
      </div>
      <select id="font-family" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="book" selected>书源字体</option>
        <option value="serif">衬线</option>
        <option value="sans">无衬线</option>
      </select>
    </div>
  </section>

  <section class="settings-view" data-settings-page="layout" hidden>
    <header class="panel-heading">
      <button
        class="icon-button panel-back"
        type="button"
        data-settings-back
        aria-label="返回设置"
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 tabindex="-1">布局</h2>
    </header>
    <div class="setting-group">
      <div class="setting-row setting-row-stacked">
        <span class="setting-label">行距</span>
        <div class="segmented-control segmented-control-wide" role="radiogroup" aria-label="行距">
          <button type="button" role="radio" aria-checked="false" data-preference-for="density" data-preference-value="compact">紧凑</button>
          <button type="button" role="radio" aria-checked="true" data-preference-for="density" data-preference-value="standard">标准</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="density" data-preference-value="comfortable">舒展</button>
        </div>
      </div>
      <select id="density" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="compact">紧凑</option>
        <option value="standard" selected>标准</option>
        <option value="comfortable">舒展</option>
      </select>
      <div class="setting-row setting-row-stacked">
        <span class="setting-label">段首缩进</span>
        <div class="segmented-control segmented-control-wide" role="radiogroup" aria-label="段首缩进">
          <button type="button" role="radio" aria-checked="true" data-preference-for="paragraph-indent" data-preference-value="book">跟随书源</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="paragraph-indent" data-preference-value="none">顶格</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="paragraph-indent" data-preference-value="two">缩进两字</button>
        </div>
      </div>
      <select id="paragraph-indent" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="book" selected>跟随书源</option>
        <option value="none">不缩进</option>
        <option value="two">缩进两字</option>
      </select>
      <div class="setting-row setting-row-stacked">
        <span class="setting-label">段落间距</span>
        <div class="segmented-control segmented-control-wide" role="radiogroup" aria-label="段落间距">
          <button type="button" role="radio" aria-checked="true" data-preference-for="paragraph-spacing" data-preference-value="book">跟随书源</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="paragraph-spacing" data-preference-value="compact">紧凑</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="paragraph-spacing" data-preference-value="comfortable">舒展</button>
        </div>
      </div>
      <select id="paragraph-spacing" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="book" selected>跟随书源</option>
        <option value="compact">紧凑</option>
        <option value="comfortable">舒展</option>
      </select>
      <div class="setting-row setting-row-stacked">
        <span class="setting-label">左右边距</span>
        <div class="segmented-control segmented-control-wide" role="radiogroup" aria-label="左右边距">
          <button type="button" role="radio" aria-checked="false" data-preference-for="page-margin" data-preference-value="narrow">窄</button>
          <button type="button" role="radio" aria-checked="true" data-preference-for="page-margin" data-preference-value="standard">标准</button>
          <button type="button" role="radio" aria-checked="false" data-preference-for="page-margin" data-preference-value="wide">宽</button>
        </div>
      </div>
      <select id="page-margin" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="narrow">窄</option>
        <option value="standard" selected>标准</option>
        <option value="wide">宽</option>
      </select>
    </div>
  </section>

  <section class="settings-view" data-settings-page="theme" hidden>
    <header class="panel-heading">
      <button
        class="icon-button panel-back"
        type="button"
        data-settings-back
        aria-label="返回设置"
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 tabindex="-1">主题与亮度</h2>
    </header>
    <div class="setting-group">
      <label class="setting-row brightness-row">
        <SunMedium aria-hidden="true" />
        <input id="brightness" type="range" min="70" max="120" value="100" aria-label="亮度" />
        <SunMedium class="brightness-high" aria-hidden="true" />
      </label>
      <div class="theme-swatches" role="radiogroup" aria-label="主题">
        <button type="button" role="radio" aria-checked="true" data-preference-for="theme" data-preference-value="system" data-swatch="system"><span class="theme-preview">Aa</span><span>跟随系统</span></button>
        <button type="button" role="radio" aria-checked="false" data-preference-for="theme" data-preference-value="light" data-swatch="light"><span class="theme-preview">Aa</span><span>浅色</span></button>
        <button type="button" role="radio" aria-checked="false" data-preference-for="theme" data-preference-value="paper" data-swatch="paper"><span class="theme-preview">Aa</span><span>纸张</span></button>
        <button type="button" role="radio" aria-checked="false" data-preference-for="theme" data-preference-value="dark" data-swatch="dark"><span class="theme-preview">Aa</span><span>深色</span></button>
      </div>
      <select id="theme" class="preference-source-select" aria-hidden="true" tabindex="-1">
        <option value="system" selected>跟随系统</option>
        <option value="light">浅色</option>
        <option value="paper">纸张</option>
        <option value="dark">深色</option>
      </select>
    </div>
  </section>

  <section class="settings-view" data-settings-page="behavior" hidden>
    <header class="panel-heading">
      <button
        class="icon-button panel-back"
        type="button"
        data-settings-back
        aria-label="返回设置"
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 tabindex="-1">阅读行为</h2>
    </header>
    <label class="setting-row switch-row">
      <span class="setting-label">点击页面两侧翻页</span>
      <input id="tap-to-paginate" type="checkbox" checked />
      <span class="switch-track" aria-hidden="true"></span>
    </label>
    <label class="setting-row switch-row">
      <span class="setting-label">左右滑动翻页</span>
      <input id="swipe-to-paginate" type="checkbox" checked />
      <span class="switch-track" aria-hidden="true"></span>
    </label>
  </section>

  <section class="settings-view" data-settings-page="book" hidden>
    <header class="panel-heading">
      <button
        class="icon-button panel-back"
        type="button"
        data-settings-back
        aria-label="返回设置"
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 tabindex="-1">本书样式</h2>
    </header>
    <label class="setting-row switch-row">
      <span class="setting-label">使用书源样式</span>
      <input id="source-styles" type="checkbox" checked />
      <span class="switch-track" aria-hidden="true"></span>
    </label>
    <button id="reset-book-preferences" class="quiet-button" type="button">恢复本书样式</button>
  </section>

  <section class="settings-view module-settings" data-settings-page="modules" hidden>
    <header class="panel-heading module-heading">
      <button
        class="icon-button panel-back"
        type="button"
        data-settings-back
        aria-label="返回设置"
      >
        <ArrowLeft aria-hidden="true" />
      </button>
      <h2 tabindex="-1">CSS 模块</h2>
      <div class="panel-heading-actions">
        <button id="add-style-module" class="icon-button" type="button" aria-label="新增模块" title="新增模块"><Plus aria-hidden="true" /></button>
        <button id="open-style-module-transfer" class="icon-button" type="button" aria-label="导入与导出" title="导入与导出"><FileJson aria-hidden="true" /></button>
      </div>
    </header>
    <label class="setting-row switch-row module-master-switch">
      <span class="setting-label">启用 CSS 模块</span>
      <input id="user-styles-enabled" type="checkbox" checked />
      <span class="switch-track" aria-hidden="true"></span>
    </label>
    <div class="module-filters">
      <label class="module-search-field">
        <span class="visually-hidden">搜索模块</span>
        <Search aria-hidden="true" />
        <input id="style-module-search" type="search" maxlength="64" placeholder="搜索模块" />
      </label>
      <label>
        <span class="visually-hidden">模块分组</span>
        <select id="style-module-filter" aria-label="模块分组">
          <option value="">全部分组</option>
        </select>
      </label>
    </div>
    <select id="style-module-list" class="module-list-source" size="5" aria-hidden="true" tabindex="-1"></select>
    <div id="style-module-list-view" class="module-list-view" role="listbox" aria-label="CSS 模块"></div>
    <div class="module-batch-actions">
      <button id="enable-style-modules" class="quiet-icon-text" type="button"><CheckCheck aria-hidden="true" />启用结果</button>
      <button id="disable-style-modules" class="quiet-icon-text" type="button"><CircleOff aria-hidden="true" />停用结果</button>
    </div>
    <div class="module-fields" aria-label="当前模块">
      <label>
        <span>名称</span>
        <input id="style-module-name" maxlength="64" autocomplete="off" />
      </label>
      <label>
        <span>分组</span>
        <input id="style-module-group" maxlength="32" autocomplete="off" />
      </label>
      <label class="switch-row module-enabled-row">
        <span class="setting-label">启用当前模块</span>
        <input id="style-module-enabled" type="checkbox" />
        <span class="switch-track" aria-hidden="true"></span>
      </label>
    </div>
    <div class="module-editor-heading">
      <span class="setting-label">样式代码</span>
      <div class="module-editor-actions">
        <button id="save-style-module" class="icon-button" type="button" aria-label="保存模块" title="保存模块"><Save aria-hidden="true" /></button>
        <button id="move-style-module-up" class="icon-button" type="button" aria-label="上移模块" title="上移模块"><ArrowUp aria-hidden="true" /></button>
        <button id="move-style-module-down" class="icon-button" type="button" aria-label="下移模块" title="下移模块"><ArrowDown aria-hidden="true" /></button>
        <button id="delete-style-module" class="icon-button danger-button" type="button" aria-label="删除模块" title="删除模块"><Trash2 aria-hidden="true" /></button>
      </div>
    </div>
    <CssEditor />
  </section>
  <output id="preferences-status" aria-live="polite"></output>

  <dialog id="style-module-transfer-dialog" class="module-transfer-dialog">
    <form method="dialog">
      <h2>模块 JSON</h2>
      <textarea id="style-module-transfer" rows="12" maxlength="131072" spellcheck="false"></textarea>
      <div class="module-transfer-actions">
        <button id="import-style-modules" type="button">导入</button>
        <button id="copy-style-modules" type="button"><Copy aria-hidden="true" />复制导出</button>
        <button type="submit">关闭</button>
      </div>
    </form>
  </dialog>
</div>
