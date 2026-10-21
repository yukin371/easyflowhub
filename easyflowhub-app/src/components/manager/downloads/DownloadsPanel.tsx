import { useCallback, useEffect, useMemo, useState } from 'react';
import './DownloadsPanel.css';
import { open } from '@tauri-apps/plugin-dialog';
import {
  cancelDownload,
  copyDownloadPath,
  enqueueDownload,
  getDownloadSettings,
  listDownloads,
  openDownload,
  pauseDownload,
  retryDownload,
  resumeDownload,
  revealDownload,
  updateDownloadSettings,
} from '../../../lib/tauri/download';
import { captureInboxItem } from '../../../lib/tauri/inbox';
import { copyTextToClipboard } from '../../../lib/tauri/clipboard';
import { formatNativeError } from '../../../lib/tauri/errors';
import type { DownloadConflictPolicy, DownloadSettings, DownloadStatus, DownloadState } from '../../../types/download';

const STATUS_LABEL: Record<DownloadState, string> = {
  queued: '排队中',
  downloading: '下载中',
  paused: '已暂停',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
};

const DOWNLOAD_ERROR_MESSAGES = {
  unavailable: '下载服务暂不可用，请在桌面应用中打开 EasyFlowHub。',
  fallback: '下载操作失败，请检查网络、目录权限和文件状态后重试。',
};

