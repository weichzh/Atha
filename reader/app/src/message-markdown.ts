import { type JSONContent } from "@tiptap/core";
import { type Schema } from "@tiptap/pm/model";
import {
  defaultMarkdownParser,
  defaultMarkdownSerializer,
  MarkdownParser,
  MarkdownSerializer,
} from "prosemirror-markdown";
import { isSafeMessageLink } from "./message-editor.ts";

export function createMessageMarkdownCodec(schema: Schema) {
  const defaults = defaultMarkdownParser.tokens;
  const parser = new MarkdownParser(schema, defaultMarkdownParser.tokenizer, {
    blockquote: { ...defaults.blockquote, block: "blockquote" },
    paragraph: { ...defaults.paragraph, block: "paragraph" },
    list_item: { ...defaults.list_item, block: "listItem" },
    bullet_list: { ...defaults.bullet_list, block: "bulletList" },
    ordered_list: { ...defaults.ordered_list, block: "orderedList" },
    heading: { ...defaults.heading, block: "heading" },
    hardbreak: { ...defaults.hardbreak, node: "hardBreak" },
    em: { ...defaults.em, mark: "italic" },
    strong: { ...defaults.strong, mark: "bold" },
    link: { ...defaults.link, mark: "link" },
  });
  const serializer = new MarkdownSerializer(
    {
      blockquote: defaultMarkdownSerializer.nodes.blockquote,
      paragraph: defaultMarkdownSerializer.nodes.paragraph,
      listItem: defaultMarkdownSerializer.nodes.list_item,
      bulletList: defaultMarkdownSerializer.nodes.bullet_list,
      orderedList: defaultMarkdownSerializer.nodes.ordered_list,
      heading: defaultMarkdownSerializer.nodes.heading,
      hardBreak: defaultMarkdownSerializer.nodes.hard_break,
      text: defaultMarkdownSerializer.nodes.text,
    },
    {
      italic: defaultMarkdownSerializer.marks.em,
      bold: defaultMarkdownSerializer.marks.strong,
      link: defaultMarkdownSerializer.marks.link,
    },
  );

  function validate(document: ReturnType<typeof parser.parse>) {
    document.descendants((node) => {
      if (node.type.name === "heading" && node.attrs.level > 3) {
        throw new Error("unsupported-markdown");
      }
      for (const mark of node.marks) {
        if (mark.type.name === "link" && !isSafeMessageLink(mark.attrs.href)) {
          throw new Error("unsupported-markdown");
        }
      }
    });
    return document;
  }

  return Object.freeze({
    parse(markdown: string): JSONContent {
      return validate(parser.parse(markdown)).toJSON();
    },
    serialize(document: JSONContent): string {
      return serializer.serialize(validate(schema.nodeFromJSON(document)));
    },
  });
}
