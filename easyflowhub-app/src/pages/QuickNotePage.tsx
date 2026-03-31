/**
 * QuickNotePage - 快速笔记页面
 * 独立管理快速笔记的逻辑和 UI
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { closeAllNoteWindows, createNote, getNote, listNotes, toggleNoteWindowsVisibility } from '../lib/tauri/notes';
import { deriveDisplayTitle, formatNoteForDisplay } from '../lib/noteParser';
import { buildPersistParams } from '../lib/notePersistence';
import type { Note } from '../types/note';
import { listen } from '@tauri-apps/api/event';
import { EditorTextarea } from '../components/shared/EditorTextarea';
import { useShortcutEngine } from '../hooks/useShortcutEngine';
import { useAutoSave } from '../hooks/useAutoSave';
import { useEditorPreferences } from '../hooks/useEditorPreferences';
import { useHistory } from '../hooks/useHistory';
import { useEditorImageInsertion } from '../hooks/useEditorImageInsertion';
import { matchesShortcutEvent } from '../types/shortcut';
import { applyTextareaSelectionUpdate, focusTextareaAtEnd } from '../lib/editorSelection';

const BG_COLOR = '#FAF8F0';
const AUTOSAVE_DELAY = 900;

function dedupeNotes(notes: Note[]): Note[] {
  const map = new Map<string, Note>();
  for (const note of notes) {
    map.set(note.id, note);
  }
  return Array.from(map.values());
}

function extractImageLinks(content: string): string[] {
  const matches = content.match(/!\[[^\]]*]\((.*?)\)/g) ?? [];
  return matches
    .map((match) => match.match(/!\[[^\]]*]\((.*?)\)/)?.[1]?.trim() ?? '')
    .filter(Boolean);
}

function isContinueCandidate(note: Note): boolean {
  return Boolean(note.content.trim() || note.tags.trim());
}

function buildSwitchLabel(note: Note): string {
  const title = deriveDisplayTitle(note.title, note.content) || '无标题';
  const snippet = note.content
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .find((line) => line !== title);

  return snippet ? `${title} · ${snippet}`.slice(0, 28) : title.slice(0, 28);
}

export function QuickNotePage() {
  const [note, setNote] = useState<Note | null>(null);
  const [alwaysOnTop, setAlwaysOnTop] = useState(true);
  const [opacity, setOpacity] = useState(1.0);
  const [recentNotes, setRecentNotes] = useState<Note[]>([]);
  const [isSwitcherOpen, setIsSwitcherOpen] = useState(false);
  const [switcherQuery, setSwitcherQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [undoSteps, setUndoSteps] = useState(100);
  const [isToolsOpen, setIsToolsOpen] = useState(false);
  const [lastVisibleOpacity, setLastVisibleOpacity] = useState(1);

  // 使用 useHistory 管理内容，支持撤销/重做
  const {
    value: displayContent,
    setValue: setDisplayContent,
    undo,
    redo,
    canUndo,
    canRedo,
    clear: clearHistory,
    flush: flushHistory,
  } = useHistory('', { maxSteps: undoSteps, debounceMs: 400 });

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const switcherInputRef = useRef<HTMLInputElement>(null);
  const toolsButtonRef = useRef<HTMLButtonElement>(null);
  const toolsPanelRef = useRef<HTMLDivElement>(null);
  const lastWheelTime = useRef(0);
  const isClosingRef = useRef(false);
  const {
    shortcutConfig,
    editorSettings,
  } = useEditorPreferences();
  const { flushSave, scheduleSave, resetSaveFeedback } = useAutoSave({
    delayMs: AUTOSAVE_DELAY,
    onSaved: (updated) => {
      setNote(updated);
      setRecentNotes((prev) => {
        const next = prev.some((item) => item.id === updated.id)
          ? prev.map((item) => (item.id === updated.id ? updated : item))
          : [updated, ...prev];
        return dedupeNotes(next).slice(0, 12);
      });
    },
  });

  const currentPersistParams = useMemo(() => {
    return buildPersistParams(note?.id ?? null, displayContent);
  }, [displayContent, note]);

  const refreshRecentNotes = useCallback(async (preferredId?: string | null) => {
    try {
      const loadedNotes = await listNotes();
      const sorted = [...loadedNotes].sort((a, b) => {
        if (a.id === preferredId) {
          return -1;
        }
        if (b.id === preferredId) {
          return 1;
        }
        const aContinue = isContinueCandidate(a);
        const bContinue = isContinueCandidate(b);
        if (aContinue !== bContinue) {
          return aContinue ? -1 : 1;
        }
        return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
      });
      setRecentNotes(dedupeNotes(sorted).slice(0, 12));
    } catch (error) {
      console.error('Failed to refresh note list:', error);
    }
  }, []);

  const toggleGhostMode = useCallback(() => {
    setOpacity((prev) => {
      if (prev <= 0.01) {
        return Math.max(0.2, lastVisibleOpacity);
      }

      setLastVisibleOpacity(prev);
      return 0;
    });
  }, [lastVisibleOpacity]);

  // 关闭窗口
  const closeWindow = useCallback(async () => {
    if (isClosingRef.current) {
      return;
    }
    isClosingRef.current = true;

    flushHistory(); // 刷新历史记录
    try {
      await flushSave(currentPersistParams);
      await invoke('close_window');
    } catch (error) {
      console.error('Failed to close window:', error);
      isClosingRef.current = false;
    }
  }, [currentPersistParams, flushSave, flushHistory]);

  // 初始化笔记
  useEffect(() => {
    const initialize = async () => {
      try {
        await refreshRecentNotes();

        // 从 URL 获取 note_id
        const urlParams = new URLSearchParams(window.location.search);
        const noteId = urlParams.get('note_id');

        if (noteId) {
          const existingNote = await getNote(noteId);
          if (existingNote) {
            setNote(existingNote);
            setDisplayContent(formatNoteForDisplay(existingNote), true); // skipHistory
            resetSaveFeedback();
          } else {
            const newNote = await createNote();
            setNote(newNote);
          }
        } else {
          const newNote = await createNote();
          setNote(newNote);
        }
      } catch (error) {
        console.error('Failed to initialize note:', error);
      }
    };

    initialize();
  }, [refreshRecentNotes]);

  // 自动聚焦
  useEffect(() => {
    return focusTextareaAtEnd(textareaRef, 100);
  }, [resetSaveFeedback]);

  useEffect(() => {
    const body = document.body;
    const html = document.documentElement;
    const root = document.getElementById('root');
    const previousBodyBackground = body.style.background;
    const previousHtmlBackground = html.style.background;
    const previousRootBackground = root?.style.background ?? '';

    body.style.background = 'transparent';
    html.style.background = 'transparent';
    if (root) {
      root.style.background = 'transparent';
    }

    return () => {
      body.style.background = previousBodyBackground;
      html.style.background = previousHtmlBackground;
      if (root) {
        root.style.background = previousRootBackground;
      }
    };
  }, []);

  useEffect(() => {
    setUndoSteps(editorSettings.undo_steps);
  }, [editorSettings.undo_steps]);

  useEffect(() => {
    if (!isSwitcherOpen) {
      return;
    }

    window.requestAnimationFrame(() => {
      switcherInputRef.current?.focus();
      switcherInputRef.current?.select();
    });
    setSelectedIndex(0);
    setIsToolsOpen(false);
  }, [isSwitcherOpen]);

  useEffect(() => {
    if (opacity > 0.01) {
      setLastVisibleOpacity(opacity);
    }
  }, [opacity]);

  useEffect(() => {
    if (isSwitcherOpen || isToolsOpen) {
      return;
    }

    return focusTextareaAtEnd(textareaRef, 20);
  }, [isSwitcherOpen, isToolsOpen]);

  useEffect(() => {
    if (!isToolsOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (!target) {
        return;
      }
      if (toolsPanelRef.current?.contains(target) || toolsButtonRef.current?.contains(target)) {
        return;
      }
      setIsToolsOpen(false);
    };

    window.addEventListener('pointerdown', handlePointerDown);
    return () => window.removeEventListener('pointerdown', handlePointerDown);
  }, [isToolsOpen]);

  // Reset selected index when query changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [switcherQuery]);

  useEffect(() => {
    const handleQuickNoteKeys = (event: KeyboardEvent) => {
      // 撤销: Ctrl+Z
      if ((event.ctrlKey || event.metaKey) && !event.shiftKey && event.key.toLowerCase() === 'z') {
        // 只在不是 switcher 打开时处理
        if (!isSwitcherOpen && canUndo) {
          event.preventDefault();
          undo();
          return;
        }
      }

      // 重做: Ctrl+Shift+Z
      if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'z') {
        if (!isSwitcherOpen && canRedo) {
          event.preventDefault();
          redo();
          return;
        }
      }

      if (matchesShortcutEvent(event, shortcutConfig.quickSwitcher)) {
        event.preventDefault();
        setSwitcherQuery('');
        setIsSwitcherOpen((prev) => !prev);
        setIsToolsOpen(false);
        return;
      }

      if (matchesShortcutEvent(event, shortcutConfig.toggleAlwaysOnTop)) {
        event.preventDefault();
        void handleToggleAlwaysOnTop();
        return;
      }

      if (isSwitcherOpen && event.key === 'Escape') {
        event.preventDefault();
        setIsSwitcherOpen(false);
        setSwitcherQuery('');
        return;
      }

      if (event.key === 'Escape') {
        event.preventDefault();
        void closeWindow();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {
        event.preventDefault();
        void closeWindow();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'h') {
        event.preventDefault();
        toggleGhostMode();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key === '.') {
        event.preventDefault();
        setIsToolsOpen((prev) => !prev);
      }
    };

    window.addEventListener('keydown', handleQuickNoteKeys, { capture: true });
    return () => window.removeEventListener('keydown', handleQuickNoteKeys, { capture: true });
  }, [closeWindow, isSwitcherOpen, shortcutConfig.quickSwitcher, canUndo, canRedo, undo, redo, toggleGhostMode]);

  useEffect(() => {
    if (!currentPersistParams) {
      return;
    }

    scheduleSave(currentPersistParams);
  }, [currentPersistParams, scheduleSave]);

  // 窗口关闭事件
  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    const setupListener = async () => {
      const unlisten = await listen('tauri://close-requested', async (event: any) => {
        if (isClosingRef.current) {
          return;
        }
        const payload = event.payload as { preventDefault?: () => void };
        if (payload?.preventDefault) {
          payload.preventDefault();
        }
        await closeWindow();
      });
      unlistenFn = unlisten;
    };

    setupListener();
    return () => unlistenFn?.();
  }, [closeWindow]);

  // 透明度控制
  useEffect(() => {
    const handleWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();

      const now = Date.now();
      if (now - lastWheelTime.current < 50) return;
      lastWheelTime.current = now;

      const delta = e.deltaY < 0 ? 0.2 : -0.2;
      setOpacity((prev) => Math.max(0, Math.min(1, Math.round((prev + delta) * 5) / 5)));
    };

    window.addEventListener('wheel', handleWheel, { passive: false });
    return () => window.removeEventListener('wheel', handleWheel);
  }, []);

  // 切换置顶
  const handleToggleAlwaysOnTop = async () => {
    try {
      const newState = await invoke<boolean>('toggle_always_on_top');
      setAlwaysOnTop(newState);
    } catch (error) {
      console.error('Failed to toggle always on top:', error);
    }
  };

  const applyShortcutContentChange = useCallback(
    (update: { value: string; selectionStart: number; selectionEnd?: number }) => {
      applyTextareaSelectionUpdate(textareaRef, update, setDisplayContent);
    },
    [setDisplayContent]
  );
  const imageInsertion = useEditorImageInsertion({
    value: displayContent,
    textareaRef,
    onChange: applyShortcutContentChange,
  });

  const handleSwitchNote = useCallback(
    async (nextNoteId: string) => {
      if (!nextNoteId || nextNoteId === note?.id) {
        return;
      }

      try {
        flushHistory(); // 刷新历史记录
        await flushSave(currentPersistParams);
        const nextNote = await getNote(nextNoteId);
        if (!nextNote) {
          return;
        }

        setNote(nextNote);
        setDisplayContent(formatNoteForDisplay(nextNote), true); // skipHistory
        clearHistory(); // 清空历史
        resetSaveFeedback();
        await refreshRecentNotes(nextNote.id);
      } catch (error) {
        console.error('Failed to switch note:', error);
      }
    },
    [currentPersistParams, flushSave, note?.id, refreshRecentNotes, resetSaveFeedback, setDisplayContent, clearHistory, flushHistory]
  );

  const handleCreateAndSwitch = useCallback(async () => {
    try {
      flushHistory(); // 刷新历史记录
      await flushSave(currentPersistParams);
      const nextNote = await createNote();
      setNote(nextNote);
      setDisplayContent(formatNoteForDisplay(nextNote), true); // skipHistory
      clearHistory(); // 清空历史
      resetSaveFeedback();
      await refreshRecentNotes(nextNote.id);
    } catch (error) {
      console.error('Failed to create and switch note:', error);
    }
  }, [currentPersistParams, flushSave, refreshRecentNotes, resetSaveFeedback, setDisplayContent, clearHistory, flushHistory]);

  const filteredRecentNotes = useMemo(() => {
    const query = switcherQuery.trim().toLowerCase();
    const uniqueRecentNotes = dedupeNotes(recentNotes);
    if (!query) {
      return uniqueRecentNotes;
    }

    return uniqueRecentNotes.filter((item) => {
      const title = deriveDisplayTitle(item.title, item.content).toLowerCase();
      const content = item.content.toLowerCase();
      const tags = item.tags.toLowerCase();
      return title.includes(query) || content.includes(query) || tags.includes(query);
    });
  }, [recentNotes, switcherQuery]);

  const imageLinks = useMemo(() => extractImageLinks(displayContent), [displayContent]);
  const isGhostMode = opacity <= 0.01;
  const quickHelpItems = [
    `切换笔记 ${shortcutConfig.quickSwitcher}`,
    `切换置顶 ${shortcutConfig.toggleAlwaysOnTop}`,
    '关闭当前 Ctrl+D',
    '透明隐藏 Ctrl+Shift+H',
    '关闭全部 Ctrl+Alt+D',
    '最小化/恢复全部 Ctrl+Alt+H',
    '新建快速笔记 Ctrl+Alt+N',
    '管理中心 Ctrl+Alt+M',
  ];
  useShortcutEngine({
    enabled: Boolean(note),
    textareaRef,
    config: shortcutConfig,
    onSave: async () => {
      await flushSave(currentPersistParams);
    },
    onClose: async () => {
      await closeWindow();
    },
    onChange: applyShortcutContentChange,
  });

  if (!note) {
    return (
      <div style={{ width: '100vw', height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: BG_COLOR }}>
        <div style={{ width: 20, height: 20, border: '2px solid #ccc', borderTopColor: '#666', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
      </div>
    );
  }

  const bgAlpha = opacity;
  const mainBg = `rgba(250, 248, 240, ${bgAlpha})`;
  const dragBg = `rgba(240, 238, 230, ${bgAlpha})`;
  return (
    <div style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, display: 'flex', flexDirection: 'column', backgroundColor: mainBg }}>
      {isSwitcherOpen ? (
        <div
          style={{
            position: 'absolute',
            top: 44,
            left: 18,
            right: 18,
            zIndex: 20,
            borderRadius: 18,
            border: '1px solid rgba(109, 90, 62, 0.16)',
            background: 'rgba(255, 252, 247, 0.94)',
            boxShadow: '0 18px 44px rgba(58, 48, 33, 0.12)',
            backdropFilter: 'blur(20px)',
            padding: 12,
          }}
        >
          <input
            ref={switcherInputRef}
            value={switcherQuery}
            onChange={(event) => setSwitcherQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setSelectedIndex((prev) => Math.min(prev + 1, filteredRecentNotes.length));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setSelectedIndex((prev) => Math.max(prev - 1, 0));
              } else if (event.key === 'Enter') {
                event.preventDefault();
                const targetNote = filteredRecentNotes[selectedIndex - 1];
                if (targetNote) {
                  setIsSwitcherOpen(false);
                  setSwitcherQuery('');
                  setSelectedIndex(0);
                  void handleSwitchNote(targetNote.id);
                } else if (selectedIndex === 0) {
                  setIsSwitcherOpen(false);
                  setSwitcherQuery('');
                  setSelectedIndex(0);
                  void handleCreateAndSwitch();
                }
              }
            }}
            placeholder="切换笔记，输入标题、内容或标签"
            style={{
              width: '100%',
              border: '1px solid rgba(109, 90, 62, 0.12)',
              borderRadius: 14,
              background: 'rgba(255,255,255,0.82)',
              padding: '10px 12px',
              fontSize: 13,
              color: '#4b3d2b',
              outline: 'none',
            }}
          />
          <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 220, overflowY: 'auto' }}>
            <button
              onClick={() => {
                setIsSwitcherOpen(false);
                setSwitcherQuery('');
                setSelectedIndex(0);
                void handleCreateAndSwitch();
              }}
              style={{
                border: selectedIndex === 0 ? '1px solid rgba(129, 151, 111, 0.4)' : '1px solid rgba(109, 90, 62, 0.12)',
                background: selectedIndex === 0 ? 'rgba(231, 239, 226, 0.9)' : 'rgba(246, 240, 228, 0.9)',
                borderRadius: 14,
                padding: '10px 12px',
                textAlign: 'left',
                color: '#6d5a3e',
                cursor: 'pointer',
              }}
            >
              新建笔记
            </button>
            {filteredRecentNotes.map((item, index) => (
              <button
                key={item.id}
                onClick={() => {
                  setIsSwitcherOpen(false);
                  setSwitcherQuery('');
                  setSelectedIndex(0);
                  void handleSwitchNote(item.id);
                }}
                style={{
                  border: selectedIndex === index + 1 ? '1px solid rgba(129, 151, 111, 0.4)' : item.id === note.id ? '1px solid rgba(129, 151, 111, 0.28)' : '1px solid rgba(109, 90, 62, 0.1)',
                  background: selectedIndex === index + 1 ? 'rgba(231, 239, 226, 0.85)' : item.id === note.id ? 'rgba(231, 239, 226, 0.7)' : 'rgba(255,255,255,0.76)',
                  borderRadius: 14,
                  padding: '10px 12px',
                  textAlign: 'left',
                  cursor: 'pointer',
                  color: '#4b3d2b',
                }}
              >
                <div style={{ fontSize: 13 }}>{buildSwitchLabel(item)}</div>
                <div style={{ marginTop: 4, fontSize: 11, color: '#9c8b75' }}>
                  {isContinueCandidate(item) ? '继续编辑' : '空白笔记'}
                </div>
              </button>
            ))}
          </div>
        </div>
      ) : null}
      {/* 拖曳栏 */}
      <div data-tauri-drag-region style={{ position: 'relative', height: 32, display: 'flex', alignItems: 'center', justifyContent: 'space-between', paddingInline: 8, backgroundColor: dragBg, flexShrink: 0 }}>
        <div style={{ paddingInline: 8, fontSize: 10, letterSpacing: '0.22em', color: `rgba(139, 125, 105, ${Math.max(0.26, bgAlpha * 0.72)})` }}>
          {isGhostMode ? 'GHOST' : `${Math.round(opacity * 100)}%`}
        </div>
        <button
          ref={toolsButtonRef}
          onClick={() => setIsToolsOpen((prev) => !prev)}
          title="工具与快捷键"
          style={{
            position: 'absolute',
            left: '50%',
            transform: 'translateX(-50%)',
            width: 20,
            height: 20,
            borderRadius: 999,
            border: 'none',
            background: 'transparent',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: `rgba(109, 90, 62, ${Math.max(0.32, bgAlpha * 0.72)})`,
            cursor: 'pointer',
          }}
        >
          <span style={{ fontSize: 12, lineHeight: 1, marginTop: -2, letterSpacing: '0.06em' }}>•••</span>
        </button>
        <button onClick={handleToggleAlwaysOnTop} style={{ width: 28, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', border: 'none', background: 'none', cursor: 'pointer', color: alwaysOnTop ? '#3b82f6' : '#9ca3af' }} title={alwaysOnTop ? '取消置顶' : '置顶'}>
          <svg width="16" height="16" fill={alwaysOnTop ? 'currentColor' : 'none'} stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
        </button>
      </div>

      {isToolsOpen ? (
        <div
          ref={toolsPanelRef}
          style={{
            position: 'absolute',
            top: 38,
            left: 14,
            right: 14,
            zIndex: 18,
            borderRadius: 18,
            border: '1px solid rgba(109, 90, 62, 0.14)',
            background: 'rgba(255, 251, 245, 0.94)',
            boxShadow: '0 20px 44px rgba(58, 48, 33, 0.14)',
            backdropFilter: 'blur(18px)',
            padding: 12,
          }}
        >
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 8 }}>
            {[
              { label: '切换笔记', action: () => { setIsToolsOpen(false); setIsSwitcherOpen(true); setSwitcherQuery(''); } },
              { label: isGhostMode ? '恢复显示' : '隐藏为 0%', action: () => { setIsToolsOpen(false); toggleGhostMode(); } },
              { label: '关闭当前', action: () => { setIsToolsOpen(false); void closeWindow(); } },
              { label: '关闭全部', action: () => { setIsToolsOpen(false); void closeAllNoteWindows(); } },
              { label: '最小化/恢复全部', action: () => { setIsToolsOpen(false); void toggleNoteWindowsVisibility(); } },
              { label: '新建并切换', action: () => { setIsToolsOpen(false); void handleCreateAndSwitch(); } },
            ].map((item) => (
              <button
                key={item.label}
                onClick={item.action}
                style={{
                  borderRadius: 14,
                  border: '1px solid rgba(109, 90, 62, 0.12)',
                  background: 'rgba(255,255,255,0.74)',
                  padding: '10px 12px',
                  textAlign: 'left',
                  fontSize: 12,
                  color: '#4b3d2b',
                  cursor: 'pointer',
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
          <div style={{ marginTop: 10, borderTop: '1px solid rgba(109, 90, 62, 0.08)', paddingTop: 10, display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            {quickHelpItems.map((item) => (
              <span
                key={item}
                style={{
                  borderRadius: 999,
                  background: 'rgba(246, 240, 228, 0.92)',
                  padding: '4px 8px',
                  fontSize: 11,
                  color: '#7f6d57',
                }}
              >
                {item}
              </span>
            ))}
          </div>
        </div>
      ) : null}

      {/* 编辑器 */}
      {imageInsertion.insertResult ? (
        <div
          style={{
            position: 'absolute',
            top: 40,
            left: 12,
            right: 12,
            zIndex: 25,
            padding: '10px 14px',
            borderRadius: 12,
            background: imageInsertion.insertResult.startsWith('✅') ? 'rgba(76, 175, 80, 0.92)' : imageInsertion.insertResult.startsWith('❌') ? 'rgba(244, 67, 54, 0.92)' : 'rgba(109, 90, 62, 0.88)',
            color: '#fff',
            fontSize: 13,
            boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
          }}
        >
          {imageInsertion.insertResult}
        </div>
      ) : null}
      <div style={{ flex: 1, padding: 8, paddingTop: 6, display: 'flex', flexDirection: 'column', gap: 8 }}>
        {imageLinks.length > 0 ? (
          <div style={{ display: 'flex', gap: 8, overflowX: 'auto', paddingInline: 4, paddingTop: 2 }}>
            {imageLinks.slice(0, 8).map((src, index) => (
              <div
                key={`${src}-${index}`}
                style={{
                  width: 74,
                  height: 54,
                  flexShrink: 0,
                  borderRadius: 12,
                  overflow: 'hidden',
                  border: '1px solid rgba(109, 90, 62, 0.12)',
                  background: 'rgba(255,255,255,0.68)',
                }}
              >
                <img
                  src={src}
                  alt={`插入图片 ${index + 1}`}
                  style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block' }}
                />
              </div>
            ))}
          </div>
        ) : null}
        <EditorTextarea
          textareaRef={textareaRef}
          editorSettings={editorSettings}
          variant="quick-note"
          dragActive={imageInsertion.isDragActive}
          dragHint="拖入图片以插入到快速笔记"
          aria-label="笔记内容"
          value={displayContent}
          onChange={(e) => setDisplayContent(e.target.value)}
          onPaste={imageInsertion.handlePaste}
          onDragEnter={imageInsertion.handleDragEnter}
          onDragLeave={imageInsertion.handleDragLeave}
          onDragOver={imageInsertion.handleDragOver}
          onDrop={imageInsertion.handleDrop}
          placeholder=""
          autoFocus
        />
      </div>
    </div>
  );
}
