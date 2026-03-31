import { useState, useCallback, type ClipboardEventHandler, type DragEventHandler, type RefObject } from 'react';
import type { Note } from '../../../types/note';
import type { ShortcutConfig } from '../../../types/shortcut';
import type { EditorSettings } from '../../../types/settings';
import type { BackupDraft } from './types';
import { BackupRestoreDialog } from '../../shared/BackupRestoreDialog';
import { EditorTextarea } from '../../shared/EditorTextarea';
import { StatusBar } from './StatusBar';
import { deriveDisplayTitle } from '../../../lib/noteParser';
import { MarkdownPreview } from './MarkdownPreview';

type EditorMode = 'edit' | 'preview';

interface NoteEditorProps {
  note: Note;
  draftTitle: string;
  draftContent: string;
  draftTags: string;
  allTags: Array<[string, number]>;
  saveStateLabel: string;
  saveError: string;
  backupDraft: BackupDraft | null;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  shortcutConfig: ShortcutConfig;
  editorSettings: EditorSettings;
  splitTags: (tagString: string) => string[];
  imageDragActive?: boolean;
  onPasteImage?: ClipboardEventHandler<HTMLTextAreaElement>;
  onDragEnterImage?: DragEventHandler<HTMLTextAreaElement>;
  onDragLeaveImage?: DragEventHandler<HTMLTextAreaElement>;
  onDragOverImage?: DragEventHandler<HTMLTextAreaElement>;
  onDropImage?: DragEventHandler<HTMLTextAreaElement>;
  onClose: () => void | Promise<void>;
  onTogglePin: (event: React.MouseEvent<HTMLButtonElement>) => void | Promise<void>;
  onTitleChange: (value: string) => void;
  onContentChange: (value: string) => void;
  onTagsChange: (value: string) => void;
  onTagRemove: (tag: string) => void;
  onKeepCurrentDraft: () => void;
  onRestoreBackupDraft: () => void;
  onMergeBackupDraft: () => void;
}

