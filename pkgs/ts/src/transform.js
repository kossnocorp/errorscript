const annotation = ": errtype";
const errorType = `
/**
 * ErrorScript-typed error. It is \`any\` alias to disable TypeScript and let
 * the ErrorScript type system to do the type checking.
 */
type errtype = any
`;

/**
 * Temporary regex-only transform, deliberately unaware of strings and comments.
 * @param {string} content
 */
export function transform(content) {
  let text = "";
  let originalStart = 0;
  /** @type {[number, number, number, number, 0][]} */
  const mappings = [];

  /** @param {number} end */
  function copyThrough(end) {
    const length = end - originalStart;
    if (length > 0) {
      // Verbatim spans use UTF-16 offsets, matching JavaScript string indices.
      mappings.push([text.length, length, originalStart, length, 0]);
      text += content.slice(originalStart, end);
    }
    originalStart = end;
  }

  // Only bare identifier bindings; already annotated and bindingless catches
  // are left alone. The lookahead preserves whitespace before the closing paren.
  for (const match of content.matchAll(/\bcatch\s*\(\s*([A-Za-z_$][\w$]*)(?=\s*\))/g)) {
    copyThrough(match.index + match[0].length);
    text += annotation;
  }
  copyThrough(content.length);
  // Inserted annotations and the alias are synthesized (unmapped) text.
  text += errorType;
  return { text, extension: ".ts", mappings };
}
