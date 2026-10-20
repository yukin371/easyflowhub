/**
 * QuickNotePage - 快速笔记页面
 * 只负责窗口特有交互（透明度 / 置顶 / 关闭 / 切换器 / 快捷键）与渲染。
 * 编辑会话、笔记切换、备份与附件操作归 useQuickNoteSession（Phase C 拆分）。
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { closeAllNoteWindows, toggleNoteWindowsVisibility } from '../lib/tauri/notes';
import { EditorTextarea } from '../components/shared/EditorTextarea';
import { ImageLightbox } from '../components/shared/ImageLightbox';
import { BackupRestoreDialog } from '../components/shared/BackupRestoreDialog';
import { useShortcutEngine } from '../hooks/useShortcutEngine';
import { useEditorPreferences } from '../hooks/useEditorPreferences';
import { useQuickNoteSession } from '../hooks/useQuickNoteSession';
import { matchesShortcutEvent } from '../types/shortcut';
import { focusTextareaAtEnd } from '../lib/editorSelection';
import { applyMarkdownCompletion } from '../lib/markdownEditing';
import {
  buildNoteSwitchLabel,
  filterNotesByQuery,
  isNoteContinueCandidate,
} from '../lib/notesUtils';

const BG_COLOR = 'var(--quick-bg)';

export function QuickNotePage() {
  const [alwaysOnTop, setAlwaysOnTop] = useState(true);
  const [opacity, setOpacity] = useState(1.0);
  const [lastVisibleOpacity, setLastVisibleOpacity] = useState(1);
  const [isSwitcherOpen, setIsSwitcherOpen] = useState(false);
  const [switcherQuery, setSwitcherQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isToolsOpen, setIsToolsOpen] = useState(false);
  const [activeImage, setActiveImage] = useState<{ src: string; alt: string } | null>(null);

  const switcherInputRef = useRef<HTMLInputElement>(null);
  const toolsButtonRef = useRef<HTMLButtonElement>(null);
  const toolsPanelRef = useRef<HTMLDivElement>(null);
  const lastWheelTime = useRef(0);
  const isClosingRef = useRef(false);

  const { shortcutConfig, editorSettings } = useEditorPreferences();
  const session = useQuickNoteSession({ maxUndoSteps: editorSettings.undo_steps });
  const {
    note,
    recentNotes,
    displayContent,
    setDisplayContent,
    undo,
    redo,
    canUndo,
    canRedo,
    textareaRef,
    attachedImages,
    resolvedImageLinks,
    removeAttachedImage,
    mergedEditorContent,
    currentDraftTags,
    backupDraft,
    clearBackupDraft,
    restoreBackupDraft,
    mergeBackupDraft,
    imageInsertion,
    applyShortcutContentChange,
    switchNote,
    createAndSwitch,
    resetSaveFeedback,
    flushSaveNow,
    flushPendingSave,
  } = session;

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

    try {
      await Promise.race([
        flushPendingSave(),
        new Promise<null>((resolve) => window.setTimeout(() => resolve(null), 350)),
      ]);

      try {
        await getCurrentWindow().destroy();
      } catch (primaryError) {
        console.error('Primary quick note destroy failed, falling back to backend close:', primaryError);
        await invoke('close_window');
      }
    } catch (error) {
      console.error('Failed to close window:', error);
      isClosingRef.current = false;
    }
  }, [flushPendingSave]);

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
      if (!document.hasFocus()) {
        return;
      }

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

  const filteredRecentNotes = useMemo(
    () => filterNotesByQuery(recentNotes, switcherQuery),
    [recentNotes, switcherQuery]
  );

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
    onSave: flushSaveNow,
    onClose: async () => {
      await closeWindow();
    },
    onChange: applyShortcutContentChange,
  });

  if (!note) {
    return (
      <div style={{ width: '100vw', height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: BG_COLOR }}>
        <div style={{ width: 20, height: 20, border: '2px solid var(--quick-border)', borderTopColor: 'var(--quick-accent)', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
      </div>
    );
  }

  const bgAlpha = opacity;
  const mainBg = `color-mix(in srgb, var(--quick-bg) ${Math.round(bgAlpha * 100)}%, transparent)`;
  const dragBg = `color-mix(in srgb, var(--quick-surface-raised) ${Math.round(bgAlpha * 100)}%, transparent)`;
  return (
    <div style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, display: 'flex', flexDirection: 'column', backgroundColor: mainBg }}>
      {backupDraft ? (
        <BackupRestoreDialog
          currentTitle={note.title}
          currentContent={mergedEditorContent}
          currentTags={currentDraftTags}
          backupTitle={backupDraft.title}
          backupContent={backupDraft.content}
          backupTags={backupDraft.tags}
          backupTimestamp={backupDraft.timestamp}
          onKeepCurrent={clearBackupDraft}
          onRestoreBackup={restoreBackupDraft}
          onMergeWithBackup={mergeBackupDraft}
        />
      ) : null}

      {isSwitcherOpen ? (
        <div
          style={{
            position: 'absolute',
            top: 44,
            left: 18,
            right: 18,
            zIndex: 20,
            borderRadius: 18,
            border: '1px solid var(--quick-border)',
            background: 'var(--quick-surface)',
            boxShadow: 'var(--quick-shadow)',
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
                  void switchNote(targetNote.id);
                } else if (selectedIndex === 0) {
                  setIsSwitcherOpen(false);
                  setSwitcherQuery('');
                  setSelectedIndex(0);
                  void createAndSwitch();
                }
              }
            }}
            placeholder="切换笔记，输入标题、内容或标签"
            style={{
              width: '100%',
              border: '1px solid var(--quick-border)',
              borderRadius: 14,
              background: 'var(--quick-input)',
              padding: '10px 12px',
              fontSize: 13,
              color: 'var(--quick-text)',
              outline: 'none',
            }}
          />
          <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 220, overflowY: 'auto' }}>
            <button
              onClick={() => {
                setIsSwitcherOpen(false);
                setSwitcherQuery('');
                setSelectedIndex(0);
                void createAndSwitch();
              }}
              style={{
                border: selectedIndex === 0 ? '1px solid var(--quick-accent)' : '1px solid var(--quick-border)',
                background: selectedIndex === 0 ? 'var(--quick-accent-soft)' : 'var(--quick-surface-raised)',
                borderRadius: 14,
                padding: '10px 12px',
                textAlign: 'left',
                color: 'var(--quick-text)',
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
                  void switchNote(item.id);
                }}
                style={{
                  border: selectedIndex === index + 1 ? '1px solid var(--quick-accent)' : item.id === note.id ? '1px solid var(--quick-accent)' : '1px solid var(--quick-border)',
                  background: selectedIndex === index + 1 ? 'var(--quick-accent-soft)' : item.id === note.id ? 'var(--quick-accent-soft)' : 'var(--quick-surface-raised)',
                  borderRadius: 14,
                  padding: '10px 12px',
                  textAlign: 'left',
                  cursor: 'pointer',
                  color: 'var(--quick-text)',
                }}
              >
                <div style={{ fontSize: 13 }}>{buildNoteSwitchLabel(item)}</div>
                <div style={{ marginTop: 4, fontSize: 11, color: 'var(--quick-muted)' }}>
                  {isNoteContinueCandidate(item) ? '继续编辑' : '空白笔记'}
                </div>
              </button>
            ))}
          </div>
        </div>
      ) : null}
      {/* 拖曳栏 */}
      <div data-tauri-drag-region style={{ position: 'relative', height: 32, display: 'flex', alignItems: 'center', justifyContent: 'space-between', paddingInline: 8, backgroundColor: dragBg, flexShrink: 0 }}>
        <div style={{ paddingInline: 8, fontSize: 10, letterSpacing: '0.22em', color: `color-mix(in srgb, var(--quick-muted) ${Math.round(Math.max(0.26, bgAlpha * 0.72) * 100)}%, transparent)` }}>
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
            color: `color-mix(in srgb, var(--quick-text) ${Math.round(Math.max(0.32, bgAlpha * 0.72) * 100)}%, transparent)`,
            cursor: 'pointer',
          }}
        >
          <span style={{ fontSize: 12, lineHeight: 1, marginTop: -2, letterSpacing: '0.06em' }}>•••</span>
        </button>
        <button
          onClick={handleToggleAlwaysOnTop}
          style={{ width: 28, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', border: 'none', background: 'none', cursor: 'pointer', color: alwaysOnTop ? 'var(--quick-accent)' : 'var(--quick-muted)' }}
          title={alwaysOnTop ? '取消置顶' : '置顶'}
        >
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
            border: '1px solid var(--quick-border)',
            background: 'var(--quick-surface)',
            boxShadow: 'var(--quick-shadow)',
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
              { label: '新建并切换', action: () => { setIsToolsOpen(false); void createAndSwitch(); } },
            ].map((item) => (
              <button
                key={item.label}
                onClick={item.action}
                style={{
                  borderRadius: 14,
                  border: '1px solid var(--quick-border)',
                  background: 'var(--quick-surface-raised)',
                  padding: '10px 12px',
                  textAlign: 'left',
                  fontSize: 12,
                  color: 'var(--quick-text)',
                  cursor: 'pointer',
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
          <div style={{ marginTop: 10, borderTop: '1px solid var(--quick-border)', paddingTop: 10, display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            {quickHelpItems.map((item) => (
              <span
                key={item}
                style={{
                  borderRadius: 999,
                  background: 'var(--quick-accent-soft)',
                  padding: '4px 8px',
                  fontSize: 11,
                  color: 'var(--quick-muted)',
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
            background: imageInsertion.insertResult.startsWith('✅') ? 'rgba(50, 166, 112, 0.92)' : imageInsertion.insertResult.startsWith('❌') ? 'rgba(214, 76, 91, 0.92)' : 'var(--quick-accent)',
            color: '#fff',
            fontSize: 13,
            boxShadow: 'var(--quick-shadow)',
          }}
        >
          {imageInsertion.insertResult}
        </div>
      ) : null}
      <div style={{ flex: 1, minHeight: 0, padding: 8, paddingTop: 6, display: 'flex', flexDirection: 'column', gap: 8 }}>
        {resolvedImageLinks.length > 0 ? (
          <div
            style={{
              flex: '0 0 auto',
              display: 'flex',
              gap: 8,
              overflowX: 'auto',
              paddingInline: 4,
              paddingBottom: 2,
            }}
          >
            {resolvedImageLinks.map(({ src, alt }, index) => (
              <div
                key={attachedImages[index]?.filename ?? `${src}-${index}`}
                style={{
                  position: 'relative',
                  width: 72,
                  minWidth: 72,
                  height: 72,
                  borderRadius: 14,
                  overflow: 'hidden',
                  border: '1px solid var(--quick-border)',
                  background: 'var(--quick-surface-raised)',
                  boxShadow: '0 6px 18px color-mix(in srgb, var(--quick-text) 12%, transparent)',
                }}
              >
                <img
                  src={src}
                  alt={alt || `插入图片 ${index + 1}`}
                  title="双击放大"
                  onDoubleClick={() => setActiveImage({ src, alt })}
                  style={{ width: '100%', height: '100%', objectFit: 'cover', display: 'block', cursor: 'zoom-in' }}
                />
                <button
                  onClick={() => removeAttachedImage(index)}
                  title="移除图片"
                  style={{
                    position: 'absolute',
                    right: 4,
                    top: 4,
                    width: 18,
                    height: 18,
                    borderRadius: 999,
                    border: 'none',
                    background: 'var(--quick-surface)',
                    color: 'var(--quick-text)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    cursor: 'pointer',
                    backdropFilter: 'blur(6px)',
                  }}
                >
                  <span style={{ fontSize: 11, lineHeight: 1 }}>×</span>
                </button>
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
          onKeyDown={(event) => {
            applyMarkdownCompletion({
              event,
              value: displayContent,
              onChange: setDisplayContent,
            });
          }}
          onPaste={imageInsertion.handlePaste}
          placeholder=""
          autoFocus
        />
      </div>
      {activeImage ? (
        <ImageLightbox src={activeImage.src} alt={activeImage.alt} onClose={() => setActiveImage(null)} />
      ) : null}
    </div>
  );
}