export function NoteEditor(props: NoteEditorProps) {
  const {
    note,
    draftTitle,
    draftContent,
    draftTags,
    allTags,
    saveStateLabel,
    saveError,
    backupDraft,
    textareaRef,
    shortcutConfig,
    editorSettings,
    splitTags,
    imageDragActive,
    onPasteImage,
    onDragEnterImage,
    onDragLeaveImage,
    onDragOverImage,
    onDropImage,
    onClose,
    onTogglePin,
    onTitleChange,
    onContentChange,
    onTagsChange,
    onTagRemove,
    onKeepCurrentDraft,
    onRestoreBackupDraft,
    onMergeBackupDraft,
  } = props;

  const [mode, setMode] = useState<EditorMode>('edit');
  const derivedTitle = deriveDisplayTitle(draftTitle, draftContent);

  const handleContentChangeWithAutoComplete = useCallback(
    (value: string) => {
      onContentChange(value);
    },
    [onContentChange]
  );

  return (
    <section className="relative flex h-full flex-col">
      {backupDraft ? (
        <BackupRestoreDialog
          currentTitle={draftTitle}
          currentContent={draftContent}
          currentTags={draftTags}
          backupTitle={backupDraft.title}
          backupContent={backupDraft.content}
          backupTags={backupDraft.tags}
          backupTimestamp={backupDraft.timestamp}
          onKeepCurrent={onKeepCurrentDraft}
          onRestoreBackup={onRestoreBackupDraft}
          onMergeWithBackup={onMergeBackupDraft}
        />
      ) : null}

      <header className="flex items-center justify-between gap-4 border-b border-[color:var(--manager-border)] px-5 py-4 sm:px-7">
        <div className="flex min-w-0 items-center gap-3">
          <button
            onClick={() => void onClose()}
            className="flex h-11 w-11 items-center justify-center rounded-full border border-[color:var(--manager-border)] bg-white/70 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
            aria-label="返回"
          >
            ←
          </button>
          <div className="min-w-0">
            <p className="manager-kicker">Zen Editor</p>
            <input
              value={draftTitle}
              onChange={(event) => onTitleChange(event.target.value)}
              placeholder={derivedTitle || '无标题'}
              className="w-full truncate border-none bg-transparent font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-2xl text-[color:var(--manager-ink-strong)] outline-none placeholder:text-[color:var(--manager-ink-subtle)]"
            />
          </div>
        </div>
        <div className="flex items-center gap-3">
          {/* Edit / Preview toggle */}
          <div className="flex rounded-full border border-[color:var(--manager-border)] p-0.5">
            <button
              onClick={() => setMode('edit')}
              className={`rounded-full px-3 py-1 text-xs transition ${
                mode === 'edit'
                  ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'text-[color:var(--manager-ink-soft)] hover:text-[color:var(--manager-ink)]'
              }`}
            >
              编辑
            </button>
            <button
              onClick={() => setMode('preview')}
              className={`rounded-full px-3 py-1 text-xs transition ${
                mode === 'preview'
                  ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'text-[color:var(--manager-ink-soft)] hover:text-[color:var(--manager-ink)]'
              }`}
            >
              预览
            </button>
          </div>
          <span className="hidden text-sm text-[color:var(--manager-ink-soft)] sm:inline">
            保存 {shortcutConfig.save} / 退出 {shortcutConfig.close}
          </span>
          <button
            onClick={(event) => void onTogglePin(event)}
            className={`rounded-full border px-4 py-2 text-sm transition ${
              note.is_pinned
                ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)]'
            }`}
          >
            {note.is_pinned ? '已置顶' : '置顶'}
          </button>
        </div>
      </header>

      <div className="flex-1 overflow-auto px-4 py-4 sm:px-6 sm:py-5">
        {mode === 'edit' ? (
          <EditorTextarea
            textareaRef={textareaRef}
            variant="manager"
            editorSettings={editorSettings}
            dragActive={imageDragActive}
            dragHint="拖入图片到编辑器以插入"
            value={draftContent}
            onChange={(event) => handleContentChangeWithAutoComplete(event.target.value)}
            onKeyDown={(event) => {
              const textarea = event.currentTarget;
              const { selectionStart, selectionEnd, value } = textarea;

              // Enter key: list auto-continuation
              if (event.key === 'Enter' && !event.shiftKey && !event.ctrlKey && !event.metaKey) {
                const lineStart = value.lastIndexOf('\n', selectionStart - 1) + 1;
                const currentLine = value.slice(lineStart, selectionStart);

                // Match todo checkbox: "- [ ] text" or "- [x] text" → continue with "- [ ] "
                const todoMatch = /^(\s*)([-*])\s*\[[ xX]\]\s*(.*)$/.exec(currentLine);
                if (todoMatch) {
                  const [, indent, bullet, text] = todoMatch;
                  // If the line is empty (just the bullet), clear it and stop list
                  if (!text.trim()) {
                    event.preventDefault();
                    const before = value.slice(0, lineStart);
                    const after = value.slice(selectionStart);
                    onContentChange(before + '\n' + after);
                    requestAnimationFrame(() => {
                      const pos = before.length + 1;
                      textarea.selectionStart = pos;
                      textarea.selectionEnd = pos;
                    });
                    return;
                  }
                  // Continue list with new checkbox
                  event.preventDefault();
                  const before = value.slice(0, selectionStart);
                  const after = value.slice(selectionStart);
                  const newLine = `\n${indent}${bullet} [ ] `;
                  onContentChange(before + newLine + after);
                  requestAnimationFrame(() => {
                    const pos = before.length + newLine.length;
                    textarea.selectionStart = pos;
                    textarea.selectionEnd = pos;
                  });
                  return;
                }

                // Match unordered list: "- text" or "* text" → continue with "- "
                const ulMatch = /^(\s*)([-*])\s+(.*)$/.exec(currentLine);
                if (ulMatch) {
                  const [, indent, bullet, text] = ulMatch;
                  if (!text.trim()) {
                    event.preventDefault();
                    const before = value.slice(0, lineStart);
                    const after = value.slice(selectionStart);
                    onContentChange(before + '\n' + after);
                    requestAnimationFrame(() => {
                      const pos = before.length + 1;
                      textarea.selectionStart = pos;
                      textarea.selectionEnd = pos;
                    });
                    return;
                  }
                  event.preventDefault();
                  const before = value.slice(0, selectionStart);
                  const after = value.slice(selectionStart);
                  const newLine = `\n${indent}${bullet} `;
                  onContentChange(before + newLine + after);
                  requestAnimationFrame(() => {
                    const pos = before.length + newLine.length;
                    textarea.selectionStart = pos;
                    textarea.selectionEnd = pos;
                  });
                  return;
                }

                // Match ordered list: "1. text" → continue with "2. "
                const olMatch = /^(\s*)(\d+)\.\s+(.*)$/.exec(currentLine);
                if (olMatch) {
                  const [, indent, numStr, text] = olMatch;
                  if (!text.trim()) {
                    event.preventDefault();
                    const before = value.slice(0, lineStart);
                    const after = value.slice(selectionStart);
                    onContentChange(before + '\n' + after);
                    requestAnimationFrame(() => {
                      const pos = before.length + 1;
                      textarea.selectionStart = pos;
                      textarea.selectionEnd = pos;
                    });
                    return;
                  }
                  event.preventDefault();
                  const before = value.slice(0, selectionStart);
                  const after = value.slice(selectionStart);
                  const nextNum = parseInt(numStr, 10) + 1;
                  const newLine = `\n${indent}${nextNum}. `;
                  onContentChange(before + newLine + after);
                  requestAnimationFrame(() => {
                    const pos = before.length + newLine.length;
                    textarea.selectionStart = pos;
                    textarea.selectionEnd = pos;
                  });
                  return;
                }
              }

              // Bracket auto-completion
              const pairs: Record<string, string> = {
                '[': ']',
                '(': ')',
                '{': '}',
                '`': '`',
              };

              const openChar = event.key;
              if (pairs[openChar]) {
                event.preventDefault();
                const closeChar = pairs[openChar];
                const before = value.slice(0, selectionStart);
                const selected = value.slice(selectionStart, selectionEnd);
                const after = value.slice(selectionEnd);

                // Special: typing `- [` at line start → `- [ ] `
                const lineStart = value.lastIndexOf('\n', selectionStart - 1) + 1;
                const beforeCursorOnLine = value.slice(lineStart, selectionStart);
                if (openChar === '[' && /^(\s*)(-|\*)\s?$/.test(beforeCursorOnLine)) {
                  const newContent = before + '[ ] ' + after;
                  onContentChange(newContent);
                  // Set cursor after the space
                  requestAnimationFrame(() => {
                    textarea.selectionStart = selectionStart + 4;
                    textarea.selectionEnd = selectionStart + 4;
                  });
                  return;
                }

                const newContent = before + openChar + selected + closeChar + after;
                onContentChange(newContent);
                // Place cursor between brackets (or after selection)
                const cursorPos = selectionStart + 1 + selected.length;
                requestAnimationFrame(() => {
                  textarea.selectionStart = selectionStart + 1;
                  textarea.selectionEnd = cursorPos;
                });
                return;
              }

              // Auto-skip closing bracket
              if (event.key === ']' || event.key === ')' || event.key === '}') {
                if (selectionStart === selectionEnd && value[selectionStart] === event.key) {
                  event.preventDefault();
                  textarea.selectionStart = selectionStart + 1;
                  textarea.selectionEnd = selectionStart + 1;
                  return;
                }
              }
            }}
            onPaste={onPasteImage}
            onDragEnter={onDragEnterImage}
            onDragLeave={onDragLeaveImage}
            onDragOver={onDragOverImage}
            onDrop={onDropImage}
            placeholder="让内容安静地展开。Esc 返回，Ctrl+S 立即保存。"
          />
        ) : (
          <MarkdownPreview
            content={draftContent}
            onToggleCheckbox={(lineIndex) => {
              const lines = draftContent.split('\n');
              const line = lines[lineIndex];
              if (!line) return;
              const todoMatch = /^(\s*)([-*])\s*\[([ xX])\]\s*(.+)$/.exec(line);
              if (!todoMatch) return;
              const [, indent, bullet, checked, text] = todoMatch;
              const newChecked = checked.toLowerCase() === 'x' ? ' ' : 'x';
              lines[lineIndex] = `${indent}${bullet} [${newChecked}] ${text}`;
              onContentChange(lines.join('\n'));
            }}
          />
        )}
      </div>

      <StatusBar
        draftTags={draftTags}
        allTags={allTags}
        splitTags={splitTags}
        onTagRemove={onTagRemove}
        onTagsChange={onTagsChange}
        saveStateLabel={saveStateLabel}
        contentLength={draftContent.length}
        saveError={saveError}
      />
    </section>
  );
}
