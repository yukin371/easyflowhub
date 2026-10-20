import { useCallback, useEffect, useMemo, useState, type ClipboardEvent, type FormEvent } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { archiveInboxItem, captureInboxItem, convertInboxItemToNote, deleteInboxItem, listInboxItems, openInboxAssetsDir, openInboxAttachment, openInboxLink, restoreInboxItem } from '../../../lib/tauri/inbox';
import { TranscribeControl } from '../shared/TranscribeControl';
import type { InboxItem } from '../../../types/inbox';

type InboxViewMode = 'inbox' | 'archived';
const MAX_IMAGE_BYTES = 32 * 1024 * 1024;

function formatCaptureTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'short', timeStyle: 'short' }).format(date);
}

function readDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ''));
    reader.onerror = () => reject(reader.error ?? new Error('读取剪贴板图片失败。'));
    reader.readAsDataURL(file);
  });
}

function itemKindLabel(item: InboxItem): string {
  if (item.kind === 'link') return '链接';
  if (item.kind === 'image') return '图片';
  if (item.kind === 'file') return '文件';
  if (item.kind === 'audio') return '录音';
  return '文本';
}

export function InboxPanel() {
  const [items, setItems] = useState<InboxItem[]>([]);
  const [viewMode, setViewMode] = useState<InboxViewMode>('inbox');
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [filePaths, setFilePaths] = useState<string[]>([]);
  const [imageDataUrl, setImageDataUrl] = useState<string>();
  const [imageName, setImageName] = useState('');
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState<string>();
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      setItems(await listInboxItems(true));
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    try {
      void getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type !== 'drop') return;
        const paths = event.payload.paths;
        if (paths.length > 0) {
          setFilePaths((current) => Array.from(new Set([...current, ...paths])));
          setStatus(`${paths.length} 个文件已添加`);
        }
      }).then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      }).catch(() => {
        // Browser preview does not expose native file-drop paths.
      });
    } catch {
      // Browser preview does not expose native file-drop paths.
    }
    return () => { disposed = true; unlisten?.(); };
  }, []);

  const visibleItems = useMemo(() => {
    const normalizedSearch = search.trim().toLocaleLowerCase();
    return items.filter((item) => {
      if (Boolean(item.archivedAt) !== (viewMode === 'archived')) return false;
      if (!normalizedSearch) return true;
      return [item.title, item.content, item.source, ...item.attachments.map((attachment) => attachment.name)]
        .join(' ').toLocaleLowerCase().includes(normalizedSearch);
    });
  }, [items, search, viewMode]);

  const handlePaste = async (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const image = Array.from(event.clipboardData.files).find((file) => file.type.startsWith('image/'));
    if (!image) return;
    event.preventDefault();
    if (image.size === 0 || image.size > MAX_IMAGE_BYTES) {
      setStatus(undefined);
      setError(image.size === 0 ? '剪贴板图片为空。' : '图片超过 32 MB 限制。');
      return;
    }
    try {
      setImageDataUrl(await readDataUrl(image));
      setImageName(image.name || '剪贴板图片');
      setError(undefined);
      setStatus('图片已附加');
    } catch (caught) {
      setStatus(undefined);
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const handlePickFiles = async () => {
    try {
      const selected = await open({ multiple: true, directory: false });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      setFilePaths((current) => Array.from(new Set([...current, ...paths])));
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const handleCapture = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsSaving(true);
    setError(undefined);
    setStatus(undefined);
    try {
      await captureInboxItem({
        title,
        content,
        source: 'manager',
        context: 'manual-capture',
        imageDataUrl,
        filePaths,
      });
      setTitle('');
      setContent('');
      setImageDataUrl(undefined);
      setImageName('');
      setFilePaths([]);
      setViewMode('inbox');
      setStatus('已保存到收集箱');
      await refresh();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setIsSaving(false);
    }
  };

  const updateItemState = async (item: InboxItem) => {
    try {
      if (item.archivedAt) await restoreInboxItem(item.id);
      else await archiveInboxItem(item.id);
      await refresh();
      setStatus(item.archivedAt ? '已移回收集箱' : '已归档');
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const convertToNote = async (item: InboxItem) => {
    if (item.convertedNoteId) return;
    try {
      await convertInboxItemToNote(item.id);
      await refresh();
      setStatus('已整理为笔记，可在“笔记”页面查看');
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const handleDelete = async (item: InboxItem) => {
    const label = item.title || item.content || itemKindLabel(item);
    if (!window.confirm(`将“${label}”移到回收站？之后可以从回收站恢复。`)) return;
    try {
      await deleteInboxItem(item.id);
      await refresh();
      setStatus('已移到回收站');
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const handleOpenAssetsDir = async () => {
    try {
      await openInboxAssetsDir();
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  return (
    <section className="flex h-full min-h-0 flex-col text-[color:var(--manager-ink)]">
      <header className="shrink-0 border-b border-[color:var(--manager-border)] px-3 py-3 sm:px-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2" role="tablist" aria-label="收集箱状态">
            {(['inbox', 'archived'] as const).map((mode) => (
              <button
                key={mode}
                type="button"
                role="tab"
                aria-selected={viewMode === mode}
                onClick={() => setViewMode(mode)}
                className={`rounded-[8px] border px-3 py-1.5 text-sm transition ${viewMode === mode
                  ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'border-transparent text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel)]'}`}
              >
                {mode === 'inbox' ? '收集箱' : '已归档'}
              </button>
            ))}
          </div>
          <input
            type="search"
            aria-label="搜索收集箱"
            placeholder="搜索内容、来源或附件"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            className="min-w-[180px] flex-1 rounded-[8px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-3 py-2 text-sm outline-none placeholder:text-[color:var(--manager-ink-subtle)] focus:border-[color:var(--manager-accent)] sm:max-w-[320px]"
          />
          <button
            type="button"
            onClick={() => void handleOpenAssetsDir()}
            title="打开录音、图片与附件所在的资源文件夹"
            className="rounded-[8px] border border-[color:var(--manager-border)] px-3 py-2 text-xs text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel)]"
          >
            打开资源文件夹
          </button>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-3 sm:px-4">
        {status ? <p role="status" className="mb-2 text-xs text-[color:var(--manager-ink-soft)]">{status}</p> : null}
        {error ? <p role="alert" className="mb-2 text-xs text-[color:var(--manager-danger)]">{error}</p> : null}
        {viewMode === 'inbox' ? (
          <form onSubmit={(event) => void handleCapture(event)} className="mb-4 border-b border-[color:var(--manager-border)] pb-4">
            <input
              aria-label="捕获标题"
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="标题（可选）"
              className="mb-2 w-full border-0 bg-transparent px-0 py-1 text-sm font-medium text-[color:var(--manager-ink-strong)] outline-none placeholder:text-[color:var(--manager-ink-subtle)]"
            />
            <textarea
              aria-label="捕获内容"
              value={content}
              onChange={(event) => setContent(event.target.value)}
              onPaste={(event) => void handlePaste(event)}
              placeholder="粘贴文字、链接或截图，也可以添加文件"
              rows={3}
              className="w-full resize-y rounded-[8px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-3 py-2 text-sm leading-6 outline-none placeholder:text-[color:var(--manager-ink-subtle)] focus:border-[color:var(--manager-accent)]"
            />
            {(imageDataUrl || filePaths.length > 0) ? (
              <div className="mt-2 flex flex-wrap gap-2 text-xs text-[color:var(--manager-ink-soft)]">
                {imageDataUrl ? <span className="rounded-[6px] bg-[color:var(--manager-panel)] px-2 py-1">图片：{imageName}</span> : null}
                {filePaths.map((path) => <span key={path} className="rounded-[6px] bg-[color:var(--manager-panel)] px-2 py-1">{path.split(/[\\/]/).pop()}</span>)}
              </div>
            ) : null}
            <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
              <div className="flex gap-2">
                <button type="button" onClick={() => void handlePickFiles()} className="rounded-[7px] border border-[color:var(--manager-border)] px-2.5 py-1.5 text-xs text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel)]">添加文件</button>
                {imageDataUrl ? <button type="button" onClick={() => { setImageDataUrl(undefined); setImageName(''); }} className="rounded-[7px] border border-[color:var(--manager-border)] px-2.5 py-1.5 text-xs text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel)]">移除图片</button> : null}
                {filePaths.length > 0 ? <button type="button" onClick={() => setFilePaths([])} className="rounded-[7px] border border-[color:var(--manager-border)] px-2.5 py-1.5 text-xs text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel)]">清除文件</button> : null}
              </div>
              <button type="submit" disabled={isSaving} className="rounded-[7px] bg-[color:var(--manager-accent)] px-3 py-1.5 text-xs text-white disabled:opacity-60">{isSaving ? '保存中…' : '保存到收集箱'}</button>
            </div>
          </form>
        ) : null}

        {isLoading ? <p className="py-8 text-center text-sm text-[color:var(--manager-ink-muted)]">加载中…</p> : null}
        {!isLoading && visibleItems.length === 0 ? (
          <div className="py-12 text-center text-sm text-[color:var(--manager-ink-muted)]">
            {viewMode === 'inbox' ? '收集箱是空的' : '暂无归档内容'}
          </div>
        ) : null}
        <div className="divide-y divide-[color:var(--manager-border)]">
          {visibleItems.map((item) => (
            <article key={item.id} className="py-3">
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="rounded-[5px] border border-[color:var(--manager-border)] px-1.5 py-0.5 text-[10px] text-[color:var(--manager-ink-muted)]">{itemKindLabel(item)}</span>
                    <span className="text-[11px] text-[color:var(--manager-ink-subtle)]">{item.source} · {formatCaptureTime(item.capturedAt)}</span>
                  </div>
                  <h3 className="mt-1 break-words text-sm font-medium text-[color:var(--manager-ink-strong)]">{item.title || item.content || '收集内容'}</h3>
                  {item.content && item.title ? <p className="mt-1 whitespace-pre-wrap break-words text-sm leading-6 text-[color:var(--manager-ink-soft)]">{item.content}</p> : null}
                  {item.context ? <p className="mt-1 text-xs text-[color:var(--manager-ink-subtle)]">上下文：{item.context}</p> : null}
                </div>
                <div className="flex shrink-0 gap-1">
                  {item.kind === 'link' ? <button type="button" onClick={() => void openInboxLink(item.id).catch((caught) => setError(String(caught)))} className="rounded-[6px] px-2 py-1 text-xs text-[color:var(--manager-accent)] hover:bg-[color:var(--manager-panel)]">打开</button> : null}
                  {!item.archivedAt && !item.convertedNoteId ? <button type="button" onClick={() => void convertToNote(item)} className="rounded-[6px] px-2 py-1 text-xs text-[color:var(--manager-accent)] hover:bg-[color:var(--manager-panel)]">整理为笔记</button> : null}
                  <button type="button" onClick={() => void updateItemState(item)} className="rounded-[6px] px-2 py-1 text-xs text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel)]">{item.archivedAt ? '移回收集箱' : '归档'}</button>
                  <button type="button" onClick={() => void handleDelete(item)} className="rounded-[6px] px-2 py-1 text-xs text-[color:var(--manager-danger)] hover:bg-[color:var(--manager-panel)]">删除</button>
                </div>
              </div>
              {item.attachments.length > 0 ? (
                <div className="mt-2 flex flex-wrap gap-2">
                  {item.attachments.map((attachment) => (
                    <button key={attachment.id} type="button" onClick={() => void openInboxAttachment(attachment.id).catch((caught) => setError(String(caught)))} className="max-w-full truncate rounded-[6px] border border-[color:var(--manager-border)] px-2 py-1 text-xs text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel)]">
                      打开附件 · {attachment.name}
                    </button>
                  ))}
                </div>
              ) : null}
              {item.attachments.some((attachment) => attachment.kind === 'audio') && !item.archivedAt ? (
                <div className="mt-2 max-w-md">
                  <TranscribeControl
                    itemId={item.id}
                    audioLabel={item.attachments.find((attachment) => attachment.kind === 'audio')?.name}
                    audioSizeBytes={item.attachments.find((attachment) => attachment.kind === 'audio')?.sizeBytes ?? null}
                    onConvertToNote={() => void convertToNote(item)}
                    onTranscribed={() => void refresh()}
                  />
                </div>
              ) : null}
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
