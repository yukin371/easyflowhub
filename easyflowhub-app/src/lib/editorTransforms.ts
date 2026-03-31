export interface TextSelectionUpdate {
  value: string;
  selectionStart: number;
  selectionEnd?: number;
}

function getLineBounds(value: string, selectionStart: number, selectionEnd: number) {
  const lineStart = value.lastIndexOf('\n', Math.max(0, selectionStart - 1)) + 1;
  let lineEnd = value.indexOf('\n', selectionEnd);
  if (lineEnd === -1) {
    lineEnd = value.length;
  }
  return { lineStart, lineEnd };
}

function stripHeadingPrefix(line: string) {
  return line.replace(/^\s*(?:#{1,6}\s+|(?:\d+\.)+\s+)/, '');
}

export function insertSnippetAtCursor(
  value: string,
  selectionStart: number,
  selectionEnd: number,
  snippet: string
): TextSelectionUpdate {
  const nextValue = `${value.slice(0, selectionStart)}${snippet}${value.slice(selectionEnd)}`;
  const caret = selectionStart + snippet.length;
  return {
    value: nextValue,
    selectionStart: caret,
    selectionEnd: caret,
  };
}

export function applyHeadingLevelToSelection(
  value: string,
  selectionStart: number,
  selectionEnd: number,
  level: 0 | 1 | 2 | 3
): TextSelectionUpdate {
  const { lineStart, lineEnd } = getLineBounds(value, selectionStart, selectionEnd);
  const selected = value.slice(lineStart, lineEnd);
  const transformed = selected
    .split('\n')
    .map((line) => {
      const trimmed = stripHeadingPrefix(line);
      if (!trimmed.trim()) {
        return trimmed;
      }
      return level === 0 ? trimmed : `${'#'.repeat(level)} ${trimmed}`;
    })
    .join('\n');

  const nextValue = `${value.slice(0, lineStart)}${transformed}${value.slice(lineEnd)}`;
  return {
    value: nextValue,
    selectionStart: lineStart,
    selectionEnd: lineStart + transformed.length,
  };
}
