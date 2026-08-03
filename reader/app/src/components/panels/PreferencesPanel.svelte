<script lang="ts">
  import {
    ArrowLeft,
    BookOpenText,
    ChevronRight,
    Columns3,
    Hand,
    Palette,
    Type,
  } from "@lucide/svelte";
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
    <label>
      字号
      <select id="font-size" aria-label="正文字号">
        <option value="24">24px</option>
        <option value="32" selected>32px</option>
        <option value="40">40px</option>
      </select>
    </label>
    <label>
      字体
      <select id="font-family">
        <option value="book" selected>书源字体</option>
        <option value="serif">衬线</option>
        <option value="sans">无衬线</option>
      </select>
    </label>
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
    <label>
      行距
      <select id="density">
        <option value="compact">紧凑</option>
        <option value="standard" selected>标准</option>
        <option value="comfortable">舒展</option>
      </select>
    </label>
    <label>
      上边距（设备 px）
      <input id="margin-top" type="number" min="144" max="288" step="8" />
    </label>
    <label>
      右边距（设备 px）
      <input id="margin-right" type="number" min="0" max="288" step="8" />
    </label>
    <label>
      下边距（设备 px）
      <input id="margin-bottom" type="number" min="144" max="288" step="8" />
    </label>
    <label>
      左边距（设备 px）
      <input id="margin-left" type="number" min="0" max="288" step="8" />
    </label>
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
    <label>
      亮度
      <input id="brightness" type="range" min="70" max="120" value="100" />
    </label>
    <label>
      主题
      <select id="theme">
        <option value="system" selected>跟随系统</option>
        <option value="light">浅色</option>
        <option value="paper">纸张</option>
        <option value="dark">深色</option>
      </select>
    </label>
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
    <label class="checkbox-label">
      <input id="tap-to-paginate" type="checkbox" checked />
      点击页面两侧翻页
    </label>
    <label class="checkbox-label">
      <input id="swipe-to-paginate" type="checkbox" checked />
      左右滑动翻页
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
    <label class="checkbox-label">
      <input id="source-styles" type="checkbox" checked />
      使用书源样式
    </label>
    <label class="checkbox-label">
      <input id="user-styles-enabled" type="checkbox" checked />
      启用自定义样式
    </label>
    <label for="user-stylesheet">CSS</label>
    <textarea
      id="user-stylesheet"
      rows="6"
      spellcheck="false"
      placeholder={"例如：p { letter-spacing: 1px; }"}
    ></textarea>
    <div class="preference-actions">
      <button id="apply-user-style" type="button">应用</button>
      <button id="reset-book-preferences" type="button">恢复本书</button>
    </div>
  </section>
  <output id="preferences-status" aria-live="polite"></output>
</div>
