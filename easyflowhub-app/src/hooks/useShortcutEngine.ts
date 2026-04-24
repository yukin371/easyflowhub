import { useEffect } from 'react';
import type { RefObject } from 'react';
import { matchesShortcutEvent, type ShortcutConfig } from '../types/shortcut';
import { applyHeadingLevelToSelection } from '../lib/editorTransforms';

interface SelectionUpdate {
  value: string;
  selectionStart: number;
  selectionEnd?: number;
}

interface UseShortcutEngineOptions {
  enabled: boolean;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  config: ShortcutConfig;
  onSave: () => void | Promise<void>;
  onClose: () => void | Promise<void>;
  onChange: (update: SelectionUpdate) => void;
  onUndo?: () => void;
  onRedo?: () => void;
  canUndo?: boolean;
  canRedo?: boolean;
}

function getLineRange(value: string, selectionStart: number, selectionEnd: number) {
  const lineStart = value.lastIndexOf('\n', Math.max(0, selectionStart - 1)) + 1;
  let lineEnd = value.indexOf('\n', selectionEnd);
  if (lineEnd === -1) {
    lineEnd = value.length;
  }

  const hasTrailingNewline = lineEnd < value.length;
  const lineText = value.slice(lineStart, lineEnd);
  return { lineStart, lineEnd, lineText, hasTrailingNewline };
}

function isNativeCutShortcut(shortcut: string): boolean {
  return shortcut.trim().toLowerCase() === 'ctrl+x';
}

export function useShortcutEngine(options: UseShortcutEngineOptions) {
  const { enabled, textareaRef, config, onSave, onClose, onChange, onUndo, onRedo, canUndo = false, canRedo = false } = options;

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) {
        return;
      }

      const element = textareaRef.current;
      if (!element || document.activeElement !== element) {
        return;
      }

      if ((event.ctrlKey || event.metaKey) && !event.shiftKey && event.key.toLowerCase() === 'z') {
        if (onUndo && canUndo) {
          event.preventDefault();
          onUndo();
        }
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'z') {
        if (onRedo && canRedo) {
          event.preventDefault();
          onRedo();
        }
        return;
      }

      const { value, selectionStart, selectionEnd } = element;

      if (matchesShortcutEvent(event, config.save)) {
        event.preventDefault();
        void onSave();
        return;
      }

      if (matchesShortcutEvent(event, config.close)) {
        event.preventDefault();
        void onClose();
        return;
      }

      if (matchesShortcutEvent(event, config.duplicateLine)) {
        event.preventDefault();
        const { lineEnd, lineText, hasTrailingNewline } = getLineRange(
          value,
          selectionStart,
          selectionEnd
        );
        const insertion = `${lineText}${hasTrailingNewline ? '\n' : ''}`;
        const nextValue =
          value.slice(0, lineEnd) + (hasTrailingNewline ? '' : '\n') + insertion + value.slice(lineEnd);
        const caret = lineEnd + (hasTrailingNewline ? 1 : 1) + lineText.length;
        onChange({ value: nextValue, selectionStart: caret, selectionEnd: caret });
        return;
      }

      const matchedDeleteLine = matchesShortcutEvent(event, config.deleteLine);
      const matchedCutLine = matchesShortcutEvent(event, config.cutLine);

      if (matchedDeleteLine || matchedCutLine) {
        if (matchedCutLine && isNativeCutShortcut(config.cutLine) && selectionStart !== selectionEnd) {
          return;
        }

        event.preventDefault();
        const { lineStart, lineEnd, lineText, hasTrailingNewline } = getLineRange(
          value,
          selectionStart,
          selectionEnd
        );
        const removeEnd = hasTrailingNewline ? lineEnd + 1 : lineEnd;
        const nextValue = value.slice(0, lineStart) + value.slice(removeEnd);
        const caret = Math.min(lineStart, nextValue.length);
        onChange({ value: nextValue, selectionStart: caret, selectionEnd: caret });

        if (matchedCutLine && navigator.clipboard) {
          void navigator.clipboard.writeText(lineText).catch(() => undefined);
        }
        return;
      }

      if (matchesShortcutEvent(event, config.insertLineBelow)) {
        event.preventDefault();
        const { lineEnd, hasTrailingNewline } = getLineRange(value, selectionStart, selectionEnd);
        const insertionPoint = lineEnd;
        const nextValue =
          value.slice(0, insertionPoint) +
          (hasTrailingNewline ? '' : '\n') +
          '\n' +
          value.slice(insertionPoint + (hasTrailingNewline ? 0 : 0));
        const caret = insertionPoint + (hasTrailingNewline ? 1 : 2);
        onChange({ value: nextValue, selectionStart: caret, selectionEnd: caret });
        return;
      }

      if (matchesShortcutEvent(event, config.insertLineAbove)) {
        event.preventDefault();
        const { lineStart } = getLineRange(value, selectionStart, selectionEnd);
        const nextValue = value.slice(0, lineStart) + '\n' + value.slice(lineStart);
        onChange({ value: nextValue, selectionStart: lineStart, selectionEnd: lineStart });
        return;
      }

      if (matchesShortcutEvent(event, config.heading1)) {
        event.preventDefault();
        onChange(applyHeadingLevelToSelection(value, selectionStart, selectionEnd, 1));
        return;
      }

      if (matchesShortcutEvent(event, config.heading2)) {
        event.preventDefault();
        onChange(applyHeadingLevelToSelection(value, selectionStart, selectionEnd, 2));
        return;
      }

      if (matchesShortcutEvent(event, config.heading3)) {
        event.preventDefault();
        onChange(applyHeadingLevelToSelection(value, selectionStart, selectionEnd, 3));
        return;
      }

      if (matchesShortcutEvent(event, config.clearHeading)) {
        event.preventDefault();
        onChange(applyHeadingLevelToSelection(value, selectionStart, selectionEnd, 0));
      }
    };

    textarea.addEventListener('keydown', handleKeyDown);
    return () => textarea.removeEventListener('keydown', handleKeyDown);
  }, [canRedo, canUndo, config, enabled, onChange, onClose, onRedo, onSave, onUndo, textareaRef]);
}