function formatBytes(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '未知大小';
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 * 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function fileNameFromUrl(value: string): string {
  try {
    const url = new URL(value);
    const last = decodeURIComponent(url.pathname.split('/').filter(Boolean).pop() ?? '');
    return last || 'download';
  } catch {
    return 'download';
  }
}

function progress(item: DownloadStatus): number | null {
  if (!item.totalBytes || item.totalBytes <= 0) return null;
  return Math.min(100, Math.round((item.downloadedBytes / item.totalBytes) * 100));
}

export function DownloadsPanel() {
  const [settings, setSettings] = useState<DownloadSettings>({ targetDir: null, conflictPolicy: 'rename' });
  const [items, setItems] = useState<DownloadStatus[]>([]);
  const [url, setUrl] = useState('');
  const [fileName, setFileName] = useState('');
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<Set<string>>(() => new Set());

  const refresh = useCallback(async () => {
    try {
      const next = await listDownloads();
      setItems(next);
    } catch (caught) {
      // Manual refresh and the polling loop share this path.  Surface a
      // native queue error in the panel instead of leaving an unhandled
      // promise rejection in the window console.
      setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES));
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    void Promise.all([getDownloadSettings(), listDownloads()]).then(([nextSettings, nextItems]) => {
      if (disposed) return;
      setSettings(nextSettings);
      setItems(nextItems);
    }).catch((caught) => { if (!disposed) setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); })
      .finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, []);

  useEffect(() => {
    const active = items.some((item) => item.status === 'queued' || item.status === 'downloading');
    if (!active) return;
    const timer = window.setInterval(() => { void refresh(); }, 1000);
    return () => window.clearInterval(timer);
  }, [items, refresh]);

  const updateSetting = async (patch: Partial<DownloadSettings>) => {
    setError(undefined);
    try {
      const next = await updateDownloadSettings({ ...settings, ...patch });
      setSettings(next);
      setNotice('下载设置已保存。');
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
  };

  const chooseDirectory = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: '选择下载目录' });
      if (!selected || Array.isArray(selected)) return;
      await updateSetting({ targetDir: selected });
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
  };

  const submit = async () => {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) { setError('请输入下载 URL。'); return; }
    try {
      const parsed = new URL(trimmedUrl);
      if (!['http:', 'https:'].includes(parsed.protocol)) throw new Error('仅支持 http 和 https URL。');
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); return; }
    if (settings.conflictPolicy === 'overwrite' && !window.confirm('覆盖策略会替换同名文件，确定继续吗？')) return;
    setError(undefined); setNotice(undefined);
    try {
      const created = await enqueueDownload({
        url: trimmedUrl,
        fileName: fileName.trim() || undefined,
        targetDir: settings.targetDir ?? undefined,
        conflictPolicy: settings.conflictPolicy,
      });
      setItems((current) => [created, ...current.filter((item) => item.id !== created.id)]);
      setUrl(''); setFileName(''); setNotice('已加入下载队列。');
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
  };

  const runStatusAction = async (id: string, action: (id: string) => Promise<DownloadStatus>) => {
    if (busy.has(id)) return;
    setBusy((current) => new Set(current).add(id)); setError(undefined); setNotice(undefined);
    try {
      const next = await action(id);
      setItems((current) => current.map((item) => item.id === id ? next : item));
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
    finally { setBusy((current) => { const next = new Set(current); next.delete(id); return next; }); }
  };

  const runPathAction = async (id: string, action: (id: string) => Promise<unknown>) => {
    if (busy.has(id)) return;
    setBusy((current) => new Set(current).add(id)); setError(undefined); setNotice(undefined);
    try { await action(id); setNotice('操作已完成。'); }
    catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
    finally { setBusy((current) => { const next = new Set(current); next.delete(id); return next; }); }
  };

  const copyPath = async (id: string) => {
    try {
      const path = await copyDownloadPath(id);
      await copyTextToClipboard(path);
      setNotice('路径已复制。');
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
  };

  const captureDownload = async (id: string) => {
    if (busy.has(id)) return;
    setBusy((current) => new Set(current).add(id)); setError(undefined); setNotice(undefined);
    try {
      const path = await copyDownloadPath(id);
      await captureInboxItem({ filePaths: [path], source: 'download', context: 'download' });
      setNotice('已加入收集箱。');
    } catch (caught) { setError(formatNativeError(caught, DOWNLOAD_ERROR_MESSAGES)); }
    finally { setBusy((current) => { const next = new Set(current); next.delete(id); return next; }); }
  };

  const activeCount = useMemo(() => items.filter((item) => item.status === 'queued' || item.status === 'downloading').length, [items]);

  return (
    <section className="flex h-full min-h-0 flex-col text-[color:var(--manager-ink)]">
      <header className="shrink-0 border-b border-[color:var(--manager-border)] px-3 py-3 sm:px-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <p className="manager-kicker">Resources</p>
            <h2 className="text-xl font-semibold text-[color:var(--manager-ink-strong)]">下载</h2>
            <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">{activeCount ? `${activeCount} 项进行中` : 'URL 下载队列'}</p>
          </div>
          <button type="button" onClick={() => void refresh()} className="downloads-tool-button">刷新</button>
        </div>
        <div className="mt-3 grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(140px,220px)_auto]">
          <label><span className="sr-only">下载 URL</span><input aria-label="下载 URL" value={url} onChange={(event) => setUrl(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') void submit(); }} placeholder="粘贴 http(s) 下载地址" className="downloads-input" /></label>
          <label><span className="sr-only">文件名</span><input aria-label="文件名" value={fileName} onChange={(event) => setFileName(event.target.value)} placeholder="文件名（可选）" className="downloads-input" /></label>
          <button type="button" onClick={() => void submit()} className="downloads-primary-button">加入队列</button>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-3 sm:px-4">
        {error ? <p role="alert" className="manager-alert-error mb-3 rounded-[8px] border px-3 py-2 text-xs">{error}</p> : null}
        {notice ? <p role="status" className="mb-3 text-xs text-[color:var(--manager-success)]">{notice}</p> : null}
        <section className="mb-4 rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-3">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-medium text-[color:var(--manager-ink-strong)]">保存到</span>
            <span className="min-w-0 flex-1 truncate text-xs text-[color:var(--manager-ink-muted)]" title={settings.targetDir ?? undefined}>{settings.targetDir ?? '未设置（使用应用默认目录）'}</span>
            <button type="button" onClick={() => void chooseDirectory()} className="downloads-tool-button">选择目录</button>
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
            <label className="text-[color:var(--manager-ink-muted)]" htmlFor="download-conflict">文件冲突</label>
            <select id="download-conflict" aria-label="文件冲突策略" value={settings.conflictPolicy} onChange={(event) => void updateSetting({ conflictPolicy: event.target.value as DownloadConflictPolicy })} className="downloads-select">
              <option value="rename">自动重命名</option><option value="fail">报告冲突</option><option value="overwrite">覆盖</option>
            </select>
          </div>
        </section>

        {loading ? <p className="py-8 text-center text-sm text-[color:var(--manager-ink-muted)]">正在读取下载队列…</p> : null}
        {!loading && items.length === 0 ? <div className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">暂无下载，粘贴 URL 后加入队列。</div> : null}
        <div className="space-y-2">
          {items.map((item) => {
            const percent = progress(item); const isBusy = busy.has(item.id);
            return <article key={item.id} className="downloads-item">
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-[color:var(--manager-ink-strong)]" title={item.url}>{item.fileName || fileNameFromUrl(item.url)}</p>
                <p className="mt-0.5 truncate text-[11px] text-[color:var(--manager-ink-subtle)]" title={item.targetPath ?? undefined}>{item.targetPath ?? item.url}</p>
                <div className="mt-2 flex items-center gap-2 text-[11px] text-[color:var(--manager-ink-muted)]"><span>{STATUS_LABEL[item.status]}</span><span>{formatBytes(item.downloadedBytes)} / {formatBytes(item.totalBytes)}</span>{item.retryCount ? <span>重试 {item.retryCount}</span> : null}</div>
                {percent !== null ? <div className="downloads-progress" aria-label={`${percent}%`}><span style={{ width: `${percent}%` }} /></div> : null}
                {item.error ? <p className="mt-1 break-words text-[11px] text-[color:var(--manager-danger)]">{item.error}</p> : null}
              </div>
              <div className="flex shrink-0 flex-wrap justify-end gap-1">
                {item.status === 'downloading' || item.status === 'queued' ? <button type="button" disabled={isBusy} onClick={() => void runStatusAction(item.id, pauseDownload)} className="downloads-row-action">暂停</button> : null}
                {item.status === 'paused' ? <button type="button" disabled={isBusy} onClick={() => void runStatusAction(item.id, resumeDownload)} className="downloads-row-action">继续</button> : null}
                {item.status === 'failed' ? <button type="button" disabled={isBusy} onClick={() => void runStatusAction(item.id, retryDownload)} className="downloads-row-action">重试</button> : null}
                {item.status === 'queued' || item.status === 'downloading' || item.status === 'paused' ? <button type="button" disabled={isBusy} onClick={() => void runStatusAction(item.id, cancelDownload)} className="downloads-row-action downloads-row-danger">取消</button> : null}
                {item.status === 'completed' && item.targetPath ? <><button type="button" disabled={isBusy} onClick={() => void runPathAction(item.id, openDownload)} className="downloads-row-action">打开</button><button type="button" disabled={isBusy} onClick={() => void runPathAction(item.id, revealDownload)} className="downloads-row-action">定位</button><button type="button" disabled={isBusy} onClick={() => void copyPath(item.id)} className="downloads-row-action">复制路径</button><button type="button" disabled={isBusy} onClick={() => void captureDownload(item.id)} className="downloads-row-action">加入收集箱</button></> : null}
              </div>
            </article>;
          })}
        </div>
      </div>
    </section>
  );
}


