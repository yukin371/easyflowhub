import type React from 'react';
import type { TextSelectionUpdate } from './editorTransforms';

interface MarkdownKeydownParams {
  key: string;
  value: string;
  selectionStart: number;
  selectionEnd: number;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
}

const CHECKBOX_LIST_REGEX = /^(\s*)((?:[-*])|(?:\d+\.))\s*\[[ xX]\]\s*(.*)$/;
const UNORDERED_LIST_REGEX = /^(\s*)([-*])\s+(.*)$/;
const ORDERED_LIST_REGEX = /^(\s*)(\d+)\.\s+(.*)$/;
const PAIRS: Record<string, string> = {
  '[': ']',
  '(': ')',
  '{': '}',
  '`': '`',
};

function getLineRange(value: string, selectionStart: number, selectionEnd: number) {
  const lineStart = value.lastIndexOf('\n', Math.max(0, selectionStart - 1)) + 1;
  let lineEnd = value.indexOf('\n', selectionEnd);
  if (lineEnd === -1) {
    lineEnd = value.length;
  }
  return {
    lineStart,
    lineEnd,
    line: value.slice(lineStart, lineEnd),
  };
}

function buildSelectionUpdate(
  value: string,
  selectionStart: number,
  selectionEnd: number = selectionStart
): TextSelectionUpdate {
  return {
    value,
    selectionStart,
    selectionEnd,
  };
}

function isPlainTyping(params: MarkdownKeydownParams) {
  return !params.ctrlKey && !params.metaKey && !params.altKey;
}

export function getMarkdownCompletionUpdate(
  params: MarkdownKeydownParams
): TextSelectionUpdate | null {
  const {
    key,
    value,
    selectionStart,
    selectionEnd,
    ctrlKey = false,
    metaKey = false,
    altKey = false,
    shiftKey = false,
  } = params;

  const { lineStart, lineEnd, line } = getLineRange(value, selectionStart, selectionEnd);

  if (key === 'Enter' && !shiftKey && !ctrlKey && !metaKey && !altKey) {
    const checkboxMatch = CHECKBOX_LIST_REGEX.exec(line);
    if (checkboxMatch) {
      const [, indent, marker, text] = checkboxMatch;
      if (!text.trim()) {
        const nextValue = `${value.slice(0, lineStart)}${value.slice(lineEnd)}`;
        return buildSelectionUpdate(nextValue, lineStart);
      }

      const nextMarker = /\d+\./.test(marker)
        ? `${Number.parseInt(marker, 10) + 1}.`
        : marker;
      const insertion = `\n${indent}${nextMarker} [ ] `;
      const nextValue = `${value.slice(0, selectionStart)}${insertion}${value.slice(selectionEnd)}`;
      const caret = selectionStart + insertion.length;
      return buildSelectionUpdate(nextValue, caret);
    }

    const unorderedMatch = UNORDERED_LIST_REGEX.exec(line);
    if (unorderedMatch) {
      const [, indent, marker, text] = unorderedMatch;
      if (!text.trim()) {
        const nextValue = `${value.slice(0, lineStart)}${value.slice(lineEnd)}`;
        return buildSelectionUpdate(nextValue, lineStart);
      }

      const insertion = `\n${indent}${marker} `;
      const nextValue = `${value.slice(0, selectionStart)}${insertion}${value.slice(selectionEnd)}`;
      const caret = selectionStart + insertion.length;
      return buildSelectionUpdate(nextValue, caret);
    }

    const orderedMatch = ORDERED_LIST_REGEX.exec(line);
    if (orderedMatch) {
      const [, indent, numStr, text] = orderedMatch;
      if (!text.trim()) {
        const nextValue = `${value.slice(0, lineStart)}${value.slice(lineEnd)}`;
        return buildSelectionUpdate(nextValue, lineStart);
      }

      const insertion = `\n${indent}${Number.parseInt(numStr, 10) + 1}. `;
      const nextValue = `${value.slice(0, selectionStart)}${insertion}${value.slice(selectionEnd)}`;
      const caret = selectionStart + insertion.length;
      return buildSelectionUpdate(nextValue, caret);
    }
  }

  if (!isPlainTyping(params)) {
    return null;
  }

  if (key in PAIRS) {
    const before = value.slice(0, selectionStart);
    const selected = value.slice(selectionStart, selectionEnd);
    const after = value.slice(selectionEnd);

    if (key === '[') {
      const beforeCursorOnLine = value.slice(lineStart, selectionStart);
      if (/^(\s*)((?:[-*])|(?:\d+\.))\s?$/.test(beforeCursorOnLine)) {
        const nextValue = `${before}[ ] ${after}`;
        const caret = selectionStart + 4;
        return buildSelectionUpdate(nextValue, caret);
      }
    }

    const closeChar = PAIRS[key];
    const nextValue = `${before}${key}${selected}${closeChar}${after}`;
    const nextSelectionStart = selectionStart + 1;
    const nextSelectionEnd = nextSelectionStart + selected.length;
    return buildSelectionUpdate(nextValue, nextSelectionStart, nextSelectionEnd);
  }

  if (key === ']' || key === ')' || key === '}') {
    if (selectionStart === selectionEnd && value[selectionStart] === key) {
      return buildSelectionUpdate(value, selectionStart + 1);
    }
  }

  return null;
}

interface ApplyMarkdownCompletionOptions {
  event: React.KeyboardEvent<HTMLTextAreaElement>;
  value: string;
  onChange: (value: string) => void;
}

export function applyMarkdownCompletion(
  options: ApplyMarkdownCompletionOptions
): boolean {
  const { event, value, onChange } = options;
  const textarea = event.currentTarget;
  const update = getMarkdownCompletionUpdate({
    key: event.key,
    value,
    selectionStart: textarea.selectionStart,
    selectionEnd: textarea.selectionEnd,
    ctrlKey: event.ctrlKey,
    metaKey: event.metaKey,
    altKey: event.altKey,
    shiftKey: event.shiftKey,
  });

  if (!update) {
    return false;
  }

  event.preventDefault();
  onChange(update.value);

  window.requestAnimationFrame(() => {
    textarea.selectionStart = update.selectionStart;
    textarea.selectionEnd = update.selectionEnd ?? update.selectionStart;
  });

  return true;
}
