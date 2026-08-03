<dialog id="content-dialog" aria-labelledby="content-dialog-title">
  <h2 id="content-dialog-title">内容预览</h2>
  <p id="content-dialog-body"></p>
  <img id="content-dialog-image" hidden alt="" />
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div id="content-dialog-table" role="region" tabindex="0" aria-label="表格预览" hidden></div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre id="content-dialog-code" role="region" tabindex="0" aria-label="代码预览" hidden></pre>
  <form method="dialog">
    <button id="close-content-dialog" value="close">关闭</button>
  </form>
</dialog>
