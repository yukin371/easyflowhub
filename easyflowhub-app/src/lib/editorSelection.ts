import type { RefObject } from 'react';

export interface EditorSelectionUpdate {
  value: string;
  selectionStart: number;
  selectionEnd?: number;
}

export function focusTextareaAtEnd(
  textareaRef: RefObject<HTMLTextAreaElement | null>,
  delayMs: number = 0
) {
  const focus = () => {
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }
    textarea.focus();
    const caret = textarea.value.length;
    textarea.setSelectionRange(caret, caret);
  };

  if (delayMs <= 0) {
    window.requestAnimationFrame(focus);
    return () => undefined;
  }

  const timer = window.setTimeout(focus, delayMs);
  return () => window.clearTimeout(timer);
}

export function applyTextareaSelectionUpdate(
  textareaRef: RefObject<HTMLTextAreaElement | null>,
  update: EditorSelectionUpdate,
  applyValue: (value: string) => void
) {
  applyValue(update.value);

  window.requestAnimationFrame(() => {
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }
    textarea.focus();
    textarea.setSelectionRange(
      update.selectionStart,
      update.selectionEnd ?? update.selectionStart
    );
  });
}
