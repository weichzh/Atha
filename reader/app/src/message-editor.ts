import { StarterKit } from "@tiptap/starter-kit";

export function isSafeMessageLink(value: string) {
  try {
    return ["http:", "https:"].includes(new URL(value).protocol);
  } catch {
    return false;
  }
}

export const messageExtensions = [
  StarterKit.configure({
    code: false,
    codeBlock: false,
    heading: { levels: [1, 2, 3] },
    horizontalRule: false,
    strike: false,
    underline: false,
    link: {
      autolink: false,
      linkOnPaste: false,
      openOnClick: false,
      HTMLAttributes: { target: null, rel: "noopener noreferrer", class: null },
      isAllowedUri: (url) => isSafeMessageLink(url),
    },
  }),
];
