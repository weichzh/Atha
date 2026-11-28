#!/usr/bin/env bash
# Current Svelte UI checks. The kernel diagnostic document is never used here.
# shellcheck disable=SC2016

click_label() {
  local selector=$1 label=$2 element
  element=$(execute_sync '
const [selector, label] = arguments;
const button = [...document.querySelectorAll(selector)].find((it) =>
  it.textContent.trim() === label && it.getClientRects().length);
return button || null;' "$(jq -cn --arg selector "$selector" --arg label "$label" '[$selector,$label]')" |
    jq -er '.["element-6066-11e4-a52e-4f735466cecf"]') || die "control not found: $label"
  webdriver_click_element_ref "$element" || die "could not click: $label"
}

set_text_input() {
  local selector=$1 value=$2
  [[ $(execute_sync '
const input = document.querySelector(arguments[0]);
if (!input) return false;
input.value = arguments[1];
input.dispatchEvent(new Event("input", {bubbles: true}));
return true;' "$(jq -cn --arg selector "$selector" --arg value "$value" '[$selector,$value]')") == true ]] ||
    die "could not fill $selector"
}

choose_shelf_sort() {
  click_label '[aria-label="书架排序"] .sort-chip' "$1"
  [[ $(execute_sync '
return [...document.querySelectorAll("[aria-label=书架排序] .sort-chip[aria-pressed=true]")]
  .some(button => button.textContent.trim() === arguments[0]);' "$(jq -cn --arg label "$1" '[$label]')") == true ]] ||
    die 'requested shelf sort did not activate'
}

set_shelf_query() {
  set_text_input 'main[aria-label="书架"] input[type=search]' "$1"
}

verify_library_layout() {
  local width=$1 height=$2
  webdriver_click_element 'button[aria-label="清单列表"]' || die 'could not switch to list view'
  wait_for_script '
const rows = [...document.querySelectorAll(".book-list [data-shelf-open]")];
const valid = rows.every(row => {
  const outer = row.getBoundingClientRect();
  const thumb = row.querySelector(".list-thumb").getBoundingClientRect();
  const title = row.querySelector(".list-title").getBoundingClientRect();
  const author = row.querySelector(".list-sub").getBoundingClientRect();
  return thumb.width > 0 && thumb.left >= outer.left - 1 && thumb.right <= title.left + 1 &&
    title.right <= outer.right + 1 && author.right <= outer.right + 1 &&
    title.top <= author.top && thumb.bottom <= outer.bottom + 1;
});
const controls = [...document.querySelectorAll(".search-bar input, .bar-controls button, [aria-label=书架排序] button")];
return { width: innerWidth, count: rows.length, valid, controls: controls.length,
  clipped: controls.some(el => el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1),
  overflow: document.documentElement.scrollWidth > innerWidth };' \
    ".width >= $((width - 2)) and .width <= $((width + 2)) and .count == $expected_books and .valid and .controls >= 6 and (.clipped | not) and (.overflow | not)" \
    "catalogue list layout failed at ${width}x${height}" >/dev/null

  if ((width == 1000)); then
    set_shelf_query 'Atha FB2 Gate'
    wait_for_script '
const rows = [...document.querySelectorAll(".book-list [data-shelf-open]")];
return { titles: rows.map(row => row.title) };' \
      '.titles == ["Atha FB2 Gate"]' 'shelf search did not filter the catalogue' >/dev/null
    set_shelf_query ''
    choose_shelf_sort '阅读进度'
    wait_for_script '
const rows = [...document.querySelectorAll(".book-list [data-shelf-open]")];
const progress = rows.map(row => Number(row.querySelector(".list-percent")?.textContent.replace("%", "") ?? -1));
return { count: rows.length, sorted: progress.every((value, i) => i === 0 || value <= progress[i-1]),
  publicHasProgress: rows.some(row => row.title === "Atha FB2 Gate" && row.querySelector(".list-percent")) };' \
      ".count == $expected_books and .sorted == true and .publicHasProgress == true" \
      'reading-progress order did not reuse the complete catalogue' >/dev/null
    choose_shelf_sort '书名 A–Z'
  fi
  webdriver_click_element 'button[aria-label="封面网格"]' || die 'could not switch to grid view'
  wait_for_script '
const cards = [...document.querySelectorAll(".book-grid .book-card")].map(card => card.getBoundingClientRect());
const overlap = cards.some((left, i) => cards.slice(i + 1).some(right =>
  left.left < right.right && left.right > right.left && left.top < right.bottom && left.bottom > right.top));
return {count: cards.length, overlap, overflow: document.documentElement.scrollWidth > innerWidth};' \
    ".count == $expected_books and (.overlap | not) and (.overflow | not)" \
    "catalogue grid layout failed at ${width}x${height}" >/dev/null
}

wait_shelf_selection() {
  wait_for_script '
const actions = document.querySelector("[aria-label=选中书籍操作]");
const buttons = [...(actions?.querySelectorAll(".selection-options button") ?? [])];
const rects = buttons.map(button => button.getBoundingClientRect());
return {selected: document.querySelectorAll(".shelf-catalog [data-shelf-open][aria-pressed=true]").length,
  labels: buttons.map(button => button.textContent.trim()),
  fits: rects.every((rect, i) => rect.width > 0 && rect.height > 0 && rect.left >= 0 &&
    rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight &&
    (i === 0 || rects[i-1].right <= rect.left + 1)),
  clipped: buttons.some(button => button.scrollWidth > button.clientWidth + 1 || button.scrollHeight > button.clientHeight + 1) };' \
    '.selected == 1 and (.labels[0] == "收藏" or .labels[0] == "取消收藏") and
      .labels[1:] == ["编辑信息","换封面","移出","删除"] and .fits and (.clipped | not)' \
    'single-book selection actions did not fit' >/dev/null
}

select_shelf_book() {
  local selector=$1 key=${2:-context}
  [[ $(execute_sync '
const button = document.querySelector(arguments[0]);
if (!button) return false;
const event = arguments[1] === "keyboard" ?
  new KeyboardEvent("keydown", {key: "F10", shiftKey: true, bubbles: true, cancelable: true}) :
  new MouseEvent("contextmenu", {button: 2, bubbles: true, cancelable: true});
button.focus();
return !button.dispatchEvent(event);' "$(jq -cn --arg selector "$selector" --arg key "$key" '[$selector,$key]')") == true ]] ||
    die 'book selection did not handle the context action'
  wait_shelf_selection
}

long_press_shelf_book() {
  [[ $(execute_async '
const done = arguments[arguments.length - 1];
const button = document.querySelector(arguments[0]);
if (!button) return done(false);
const rect = button.getBoundingClientRect();
const event = type => new PointerEvent(type, {bubbles: true, cancelable: true, pointerId: 91,
  pointerType: "touch", isPrimary: true, button: 0, clientX: rect.left + 20, clientY: rect.top + 20});
button.dispatchEvent(event("pointerdown"));
setTimeout(() => { button.dispatchEvent(event("pointerup")); done(true); }, 550);
' "$(jq -cn --arg selector "$1" '[$selector]')") == true ]] || die 'could not long-press a catalogue book'
  wait_shelf_selection
}

reader_ready() {
  wait_for_script '
return {ready: document.querySelector(".reader-root")?.dataset.screenReady === "true",
  busy: document.querySelector(".reader-shell")?.getAttribute("aria-busy"),
  blocked: Boolean(document.querySelector(".opening-back, .opening-veil, .error-overlay"))};' \
    '.ready == true and .busy == "false" and .blocked == false' 'current reader did not settle' >/dev/null
}

webdriver_key() {
  local key=$1 payload
  payload=$(jq -cn --arg key "$key" '{actions:[{type:"key",id:"atha-keyboard",actions:[
    {type:"keyDown",value:$key},{type:"keyUp",value:$key}]}]}')
  webdriver_value POST "/session/$session_id/actions" "$payload" >/dev/null
}

reader_chrome() {
  [[ $(execute_sync 'return Boolean(document.querySelector(".reader-controls"));') == true ]] && return
  local point
  point=$(execute_sync 'const r=document.querySelector(".reader").getBoundingClientRect(); return {x:r.left+r.width/2,y:r.top+r.height/2};')
  perform_pointer_action "$point" tap || die 'could not reveal reader chrome'
  wait_for_script 'return { chrome: Boolean(document.querySelector("#reader-back")) };' \
    '.chrome == true' 'reader chrome did not open' >/dev/null
}

reader_back() {
  reader_chrome
  webdriver_click_element '#reader-back' || die 'could not return with the reader back control'
  current_book_kind=
  diagnostics_enabled=0
}

verify_continuous_shelf() {
  local fixture=$run_root/ShelfGateRoundTrip.txt fixture_id fixture_source card state
  printf '%s\n' 'Atha native shelf fixture body marker.' >"$fixture"
  printf '%s\n' "$fixture" 'ShelfGateRoundTrip' 'Atha native shelf fixture body marker.' \
    '原生验收标签' '原生验收主题' '原生验收系列' >>"$privacy_file"
  fire_tauri_drag_event 'tauri://drag-drop' "$fixture" >/dev/null || die 'could not import round-trip fixture'
  card='.shelf-catalog [data-shelf-open][title="ShelfGateRoundTrip"]'
  state=$(wait_for_script '
const cards = [...document.querySelectorAll(".shelf-catalog [data-shelf-open]")];
return {count: cards.length, id: cards.find(card => card.title === "ShelfGateRoundTrip")?.dataset.shelfOpen};' \
    ".count == $((expected_books + 1)) and (.id | test(\"^[a-f0-9]{64}$\"))" 'round-trip fixture did not import')
  fixture_id=$(jq -er .id <<<"$state")
  printf '%s\n' "$fixture_id" >>"$privacy_file"
  fixture_source=$(jq -er '.sourcePath | select(test("^[a-f0-9]{64}\\.txt$"))' "$library_root/Library/$fixture_id.json") ||
    die 'round-trip fixture has no staged source'

  select_shelf_book "$card"
  click_label '.selection-options button' '收藏'
  wait_for_script '
return {favorite: Boolean(document.querySelector(".shelf-catalog [title=ShelfGateRoundTrip] [aria-label=已收藏]")),
  selecting: Boolean(document.querySelector(".shelf-actions"))};' \
    '.favorite and (.selecting | not)' 'favorite did not save and exit selection' >/dev/null
  select_shelf_book "$card" keyboard
  click_label '.selection-options button' '编辑信息'
  wait_for_script 'return {open: Boolean(document.querySelector(".edit-dialog"))};' '.open' 'classification dialog did not open' >/dev/null
  set_text_input '.edit-form label:nth-of-type(1) textarea' '原生验收标签'
  set_text_input '.edit-form label:nth-of-type(2) textarea' '原生验收主题'
  set_text_input '.edit-form label:nth-of-type(3) textarea' '原生验收系列'
  webdriver_click_element '.edit-form button[type=submit]' || die 'could not save fixture classification'
  wait_for_script 'return {saved: document.querySelector(".toast-banner")?.textContent === "已保存分类信息"};' \
    '.saved' 'classification did not save' >/dev/null
  open_library "$((expected_books + 1))"
  wait_for_script '
return {favorite: Boolean(document.querySelector(".shelf-catalog [title=ShelfGateRoundTrip] [aria-label=已收藏]")),
  facets: [...document.querySelectorAll(".quick-pill")].map(el => el.textContent.trim())};' \
    '.favorite and (.facets | index("原生验收标签") != null)' 'organization did not survive reload' >/dev/null
  webdriver_click_element 'button[aria-label="筛选藏书"]' || die 'could not open filters'
  webdriver_click_element '#shelf-fav-switch' || die 'could not enable favorite filter'
  click_label '.filter-popover .facet-pill span:first-child' '原生验收标签'
  webdriver_click_element 'button[aria-label="筛选藏书"]' || die 'could not dismiss filters'
  set_shelf_query 'Atha FB2 Gate'
  wait_for_script '
return {count: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length,
  conditions: document.querySelectorAll(".condition-chip").length};' \
    '.count == 0 and .conditions == 3' 'query did not honor favorite and tag' >/dev/null
  click_label '.condition-chip' '仅看收藏'
  click_label '.condition-chip' '标签：原生验收标签'
  wait_for_script '
return {titles: [...document.querySelectorAll(".shelf-catalog [data-shelf-open]")].map(el => el.title),
  query: document.querySelector("main[aria-label=书架] input[type=search]")?.value,
  conditions: document.querySelectorAll(".condition-chip").length};' \
    '.titles == ["Atha FB2 Gate"] and .query == .titles[0] and .conditions == 1' \
    'removing facet chips did not preserve the query and expand the results' >/dev/null
  set_shelf_query '原生验收系列'
  click_label '.quick-pill' '原生验收标签'
  choose_shelf_sort '书名 Z–A'
  webdriver_click_element 'button[aria-label="清单列表"]' || die 'could not switch the filtered list'
  wait_for_script '
return {titles: [...document.querySelectorAll(".book-list [data-shelf-open]")].map(el => el.title)};' \
    '.titles == ["ShelfGateRoundTrip"]' 'series search did not isolate the fixture' >/dev/null
  webdriver_click_element "$card" || die 'could not open filtered fixture'
  reader_ready
  reader_chrome
  webdriver_click_element 'button[aria-label="目录与全书搜索"]' || die 'could not open reader directory'
  click_label '.directory-dialog button' '全书搜索'
  set_text_input '.directory-dialog input[type=search], .directory-dialog input[placeholder]' 'native shelf fixture body marker'
  click_label '.directory-dialog button[type=submit]' '搜索'
  wait_for_script '
return {exact: [...document.querySelectorAll(".directory-dialog button")].some(el =>
  el.textContent.includes("Atha native shelf fixture body marker."))};' \
    '.exact' 'the reader did not find the distinct fixture text' >/dev/null
  execute_sync 'document.activeElement.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true,cancelable:true})); return true;' >/dev/null
  wait_for_script 'return {dismissed: !document.querySelector(".directory-dialog")};' '.dismissed' 'reader search did not close' >/dev/null
  reader_back
  wait_for_script '
const button = document.querySelector(".book-list [data-shelf-open][title=ShelfGateRoundTrip]");
return {query: document.querySelector("main[aria-label=书架] input[type=search]")?.value,
  tag: [...document.querySelectorAll(".condition-chip")].some(el => el.textContent.trim() === "标签：原生验收标签"),
  sort: document.querySelector("[aria-label=书架排序] .sort-chip[aria-pressed=true]")?.textContent.trim(),
  focused: Boolean(button && document.activeElement === button),
  cards: document.querySelectorAll(".book-list [data-shelf-open]").length};' \
    '.query == "原生验收系列" and .tag and .sort == "书名 Z–A" and .focused and .cards == 1' \
    'return did not preserve query, tag, list, sort and focus' >/dev/null

  local recent='button[aria-label="展开最近在读"]'
  webdriver_click_element "$recent" || die 'could not open the recent reading drawer'
  webdriver_click_element '.recent-drawer[open] .recent-book[title="ShelfGateRoundTrip"]' || die 'could not resume fixture from Recent Reading'
  reader_ready
  reader_back
  wait_for_script '
const button = document.querySelector("[data-shelf-recent-return]");
return {focused: Boolean(button && document.activeElement === button), closed: !document.querySelector(".recent-drawer[open]")};' \
    '.focused and .closed' 'return from recent reading did not restore the drawer trigger' >/dev/null

  webdriver_click_element '.clear-all-link' || die 'could not reset all shelf conditions'
  choose_shelf_sort '书名 A–Z'
  webdriver_click_element 'button[aria-label="封面网格"]' || die 'could not restore the grid'
  long_press_shelf_book "$card"
  click_label '.selection-options button' '删除'
  wait_for_script '
const dialog = document.querySelector(".confirm-dialog");
return {confirmedScope: Boolean(dialog?.textContent.includes("对话和历史快照仍保留") &&
  dialog.textContent.includes("阅读进度、书签和排版设置"))};' \
    '.confirmedScope' 'local-data delete confirmation did not describe the scope' >/dev/null
  click_label '.confirm-dialog button' '删除本地数据'
  wait_for_script '
return {count: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length,
  fixture: Boolean(document.querySelector(".shelf-catalog [title=ShelfGateRoundTrip]")),
  conditions: document.querySelectorAll(".condition-chip").length,
  deleted: document.querySelector(".toast-banner")?.textContent === "已删除 1 册"};' \
    ".count == $expected_books and (.fixture | not) and .conditions == 0 and .deleted" \
    'local deletion did not restore the original shelf' >/dev/null
  [[ ! -e $library_root/Library/$fixture_id.json && ! -e $library_root/ImportedBooks/$fixture_id &&
    ! -e $library_root/SourceBooks/$fixture_source ]] || die 'fixture files survived local deletion'
  continuous_shelf_evidence='{"savedOrganization":true,"organizationAfterReload":true,"scopedSearch":true,"expandedSearch":true,"realReaderContent":true,"returnedQueryFilterSortViewFocus":true,"recentReadingDrawerResume":true,"contextSelection":true,"keyboardSelection":true,"syntheticTouchLongPressSelection":true,"fixtureDeleted":true}'
}

nav_tab() {
  click_label '.nav-shell [role=tab]' "$1"
}

settings_section() {
  local section=$1
  if [[ $(execute_sync 'return Boolean(document.querySelector(".settings-back"));') == true ]]; then
    webdriver_click_element '.settings-back' || die 'could not return to settings index'
  fi
  webdriver_click_element ".settings-index [data-setting='$section']" || die "could not open settings section: $section"
}

verify_library_settings() {
  nav_tab '设置'
  settings_section appearance
  execute_sync 'globalThis.__readerPreferencesBeforeTheme = localStorage.getItem("atha.reader.application.v1"); return true;' >/dev/null
  click_label '.settings-shell [role=radio]' '浅色纸感'
  wait_for_script '
return {theme: document.documentElement.dataset.appTheme,
  saved: JSON.parse(localStorage.getItem("atha.app.appearance.v1") || "null")?.theme,
  unchanged: localStorage.getItem("atha.reader.application.v1") === globalThis.__readerPreferencesBeforeTheme,
  modes: document.querySelectorAll(".settings-shell [role=radio]").length,
  overflow: document.documentElement.scrollWidth > innerWidth};' \
    '.theme == "light" and .saved == "light" and .unchanged and .modes == 3 and (.overflow | not)' \
    'application theme did not save independently of book appearance' >/dev/null
  capture_public_ui settings
  click_label '.settings-shell [role=radio]' '跟随系统'
  settings_section typography
  execute_sync '
globalThis.__appearanceBeforeReset = localStorage.getItem("atha.app.appearance.v1");
localStorage.setItem("atha.reader.application.v1", JSON.stringify({
  schema: 1, preferences: {theme: "paper", brightness: 90, fontSize: 24, fontFamily: "serif", density: "compact"}
}));
return true;' >/dev/null
  click_label '.settings-detail button' '恢复默认'
  click_label '.confirm-dialog button' '确认恢复'
  wait_for_script '
const preferences = JSON.parse(localStorage.getItem("atha.reader.application.v1") || "null")?.preferences;
return {defaults: JSON.stringify(preferences) === JSON.stringify({
  theme: "system", brightness: 100, fontSize: 19, fontFamily: "book", density: "standard"}),
  unchanged: localStorage.getItem("atha.app.appearance.v1") === globalThis.__appearanceBeforeReset};' \
    '.defaults and .unchanged' 'reading defaults did not reset independently' >/dev/null

  settings_section dictionary
  wait_for_script '
const scope = document.querySelector(".settings-detail");
return {scales: scope?.querySelectorAll("[role=radio]").length,
  import: Boolean([...scope?.querySelectorAll("button") ?? []].find(el => el.textContent.trim() === "导入本地词典")),
  empty: Boolean(scope?.querySelector(".dict-empty")),
  overflow: document.documentElement.scrollWidth > innerWidth};' \
    '.scales == 6 and .import and .empty and (.overflow | not)' 'dictionary controls did not match the isolated empty library' >/dev/null
  settings_section storage
  wait_for_script '
const scope = document.querySelector(".settings-detail");
return {usage: [...scope?.querySelectorAll(".legend-name") ?? []].map(el => el.textContent),
  total: scope?.querySelector(".storage-total")?.textContent,
  actions: [...scope?.querySelectorAll(".backup-actions button") ?? []].map(el => el.textContent.trim()),
  overflow: document.documentElement.scrollWidth > innerWidth};' \
    '.usage == ["藏书文件","离线词典","排版缓存","笔记与偏好"] and (.total | length > 0) and
      .actions == ["导出 .atha-data 完整备份","从备份文件恢复"] and (.overflow | not)' \
    'storage and backup entries did not match the current settings' >/dev/null
  nav_tab '书架'
  select_shelf_book '.shelf-catalog [data-shelf-open][title="Atha FB2 Gate"]'
  webdriver_click_element 'button[aria-label="退出选择"]' || die 'could not exit shelf selection'
  long_press_shelf_book '.shelf-catalog [data-shelf-open][title="Atha FB2 Gate"]'
  webdriver_click_element 'button[aria-label="退出选择"]' || die 'could not exit long-press selection'
}

verify_reading_memory() {
  # A restarted automation session may use fresh site storage. Seed the
  # public recent-reading record after setting the requested viewport.
  local statistics
  statistics=$(jq -cn --arg id "$public_id" '{
    schema: 1, days: [{date:"2026-08-13",durationMs:90000}],
    books: [{contentVersion:$id,durationMs:90000,lastReadDate:"2026-08-13"}]
  } | [tojson]')
  execute_sync 'localStorage.setItem("atha.reader.statistics.v1", arguments[0]); return true;' "$statistics" >/dev/null
  nav_tab '阅读记忆'
  wait_for_script '
return {heading: document.querySelector(".memory-shell h1")?.textContent,
  search: Boolean(document.querySelector(".memory-shell input[aria-label=搜索笔记与回复]")),
  overflow: document.documentElement.scrollWidth > innerWidth};' \
    '.heading == "阅读记忆" and .search and (.overflow | not)' \
    'cross-book search did not become ready' >/dev/null
  set_text_input '.memory-shell input[type=search]' '公共记忆'
  wait_for_script '
const rows = [...document.querySelectorAll(".hit-row")];
return {count: rows.length, jump: document.querySelectorAll(".hit-row .jump-btn").length,
  snapshots: rows.filter(row => row.querySelector(".hit-actions button")).length,
  missing: document.querySelectorAll(".missing-edition-note").length,
  overflow: document.documentElement.scrollWidth > innerWidth};' \
    '.count == 2 and .jump == 1 and .snapshots == 2 and .missing == 1 and (.overflow | not)' \
    'seeded memory did not distinguish available from missing source editions' >/dev/null
  set_text_input '.memory-shell input[type=search]' ''
  wait_for_script '
return {publicRecent: [...document.querySelectorAll(".recent-chip .chip-title")]
  .some(el => el.textContent === "Atha FB2 Gate")};' \
    '.publicRecent' 'the memory view did not show the recent public book after clearing a search' >/dev/null
  set_text_input '.memory-shell input[type=search]' '公共记忆'
  wait_for_script 'return {hits: document.querySelectorAll(".hit-row").length};' \
    '.hits == 2' 'restored memory query did not produce the two seeded hits' >/dev/null
  local snapshot_button
  snapshot_button=$(execute_sync '
const row = [...document.querySelectorAll(".hit-row")].find(el => el.querySelector(".jump-btn"));
const button = row?.querySelector(".hit-actions button:not(.jump-btn)");
button?.scrollIntoView({block:"center", inline:"nearest"});
return button || null;' |
    jq -er '.["element-6066-11e4-a52e-4f735466cecf"]') || die 'snapshot control missing'
  webdriver_click_element_ref "$snapshot_button" || die 'could not open reading memory snapshots'
  wait_for_script '
return {versions: [...document.querySelectorAll(".capture-badge")].map(el => el.textContent.trim()).sort(),
  rendered: [...document.querySelectorAll(".snapshot-dialog .snapshot-shadow-host")].filter(el =>
    el.querySelector("div") || el.shadowRoot?.querySelector(".book")).length,
  overflow: document.documentElement.scrollWidth > innerWidth};' \
    '.versions == ["历史捕获版本","当前版本快照"] and .rendered == 2 and (.overflow | not)' \
    'both current and historical snapshots did not render' >/dev/null
  webdriver_click_element '.snapshot-dialog button[aria-label="关闭快照"]' || die 'could not dismiss snapshots'
  webdriver_click_element '.hit-row .jump-btn' || die 'could not jump back to the seeded original'
  reader_ready
  current_book_kind=public
  wait_for_script '
return {edition: new URL(location.href).searchParams.get("version")?.length,
  thread: Boolean(document.querySelector(".pane-c .source-quote")), chapter: document.querySelector(".running-chapter")?.textContent.trim()};' \
    '.edition == 64 and .thread' 'memory hit did not reopen the book and its source thread' >/dev/null
  webdriver_click_element '.pane-c button[aria-label="关闭笔记工作区"]' || die 'could not close seeded thread'
  reader_back
  wait_for_script '
return {query: document.querySelector(".memory-shell input[type=search]")?.value,
  hits: document.querySelectorAll(".hit-row").length};' \
    '.query == "公共记忆" and .hits == 2' 'reader return did not preserve memory search' >/dev/null
  nav_tab '书架'
}

reader_progress() {
  execute_sync '
const key = "atha.reader.progress." + new URL(location.href).searchParams.get("state") + ".v1";
const saved = JSON.parse(localStorage.getItem(key) || "null");
return {book: saved?.contentVersion, section: JSON.parse(saved?.locator || "null")?.start?.section, locator: saved?.locator};'
}

verify_desktop_workspace() {
  local width height geometry before after dimensions='[]'
  local -a widths=(360 600 1000 1280 1600)
  for width in "${widths[@]}"; do
    height=760
    if ((width == 1280)); then height=800; elif ((width == 1600)); then height=900; fi
    set_window_size "$width" "$height"
    geometry=$(wait_for_script '
const a = document.querySelector(".region-a")?.getBoundingClientRect();
const r = document.querySelector(".reader")?.getBoundingClientRect();
const p = document.querySelector(".page")?.getBoundingClientRect();
return {width: innerWidth, height:innerHeight, availableWidth:screen.availWidth, filled: a && Math.abs(a.width - innerWidth) <= 2,
  centered: r && p && Math.abs((p.left - r.left) - (r.right - p.right)) <= 2,
  page: p?.width, margin: r && p && p.left - r.left,
  hidden: !document.querySelector(".reader-controls,.pane-b,.pane-c"), overflow: document.documentElement.scrollWidth > innerWidth};' \
      "$(viewport_width_predicate "$width") and .filled and .centered and .page > 250 and .page <= 761 and .margin >= 30 and .hidden and (.overflow | not)" \
      "focus reading did not fit at ${width}px")
    dimensions=$(jq -cn --argjson previous "$dimensions" --argjson requestedWidth "$width" --argjson requestedHeight "$height" \
      --argjson actual "$geometry" '$previous + [{requestedWidth:$requestedWidth,requestedHeight:$requestedHeight,width:$actual.width,height:$actual.height}]')
    before=$(reader_progress)
    reader_chrome
    capture_public_ui reader-main
    webdriver_click_element 'button[aria-label="目录与全书搜索"]' || die 'directory is unavailable'
    wait_for_script '
const dialog = document.querySelector(".directory-dialog");
const r = dialog?.getBoundingClientRect();
return {toc: dialog?.querySelectorAll(".toc-jump").length,
  tabs: [...dialog?.querySelectorAll(".dir-tab") ?? []].map(el => el.textContent.trim()),
  fits: r && r.left >= 0 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1};' \
      '.fits and .toc >= 3 and .tabs[0] == "目录" and .tabs[2] == "全书搜索"' \
      'directory and search entry did not fit' >/dev/null
    webdriver_click_element 'button[aria-label="关闭目录"]' || die 'could not dismiss directory'
    wait_for_script '
return {closed: !document.querySelector(".directory-dialog"),
  focused: Boolean(document.activeElement.closest(".reader-shell"))};' \
      '.closed and .focused' 'directory back did not return focus to the reading surface' >/dev/null
    webdriver_key n || die 'notes keyboard shortcut did not reach the reader'
    wait_for_script '
const b = document.querySelector(".pane-b")?.getBoundingClientRect();
const a = document.querySelector(".region-a")?.getBoundingClientRect();
return {notes: document.querySelectorAll(".note-index-row").length, fits: b && b.width > 0 && b.left >= 0 && b.right <= innerWidth + 1 && b.bottom <= innerHeight + 1,
  ordered: innerWidth < 800 || a?.right <= b?.left + 2};' \
      '.notes >= 1 and .fits and .ordered' 'A+B notes list did not fit' >/dev/null
    capture_public_ui reader-notes
    webdriver_click_element '.note-index-row' || die 'could not open a seeded conversation'
    wait_for_script '
const a = document.querySelector(".region-a");
const b = document.querySelector(".pane-b")?.getBoundingClientRect();
const c = document.querySelector(".pane-c")?.getBoundingClientRect();
const send = document.querySelector(".pane-c .send-btn")?.getBoundingClientRect();
return {fit: c && c.width > 0 && c.left >= 0 && c.right <= innerWidth + 2 && c.bottom <= innerHeight + 2,
  ordered: innerWidth < 800 ? !b : b?.right <= c?.left + 2 &&
    (innerWidth < 1440 ? a?.classList.contains("hidden-a") : a?.getBoundingClientRect().right <= b?.left + 2),
  quote: Boolean(document.querySelector(".pane-c .source-quote")),
  send: send && send.width > 0 && send.bottom <= innerHeight + 1,
  overflow: document.documentElement.scrollWidth > innerWidth};' \
      '.fit and .ordered and .quote and .send and (.overflow | not)' 'notes thread did not match the viewport workspace' >/dev/null
    capture_public_ui reader-thread
    webdriver_click_element '.pane-c button[aria-label="返回笔记列表"], .pane-c button[aria-label="收起对话线程"]' ||
      die 'thread back control is unavailable'
    wait_for_script '
return {list: Boolean(document.querySelector(".pane-b")), closed: !document.querySelector(".pane-c")};' \
      '.list and .closed' 'thread back did not restore the notes list' >/dev/null
    webdriver_click_element '.pane-b button[aria-label="导出笔记"]' || die 'notes export menu is unavailable'
    wait_for_script '
return {actions: document.querySelectorAll(".export-popover button").length};' \
      '.actions >= 1' 'notes export action did not become visible' >/dev/null
    execute_sync 'document.activeElement.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true,cancelable:true})); return true;' >/dev/null
    webdriver_click_element '.pane-b button[aria-label="关闭笔记工作区"]' || die 'could not close notes'
    wait_for_script '
return {closed: !document.querySelector(".pane-b,.pane-c")};' \
      '.closed' 'closing notes did not return to the reader' >/dev/null
    after=$(reader_progress)
    [[ $(jq -r .section <<<"$before") == "$(jq -r .section <<<"$after")" ]] ||
      die "switching tools changed the reading section at ${width}px"
  done
  desktop_workspace_evidence=$(jq -cn --argjson geometry "$geometry" --argjson dimensions "$dimensions" \
    '{widths:[360,600,1000,1280,1600],defaultMode:"focus-reading",workspaceMode:"explicit",tools:["directory","search","notes"],
      viewports:$dimensions, desktopPageWidth:$geometry.page, noteLayouts:["portrait","A+B","B+C","A+B+C"], focusVerified:true, sectionStable:true, narrowRegression:true}')
}

verify_reader_interface_navigation() {
  local blocked
  blocked=$(execute_async '
const done = arguments[arguments.length - 1];
const probe = new Image();
let violations = 0;
const onViolation = event => { if (event.effectiveDirective === "img-src") violations++; };
document.addEventListener("securitypolicyviolation", onViolation);
probe.src = "http://127.0.0.1:9/blocked.png";
document.body.append(probe);
setTimeout(() => {
  probe.remove();
  document.removeEventListener("securitypolicyviolation", onViolation);
  done({blocked: violations > 0, loaded: probe.naturalWidth > 0});
}, 150);')
  jq -e '.blocked and (.loaded | not)' <<<"$blocked" >/dev/null ||
    die 'candidate reader did not enforce its external image CSP'
  reader_chrome
  webdriver_click_element 'button[aria-label="目录与全书搜索"]' || die 'could not open reader tools'
  click_label '.dir-tabs button' '全书搜索'
  set_text_input '.directory-dialog input[type=search]' '第二节正文'
  webdriver_click_element '.directory-dialog button[type=submit]' || die 'could not submit book search'
  wait_for_script '
return {results: [...document.querySelectorAll(".directory-dialog .dir-list button")].filter(el =>
  el.textContent.includes("第二节正文")).length};' \
    '.results == 1' 'reader search did not find fixture body text' >/dev/null
  webdriver_click_element 'button[aria-label="关闭目录"]' || die 'could not dismiss book search'
  wait_for_script 'return {closed: !document.querySelector(".directory-dialog")};' \
    '.closed' 'reader search did not return to the reading screen' >/dev/null
  jq -cn '{directorySearch:true, dismissSearch:true, externalImageCsp:true}'
}

verify_reader_redesign() {
  local width height saved before after pressed chapter_one
  chapter_one=$(jq -er '.sections[1].id' "$public_manifest")
  for width in 360 1000 1600; do
    height=760
    ((width != 1600)) || height=900
    set_window_size "$width" "$height"
    reader_chrome
    wait_for_script '
const rect = selector => document.querySelector(selector)?.getBoundingClientRect();
const tops = rect(".top-island"), bottom = rect(".bottom-island"), page = rect(".page");
const targets = [...document.querySelectorAll(".top-island button,.bottom-island button")].filter(el => el.getClientRects().length);
return {title: document.querySelector(".island-book")?.textContent,
  fits: tops && bottom && page && tops.left >= 0 && tops.right <= innerWidth && bottom.right <= innerWidth &&
    tops.bottom <= page.top + 2 && bottom.top >= page.bottom - 2 && bottom.bottom <= innerHeight,
  targets: targets.length >= 5 && targets.every(el => {const r=el.getBoundingClientRect();return r.width >= 44 && r.height >= 44;})};' \
      '.title == "Atha FB2 Gate" and .fits and .targets' 'current top and bottom chrome did not fit' >/dev/null
    capture_public_ui reader-main
    pressed=$(execute_sync 'return document.querySelector(".island-right [aria-pressed]")?.getAttribute("aria-pressed");')
    webdriver_click_element '.island-right [aria-pressed]' || die 'could not toggle bookmark'
    wait_for_script 'return {pressed: document.querySelector(".island-right [aria-pressed]")?.getAttribute("aria-pressed")};' \
      ".pressed != $pressed" 'bookmark did not toggle' >/dev/null
    webdriver_click_element '.island-right [aria-pressed]' || die 'could not restore bookmark'
    wait_for_script 'return {pressed: document.querySelector(".island-right [aria-pressed]")?.getAttribute("aria-pressed")};' \
      ".pressed == $pressed" 'bookmark did not restore' >/dev/null

    if ((width == 360)); then
      # The first TOC item is the first chapter, not the FB2 preface.
      webdriver_click_element 'button[aria-label="目录与全书搜索"]' || die 'could not open the TOC'
      click_label '.toc-jump' '第一章'
      before=$(wait_for_script '
const url = new URL(location.href);
const state = JSON.parse(localStorage.getItem("atha.reader.progress." + url.searchParams.get("state") + ".v1") || "null");
return {section: JSON.parse(state?.locator || "null")?.start?.section, locator: state?.locator};' \
        ".section == \"$chapter_one\" and .locator != null" 'TOC did not navigate to Chapter 1')
      reader_chrome
      webdriver_click_element 'button[aria-label="下一页"]' || die 'could not navigate forward'
      wait_for_script '
const key = "atha.reader.progress." + new URL(location.href).searchParams.get("state") + ".v1";
return {locator: JSON.parse(localStorage.getItem(key) || "null")?.locator};' \
        ".locator != $(jq -c .locator <<<"$before")" 'forward navigation did not change the locator' >/dev/null
      webdriver_click_element 'button[aria-label="上一页"]' || die 'could not navigate back'
      wait_for_script '
const key = "atha.reader.progress." + new URL(location.href).searchParams.get("state") + ".v1";
return {locator: JSON.parse(localStorage.getItem(key) || "null")?.locator};' \
        ".locator == $(jq -c .locator <<<"$before")" 'back navigation did not restore the locator' >/dev/null
      # Exercise the current scrubber via its keyboard interaction.
      execute_sync 'document.querySelector(".scrubber-wrap [role=slider]").focus(); return true;' >/dev/null
      webdriver_key $'\uE010' || die 'could not move the progress scrubber to End'
      wait_for_script '
return {progress: Number(document.querySelector(".scrubber-wrap [role=slider]")?.getAttribute("aria-valuenow"))};' \
        '.progress >= 90' 'reader scrubber did not navigate near the end' >/dev/null
    fi

    reader_chrome
    saved=$(execute_sync 'return localStorage.getItem("atha.reader.application.v1");')
    webdriver_click_element 'button[aria-label="排版与主题设置"]' || die 'could not open reading settings'
    capture_public_ui reader-quick-settings
    if ((width == 1000)); then
      click_label '.reader-settings-dialog [role=tab]' '本书覆盖'
      click_label '.reader-settings-dialog [aria-label="阅读方式"] button' '连续滚动'
      wait_for_script '
const r = document.querySelector(".reader");
const bounds = r?.getBoundingClientRect();
const page = document.querySelector(".page")?.getBoundingClientRect();
return {scroll: r?.dataset.readingMode === "scroll",
  centered: bounds && page && Math.abs((page.left - bounds.left) - (bounds.right - page.right)) <= 2,
  width: page?.width,
  changeReady: [...document.querySelectorAll("[aria-label=阅读方式] button")].every(button => !button.disabled)};' \
        '.scroll and .centered and .width >= 700 and .width <= 761 and .changeReady' \
        'scroll mode did not maintain the centered reading measure' >/dev/null
      click_label '.reader-settings-dialog [aria-label="阅读方式"] button' '滑动'
      wait_for_script '
return {paged: document.querySelector(".reader")?.dataset.readingMode === "paged",
  changeReady: [...document.querySelectorAll("[aria-label=阅读方式] button")].every(button => !button.disabled)};' \
        '.paged and .changeReady' 'could not restore paged reading' >/dev/null
      click_label '.reader-settings-dialog [role=tab]' '全书阅读默认'
    fi
    local size
    size=$(execute_sync 'const slider = document.querySelector("[aria-label=页面字号] [role=slider]"); slider?.focus(); return Number(slider?.getAttribute("aria-valuenow"));')
    [[ $size =~ ^[0-9]+$ && $size -lt 40 ]] || die 'could not focus the reader font-size slider'
    webdriver_key $'\uE014' || die 'could not increase the font size'
    wait_for_script '
return {size: JSON.parse(localStorage.getItem("atha.reader.application.v1") || "null")?.preferences?.fontSize};' \
      ".size == $((size + 1))" 'font-size change did not save' >/dev/null
    click_label '.reader-settings-dialog [role=radio]' '夜间'
    wait_for_script '
return {tone: document.querySelector(".reader-root")?.dataset.readerTheme,
  saved: JSON.parse(localStorage.getItem("atha.reader.application.v1") || "null")?.preferences?.theme};' \
      '.tone == "dark" and .saved == "dark"' 'book theme did not apply and persist' >/dev/null
    webdriver_value POST "/session/$session_id/refresh" '{}' >/dev/null
    reader_ready
    wait_for_script 'return {tone:document.querySelector(".reader-root")?.dataset.readerTheme,
  size: JSON.parse(localStorage.getItem("atha.reader.application.v1") || "null")?.preferences?.fontSize};' \
      ".tone == \"dark\" and .size == $((size + 1))" 'book theme and font size did not survive reload' >/dev/null
    execute_sync '
const value = arguments[0];
if (value === null) localStorage.removeItem("atha.reader.application.v1");
else localStorage.setItem("atha.reader.application.v1", value);
return true;' "$(jq -cn --argjson value "$saved" '[$value]')" >/dev/null
    webdriver_value POST "/session/$session_id/refresh" '{}' >/dev/null
    reader_ready
    after=$(execute_sync 'return localStorage.getItem("atha.reader.application.v1");')
    [[ $after == "$saved" ]] || die 'failed to restore the original reader preferences'
  done
  say 'current reader chrome, navigation, bookmark, and theme persistence passed'
}

verify_original_image_preview() {
  local point
  point=$(execute_sync '
const page = document.querySelector(".page").getBoundingClientRect();
return {x: page.left + 112, y: page.top + 54};')
  perform_pointer_action "$point" tap || die 'could not click the EPUB illustration'
  wait_for_script '
const popup = document.querySelector(".atha-lightbox");
return {opened: Boolean(popup?.querySelector(".lightbox-stage img, .lightbox-stage svg")),
  fits: popup?.getBoundingClientRect().width === innerWidth};' \
    '.opened and .fits' 'the reader did not open the EPUB illustration' >/dev/null
  point=$(execute_sync '
const r = document.querySelector(".lightbox-stage img,.lightbox-stage svg").getBoundingClientRect();
return {x: r.left + r.width / 2, y: r.top + r.height / 2};')
  perform_pointer_action "$point" tap || die 'could not click inside the preview'
  [[ $(execute_sync 'return Boolean(document.querySelector(".atha-lightbox"));') == true ]] ||
    die 'clicking the illustration dismissed its preview'
  perform_pointer_action '{"x":4,"y":4}' tap || die 'could not click the lightbox backdrop'
  wait_for_script 'return {closed: !document.querySelector(".atha-lightbox")};' \
    '.closed' 'lightbox backdrop did not restore reading' >/dev/null
}

verify_authoring_cycle() {
  local fixture="$run_root/AthaSourceReturn.epub" marker reply selector edition section point quote
  local row width height jump viewports='[]'
  marker="atha-linux-original-cycle-$BASHPID-$RANDOM"
  # Exercise long-message scroll and composer reachability with normal
  # paragraphs. A single unbroken 1600-character paragraph is a different
  # stress case for the medium workspace's intrinsic width.
  reply="$marker-reply"$'\n\n'"$(printf '长内容片段。长内容片段。长内容片段。长内容片段。\n\n%.0s' {1..80})"
  selector='.shelf-catalog [data-shelf-open][title="Atha Linux Source Return"]'
  python3 "$script_root/tests/reader-linux-epub.py" "$fixture"
  printf '%s\n' "$fixture" "$(basename -- "$fixture")" 'Atha Linux Source Return' '原文锚点' "$marker" >>"$privacy_file"
  open_library "$expected_books"
  set_window_size 360 760
  fire_tauri_drag_event 'tauri://drag-drop' "$fixture" >/dev/null || die 'could not import the original EPUB'
  wait_for_script '
return {ready: Boolean(document.querySelector(".shelf-catalog [title=\"Atha Linux Source Return\"]")),
  count: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length};' \
    ".ready and .count == $((expected_books + 1))" 'original EPUB did not import' >/dev/null
  edition=$(execute_sync 'return document.querySelector(".shelf-catalog [title=\"Atha Linux Source Return\"]")?.dataset.shelfOpen;' | jq -er .)
  printf '%s\n' "$edition" >>"$privacy_file"
  set_shelf_query 'Atha Linux Source Return'
  wait_for_script '
return {titles: [...document.querySelectorAll(".shelf-catalog [data-shelf-open]")].map(el => el.title)};' \
    '.titles == ["Atha Linux Source Return"]' 'shelf search did not find the original EPUB' >/dev/null
  webdriver_click_element "$selector" || die 'could not open the EPUB from the catalogue'
  reader_ready
  capture_public_ui authoring-illustration
  verify_original_image_preview
  reader_chrome
  webdriver_click_element 'button[aria-label="目录与全书搜索"]' || die 'could not open the EPUB directory'
  click_label '.toc-jump' '原文章节'
  wait_for_script '
return {chapter: document.querySelector(".island-chapter,.running-chapter span")?.textContent.trim(),
  closed: !document.querySelector(".directory-dialog")};' \
    '.chapter == "原文章节" and .closed' 'original EPUB did not navigate to its text chapter' >/dev/null
  section=$(reader_progress | jq -er .section) || die 'could not identify the original text section'
  # The authored fixture repeats the same public line throughout this chapter,
  # so a real drag within the measured reading column hits selectable text.
  capture_public_ui authoring-selection-page
  point=$(execute_sync 'const r=document.querySelector(".page").getBoundingClientRect();
return {x:r.left+3,y:r.top+77,endX:r.left+188,endY:r.top+77};')
  perform_pointer_action "$point" drag || die 'trusted mouse drag failed on the original EPUB'
  wait_for_script 'return {actions: Boolean(document.querySelector(".selection-capsule")), selectedLength: window.getSelection()?.toString().length};' \
    '.actions' 'trusted mouse drag did not produce selection actions' >/dev/null
  click_label '.selection-capsule button' '查词'
  wait_for_script 'return {open: Boolean(document.querySelector(".dict-card"))};' '.open' 'selection lookup did not open' >/dev/null
  webdriver_click_element 'button[aria-label="关闭词典"]' || die 'could not dismiss lookup'
  click_label '.selection-capsule button' '对话'
  wait_for_script '
return {source: document.querySelector(".pane-c .source-quote")?.textContent.includes("原文锚点"),
  editor: Boolean(document.querySelector(".pane-c .ProseMirror"))};' \
    '.source and .editor' 'conversation draft did not capture the selected source text' >/dev/null
  quote=$(execute_sync 'return document.querySelector(".pane-c .source-quote p").textContent;')
  webdriver_send_keys '.pane-c .ProseMirror' "$marker" || die 'could not enter the synthetic note'
  click_label_or_aria_send '创建对话'
  wait_for_script '
return {messages: document.querySelectorAll(".pane-c .message-node").length,
  editor: Boolean(document.querySelector(".pane-c .ProseMirror"))};' \
    '.messages == 1 and .editor' 'source-linked note did not save' >/dev/null
  webdriver_click_element '.pane-c button[aria-label="排版工具"]' || die 'could not open reply format tools'
  webdriver_click_element '.pane-c button[aria-label="切换 Markdown 源码模式"]' || die 'could not switch the reply to Markdown'
  wait_for_script 'return {input: Boolean(document.querySelector(".pane-c .markdown-textarea"))};' \
    '.input' 'the Markdown reply editor did not appear' >/dev/null
  set_text_input '.pane-c .markdown-textarea' "$reply"
  click_label_or_aria_send '发送'
  wait_for_script '
const messages = [...document.querySelectorAll(".pane-c .msg-content")];
return {messages:messages.length, long: messages.some(el => el.textContent.length > 1500)};' \
    '.messages == 2 and .long' 'long reply did not save' >/dev/null
  webdriver_click_element '.pane-c button[aria-label="关闭笔记工作区"]' || die 'could not close source discussion'
  reader_back
  wait_for_script 'return {shelf: Boolean(document.querySelector(".library-shell"))};' '.shelf' 'authoring did not return to the shelf' >/dev/null
  set_shelf_query ''
  # Additional book exists only in this isolated authoring store.
  expected_books=$((expected_books + 1))
  for width in 360 1000 1600; do
    height=760
    ((width != 1600)) || height=900
    open_library "$expected_books"
    set_window_size "$width" "$height"
    nav_tab '阅读记忆'
    set_text_input '.memory-shell input[type=search]' "$marker"
    wait_for_script '
const rows = [...document.querySelectorAll(".hit-row")];
return {count: rows.length, sameBook: rows.every(el => el.querySelector(".hit-book")?.textContent === "Atha Linux Source Return"),
  quotes: rows.every(el => el.querySelector(".hit-quote")?.textContent.includes("原文锚点")),
  overflow: document.documentElement.scrollWidth > innerWidth};' \
      '.count == 2 and .sameBook and .quotes and (.overflow | not)' 'memory query missed the original note or reply' >/dev/null
    # Eliminate ordinary progress restoration as the reason the right section opens.
    execute_sync 'localStorage.removeItem(arguments[0]); return true;' \
      "$(jq -cn --arg key "atha.reader.progress.${edition:0:16}.v1" '[$key]')" >/dev/null
    row=$(execute_sync '
const button = [...document.querySelectorAll(".hit-row")].find(el => el.querySelector(".hit-note")?.textContent.includes(arguments[0]))?.querySelector(".jump-btn");
button?.scrollIntoView({block:"center", inline:"nearest"});
return button || null;' "$(jq -cn --arg reply "$marker-reply" '[$reply]')" |
      jq -er '.["element-6066-11e4-a52e-4f735466cecf"]') || die 'long reply had no source jump'
    webdriver_click_element_ref "$row" || die 'could not jump to the EPUB source'
    reader_ready
    wait_for_script 'return {dialog: Boolean(document.querySelector(".pane-c .send-btn"))};' \
      '.dialog' 'restored thread did not initialize its composer' >/dev/null
    capture_public_ui authoring-source-return
    jump=$(wait_for_script '
const params = new URL(location.href).searchParams;
const saved = JSON.parse(localStorage.getItem("atha.reader.progress." + params.get("state") + ".v1") || "null");
const loc = JSON.parse(saved?.locator || "null");
const scope = document.querySelector(".pane-c");
const scroll = scope?.querySelector(".thread-scroll");
const send = scope?.querySelector(".send-btn")?.getBoundingClientRect();
if (scroll) scroll.scrollTop = scroll.scrollHeight;
return {width:innerWidth, edition: saved?.contentVersion, section: loc?.start?.section,
  quote: scope?.querySelector(".source-quote p")?.textContent,
  messages: scope?.querySelectorAll(".message-node").length,
  scrolled: scroll?.scrollTop > 0,
  threadRect: scope?.getBoundingClientRect().toJSON(),
  scrollHeight: scroll?.scrollHeight, clientHeight: scroll?.clientHeight, sendRect: send?.toJSON(),
  sendVisible: send && send.width > 0 && send.left >= 0 && send.right <= innerWidth && send.bottom <= innerHeight + 1};' \
      ".edition == \"$edition\" and .section == \"$section\" and .quote == $quote and .messages == 2 and .scrolled and .sendVisible" \
      'memory jump did not restore the saved EPUB source and its conversation')
    webdriver_send_keys '.pane-c .ProseMirror' "draft-$marker" || die 'could not type in the returned composer'
    wait_for_script 'return {send: Boolean(document.querySelector(".pane-c .send-btn:not(:disabled)"))};' \
      '.send' 'returned composer did not accept a new draft' >/dev/null
    viewports=$(jq -cn --argjson prior "$viewports" --argjson width "$(jq .width <<<"$jump")" \
      '$prior + [{width:$width, searchHits:2, sameEdition:true, sameSection:true, sourceQuote:true, longReply:true, sendVisible:true, sendEnabled:true}]')
    webdriver_click_element '.pane-c button[aria-label="关闭笔记工作区"]' || die 'could not close restored conversation'
    reader_back
    wait_for_script '
return {memory: Boolean(document.querySelector(".memory-shell")), results: document.querySelectorAll(".hit-row").length};' \
      '.memory and .results == 2' 'source return did not restore the results list' >/dev/null
  done
  authoring_cycle_evidence=$(jq -cn --argjson viewports "$viewports" \
    '{fixture:"original EPUB", imagePreview:true, trustedMouseSelection:true, noteSaved:true, replySent:true, reopened:true,
      searchHits:2, jumpToSource:true, sameEdition:true, sameSection:true, viewports:$viewports}')
}

click_label_or_aria_send() {
  wait_for_script '
return {ready: Boolean(document.querySelector(".pane-c .send-btn:not(:disabled)"))};' \
    '.ready' 'conversation send did not become available' >/dev/null
  webdriver_click_element ".pane-c .send-btn[aria-label='$1']" || die 'could not submit the conversation message'
}
