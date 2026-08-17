# ADR-0011：书架自定义封面

## 状态

accepted

## 日期

2026-08-18

## 背景

书籍内置封面可能缺失、不合适或在首次准备前暂不可用。封面是书架元数据而不是书内正文，不能由 Svelte 直接持有路径或绕过不可信图片边界，也不能只存在 WebView 缓存而被完整资料备份遗漏。

## 决定

1. `LocalLibrary` 在既有 `Library/` 内保存按完整书籍身份命名的自定义封面，只接受 JPEG、PNG 或 WebP，并复用 16 MiB、8192 单边和 20000000 像素边界。
2. 自定义封面优先于 importer 发布的内置封面；恢复操作只移除自定义版本并回退内置封面，不改书源或导入缓存。
3. 列表 DTO 只增加 `hasCustomCover`，资源仍由 `atha-cover` 协议返回；Tauri picker 不把路径、图片字节或书籍身份写入日志。
4. 自定义封面随书架记录进入 schema 1 `.atha-data`；旧备份仍可恢复，新备份只能由理解该严格 entry 的版本恢复。移出书架或删除本地数据均清理该封面。

## 后果

- 封面选择、校验、备份和读取只有一个后端事实所有者，前端不会形成第二份持久化状态。
- 自定义封面不提供裁剪、编辑、联网检索或格式转换；这些能力只有出现真实需求后才重新决策。
- 回退到旧版本不会破坏书源与书架 JSON，但旧版本会把新增封面 entry 视为未知，因此不能恢复由新版本生成的完整资料备份。

## 检查位置

- `backend/atha-backend/src/reader/library.rs`；
- `backend/atha-backend/src/local_data.rs`；
- `backend/atha-backend/tests/local_data.rs`；
- `reader/app/src-tauri/src/lib.rs`；
- `reader/app/src/components/LibraryView.svelte`。
