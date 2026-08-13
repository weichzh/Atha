const UTF8 = new TextEncoder();

export function parseUserStylesheet(css) {
  if (typeof css !== "string" || UTF8.encode(css).length > 65536) {
    throw new Error("invalid-user-style");
  }
  if (/@import|(?:url|src|image|image-set)\s*\(/i.test(css) || css.includes("\\")) {
    throw new Error("css-subresource");
  }
  if (/:host(?:-context)?\b|::part\b|::slotted\b/i.test(css)) {
    throw new Error("active-style");
  }
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(css);
  const inspect = (rules) => {
    for (const rule of rules) {
      if (/(?:url|src|image|image-set)\s*\(/i.test(rule.cssText) || rule.cssText.includes("\\")) {
        throw new Error("css-subresource");
      }
      if (/:host(?:-context)?\b|::part\b|::slotted\b/i.test(rule.cssText)) {
        throw new Error("active-style");
      }
      if (rule.cssRules) inspect(rule.cssRules);
    }
  };
  inspect(sheet.cssRules);
  return sheet;
}

export function validateUserStylesheet(css, parse = parseUserStylesheet) {
  const sheet = parse(css);
  const uncommented = css.replace(/\/\*[\s\S]*?\*\//gu, "").trim();
  if (uncommented && sheet.cssRules.length === 0) throw new Error("invalid-user-style");
}
