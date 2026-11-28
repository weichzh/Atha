import { copyAppText, readDesktopClipboard } from "./desktop-clipboard.ts";

export interface DesktopMenuItem {
  label: string;
  select: () => void | Promise<void>;
  disabled?: boolean;
  separate?: boolean;
}

function editTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return null;
  const field = target.closest("input, textarea");
  if (field instanceof HTMLTextAreaElement ||
    (field instanceof HTMLInputElement && /^(text|search|url|tel|password)$/.test(field.type))) {
    return field.disabled ? null : field;
  }
  if (!target.isContentEditable) return null;
  // Use the editing host, not a paragraph or the inline element under the pointer.
  let host = target;
  while (host.parentElement?.isContentEditable) host = host.parentElement;
  return host;
}

function selectionAt(editor: HTMLElement) {
  if (editor instanceof HTMLInputElement || editor instanceof HTMLTextAreaElement) {
    const start = editor.selectionStart ?? 0;
    const end = editor.selectionEnd ?? start;
    const direction = editor.selectionDirection ?? "none";
    const value = editor.value;
    return {
      text: value.slice(start, end),
      restore: () => editor.setSelectionRange(start, end, direction),
      stillCurrent: () => editor.value === value && editor.selectionStart === start && editor.selectionEnd === end,
      selectAll: () => editor.select(),
    };
  }

  const selection = editor.ownerDocument.getSelection();
  const candidate = selection?.rangeCount ? selection.getRangeAt(0) : null;
  const range = candidate && editor.contains(candidate.commonAncestorContainer) ? candidate.cloneRange() : null;
  const text = range?.toString() ?? "";
  const setRange = (next: Range) => {
    selection?.removeAllRanges();
    selection?.addRange(next);
  };
  return {
    text,
    restore: () => {
      if (range && editor.contains(range.commonAncestorContainer)) setRange(range);
    },
    stillCurrent: () => {
      if (!range || !selection?.rangeCount) return false;
      const current = selection.getRangeAt(0);
      return range.startContainer === current.startContainer && range.startOffset === current.startOffset &&
        range.endContainer === current.endContainer && range.endOffset === current.endOffset &&
        current.toString() === text;
    },
    selectAll: () => {
      const all = editor.ownerDocument.createRange();
      all.selectNodeContents(editor);
      setRange(all);
    },
  };
}

export function editMenuFor(target: EventTarget | null) {
  const editor = editTarget(target);
  if (!editor) return null;
  const snapshot = selectionAt(editor);
  const doc = editor.ownerDocument;
  const field = editor instanceof HTMLInputElement || editor instanceof HTMLTextAreaElement ? editor : null;
  const mayChange = () => field ? !field.readOnly && !field.disabled : editor.isContentEditable;
  const canCopy = snapshot.text.length > 0 && !(editor instanceof HTMLInputElement && editor.type === "password");

  const restore = () => {
    if (!editor.isConnected) return false;
    editor.focus({ preventScroll: true });
    snapshot.restore();
    return doc.activeElement === editor;
  };

  const change = (command: "delete" | "insertText", text?: string) => {
    if (!editor.isConnected || doc.activeElement !== editor || !mayChange() || !snapshot.stillCurrent()) return;
    if (!doc.execCommand(command, false, text)) throw new Error("Text edit was not applied");
  };

  const items: DesktopMenuItem[] = [
    {
      label: "剪切",
      disabled: !canCopy || !mayChange(),
      select: async () => {
        if (!restore()) return;
        await copyAppText(snapshot.text);
        change("delete");
      },
    },
    {
      label: "复制",
      disabled: !canCopy,
      select: async () => {
        if (restore()) await copyAppText(snapshot.text);
      },
    },
    {
      label: "粘贴纯文本",
      disabled: !mayChange(),
      select: async () => {
        if (!restore()) return;
        // Read only on this explicit action, and don't overwrite edits made while it was pending.
        const text = await readDesktopClipboard();
        if (text) change("insertText", text);
      },
    },
    {
      label: "全选",
      separate: true,
      select: () => {
        if (restore()) snapshot.selectAll();
      },
    },
  ];
  return { editor, items, restore };
}
