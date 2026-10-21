/**
 * QuickNotePage - 快速笔记页面
 * 只负责窗口特有交互（透明度 / 置顶 / 关闭 / 切换器 / 快捷键）与渲染。
 * 编辑会话、笔记切换、备份与附件操作归 useQuickNoteSession（Phase C 拆分）；
 * 快捷键映射归 useQuickNoteShortcuts，切换器 / 工具栏 / 图片条为共享展示组件。
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { closeAllNoteWindows, toggleNoteWindowsVisibility } from '../lib/tauri/notes';
import { closeCurrentWindow, toggleWindowAlwaysOnTop } from '../lib/tauri/window';
import { EditorTextarea } from '../components/shared/EditorTextarea';
import { ImageLightbox } from '../components/shared/ImageLightbox';
import { BackupRestoreDialog } from '../components/shared/BackupRestoreDialog';
import { NoteSwitcherPanel } from '../components/shared/NoteSwitcherPanel';
import { QuickNoteToolbar } from '../components/shared/QuickNoteToolbar';
import { QuickNoteToolsPanel } from '../components/shared/QuickNoteToolsPanel';
import { QuickNoteImageStrip } from '../components/shared/QuickNoteImageStrip';
import { useShortcutEngine } from '../hooks/useShortcutEngine';
import { useEditorPreferences } from '../hooks/useEditorPreferences';
import { useQuickNoteShortcuts } from '../hooks/useQuickNoteShortcuts';
import { useQuickNoteSession } from '../hooks/useQuickNoteSession';
import { focusTextareaAtEnd } from '../lib/editorSelection';
import { applyMarkdownCompletion } from '../lib/markdownEditing';
import { filterNotesByQuery } from '../lib/notesUtils';

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
        await closeCurrentWindow();
      }
    } catch (error) {
      console.error('Failed to close window:', error);
      isClosingRef.current = false;
    }
  }, [flushPendingSave]);

  /** Leave the switcher and clear its transient state. */
  const closeSwitcher = useCallback(() => {
    setIsSwitcherOpen(false);
    setSwitcherQuery('');
    setSelectedIndex(0);
  }, []);

  const toggleSwitcher = useCallback(() => {
    setSwitcherQuery('');
    setIsSwitcherOpen((prev) => !prev);
    setIsToolsOpen(false);
  }, []);

  const switchToNote = useCallback(
    (id: string) => {
      closeSwitcher();
      void switchNote(id);
    },
    [closeSwitcher, switchNote],
  );

  const createAndSwitchNote = useCallback(() => {
    closeSwitcher();
    void createAndSwitch();
  }, [closeSwitcher, createAndSwitch]);

  const toggleAlwaysOnTop = useCallback(async () => {
    try {
      setAlwaysOnTop(await toggleWindowAlwaysOnTop());
    } catch (error) {
      console.error('Failed to toggle always on top:', error);
    }
  }, []);

  const toggleTools = useCallback(() => {
    setIsToolsOpen((prev) => !prev);
  }, []);

  const runTool = useCallback((action: () => void) => {
    setIsToolsOpen(false);
    action();
  }, []);

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

  // 窗口快捷键映射（撤销 / 重做、切换器、置顶、关闭、透明与工具面板）
  useQuickNoteShortcuts({
    canUndo,
    canRedo,
    isSwitcherOpen,
    shortcutConfig,
    onUndo: undo,
    onRedo: redo,
    onClose: () => {
      void closeWindow();
    },
    onToggleAlwaysOnTop: () => {
      void toggleAlwaysOnTop();
    },
    onToggleGhost: toggleGhostMode,
    onOpenSwitcher: toggleSwitcher,
    onCloseSwitcher: closeSwitcher,
    onToggleTools: toggleTools,
  });

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

  const filteredRecentNotes = useMemo(
    () => filterNotesByQuery(recentNotes, switcherQuery),
    [recentNotes, switcherQuery]
  );

  const isGhostMode = opacity <= 0.01;
  const quickHelpItems = useMemo(
    () => [
      `切换笔记 ${shortcutConfig.quickSwitcher}`,
      `切换置顶 ${shortcutConfig.toggleAlwaysOnTop}`,
      '关闭当前 Ctrl+D',
      '透明隐藏 Ctrl+Shift+H',
      '关闭全部 Ctrl+Alt+D',
      '最小化/恢复全部 Ctrl+Alt+H',
      '新建快速笔记 Ctrl+Alt+N',
      '管理中心 Ctrl+Alt+M',
    ],
    [shortcutConfig.quickSwitcher, shortcutConfig.toggleAlwaysOnTop],
  );
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
        <NoteSwitcherPanel
          query={switcherQuery}
          onQueryChange={setSwitcherQuery}
          selectedIndex={selectedIndex}
          onSelectIndex={setSelectedIndex}
          notes={filteredRecentNotes}
          currentNoteId={note.id}
          inputRef={switcherInputRef}
          onCreateAndSwitch={createAndSwitchNote}
          onSwitchNote={switchToNote}
        />
      ) : null}

      <QuickNoteToolbar
        opacity={opacity}
        dragBackground={dragBg}
        alwaysOnTop={alwaysOnTop}
        toolsButtonRef={toolsButtonRef}
        onToggleTools={toggleTools}
        onToggleAlwaysOnTop={() => {
          void toggleAlwaysOnTop();
        }}
      />

      {isToolsOpen ? (
        <QuickNoteToolsPanel
          panelRef={toolsPanelRef}
          isGhostMode={isGhostMode}
          helpItems={quickHelpItems}
          onSwitchNote={() => runTool(toggleSwitcher)}
          onToggleGhost={() => runTool(toggleGhostMode)}
          onCloseCurrent={() => runTool(() => void closeWindow())}
          onCloseAll={() => runTool(() => void closeAllNoteWindows())}
          onToggleAllVisibility={() => runTool(() => void toggleNoteWindowsVisibility())}
          onNewAndSwitch={() => runTool(createAndSwitchNote)}
        />
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
          <QuickNoteImageStrip
            images={resolvedImageLinks}
            filenames={attachedImages.map((attachment) => attachment.filename)}
            onOpen={setActiveImage}
            onRemove={removeAttachedImage}
          />
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
