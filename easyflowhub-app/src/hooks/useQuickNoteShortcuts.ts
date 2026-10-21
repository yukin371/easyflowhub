import { useEffect } from 'react';
import { matchesShortcutEvent, type ShortcutConfig } from '../types/shortcut';

export interface QuickNoteShortcutActions {
  /** Undo the draft history. */
  onUndo: () => void;
  /** Redo the draft history. */
  onRedo: () => void;
  /** Close the current quick-note window (flush + destroy). */
  onClose: () => void;
  /** Toggle the always-on-top flag through the native window command. */
  onToggleAlwaysOnTop: () => void;
  /** Toggle "ghost mode" (opacity 0). */
  onToggleGhost: () => void;
  /** Open or close the note switcher. */
  onOpenSwitcher: () => void;
  /** Close the switcher overlay (Escape) without leaving the window. */
  onCloseSwitcher: () => void;
  /** Open or close the tools panel. */
  onToggleTools: () => void;
}

interface UseQuickNoteShortcutsOptions extends QuickNoteShortcutActions {
  canUndo: boolean;
  canRedo: boolean;
  /** The switcher owns Escape while it is open. */
  isSwitcherOpen: boolean;
  shortcutConfig: ShortcutConfig;
}

function isCommandKey(event: KeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey;
}

/**
 * Owns the quick-note window's global keyboard map (undo / redo, note switcher,
 * always-on-top, Escape and Ctrl+D close, ghost mode and the tools panel).
 * QuickNotePage keeps the state and rendering; the ordering and precedence of
 * the shortcuts live here so the page stops growing around them.
 */
export function useQuickNoteShortcuts({
  canUndo,
  canRedo,
  isSwitcherOpen,
  shortcutConfig,
  onUndo,
  onRedo,
  onClose,
  onToggleAlwaysOnTop,
  onToggleGhost,
  onOpenSwitcher,
  onCloseSwitcher,
  onToggleTools,
}: UseQuickNoteShortcutsOptions) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!document.hasFocus()) {
        return;
      }

      const key = event.key.toLowerCase();
      const commandKey = isCommandKey(event);

      // 撤销 / 重做仅在切换器关闭时交给编辑器
      if (commandKey && !event.shiftKey && key === 'z' && !isSwitcherOpen && canUndo) {
        event.preventDefault();
        onUndo();
        return;
      }

      if (commandKey && event.shiftKey && key === 'z' && !isSwitcherOpen && canRedo) {
        event.preventDefault();
        onRedo();
        return;
      }

      if (matchesShortcutEvent(event, shortcutConfig.quickSwitcher)) {
        event.preventDefault();
        onOpenSwitcher();
        return;
      }

      if (matchesShortcutEvent(event, shortcutConfig.toggleAlwaysOnTop)) {
        event.preventDefault();
        onToggleAlwaysOnTop();
        return;
      }

      if (isSwitcherOpen && event.key === 'Escape') {
        event.preventDefault();
        onCloseSwitcher();
        return;
      }

      if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
        return;
      }

      if (commandKey && key === 'd') {
        event.preventDefault();
        onClose();
        return;
      }

      if (commandKey && event.shiftKey && key === 'h') {
        event.preventDefault();
        onToggleGhost();
        return;
      }

      if (commandKey && event.key === '.') {
        event.preventDefault();
        onToggleTools();
      }
    };

    window.addEventListener('keydown', handleKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', handleKeyDown, { capture: true });
  }, [
    canRedo,
    canUndo,
    isSwitcherOpen,
    onClose,
    onCloseSwitcher,
    onOpenSwitcher,
    onToggleAlwaysOnTop,
    onToggleGhost,
    onToggleTools,
    onRedo,
    onUndo,
    shortcutConfig.quickSwitcher,
    shortcutConfig.toggleAlwaysOnTop,
  ]);
}
