<div class="tool-panel preferences-panel">
  <h2>阅读设置</h2>
  <label>
    亮度
    <input id="brightness" type="range" min="70" max="120" value="100" />
  </label>
  <label>
    主题
    <select id="theme">
      <option value="system" selected>跟随系统</option>
      <option value="light">亮色</option>
      <option value="dark">暗色</option>
    </select>
  </label>
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
  <label>
    行距
    <select id="density">
      <option value="compact">紧凑</option>
      <option value="standard" selected>标准</option>
      <option value="comfortable">舒展</option>
    </select>
  </label>
  <details class="advanced-preferences">
    <summary>页面布局</summary>
    <label>
      上边距（设备 px）
      <input id="margin-top" type="number" min="0" max="288" step="8" />
    </label>
    <label>
      右边距（设备 px）
      <input id="margin-right" type="number" min="0" max="288" step="8" />
    </label>
    <label>
      下边距（设备 px）
      <input id="margin-bottom" type="number" min="0" max="288" step="8" />
    </label>
    <label>
      左边距（设备 px）
      <input id="margin-left" type="number" min="0" max="288" step="8" />
    </label>
  </details>
  <button id="reset-application-preferences" type="button">恢复应用默认</button>
  <hr />
  <label class="checkbox-label">
    <input id="source-styles" type="checkbox" checked />
    使用书源样式
  </label>
  <details class="advanced-preferences">
    <summary>本书自定义样式</summary>
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
  </details>
  <output id="preferences-status" aria-live="polite"></output>
</div>
