/**
 * Import external script files into a configured storage root.
 */

import { useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { scriptsApi } from '../../../lib/api/scriptmgr';
import type { RootsDetail } from '../../../types/scriptmgr';
import { Modal, SCRIPT_EXTENSIONS, inputClass, primaryButton, secondaryButton } from './ScriptDialogShell';

export function ScriptImportDialog({
  roots,
  onClose,
  onImported,
}: {
  roots: RootsDetail | null;
  onClose: () => void;
  onImported: () => void;
}) {
  const availableRoots = useMemo(() => roots?.roots ?? [], [roots]);
  const [root, setRoot] = useState(availableRoots[0] ?? '');
  const [paths, setPaths] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string>();
  const [summary, setSummary] = useState<string>();

  useEffect(() => {
    if (!root && availableRoots.length > 0) setRoot(availableRoots[0]);
  }, [availableRoots, root]);

  const pickFiles = async () => {
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [{ name: '脚本', extensions: SCRIPT_EXTENSIONS }],
    });
    if (!selected) return;
    const list = Array.isArray(selected) ? selected : [selected];
    setPaths((current) => Array.from(new Set([...current, ...list])));
  };

  const handleImport = async () => {
    setImporting(true);
    setError(undefined);
    setSummary(undefined);
    try {
      const response = await scriptsApi.importFiles(root, paths);
      const imported = response.imported?.length ?? 0;
      const failed = response.failed ?? [];
      setSummary(`已导入 ${imported} 个脚本${failed.length > 0 ? `，${failed.length} 个失败` : ''}`);
      if (failed.length > 0) {
        setError(failed.map((item) => `${item.path}: ${item.error}`).join('\n'));
      }
      setPaths([]);
      onImported();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Modal
      title="导入脚本"
      subtitle="从磁盘复制脚本文件到已配置的存储位置（不覆盖同名文件）"
      onClose={onClose}
      footer={
        <div className="flex items-center justify-end gap-2">
          <button type="button" className={secondaryButton} onClick={onClose}>
            关闭
          </button>
          <button
            type="button"
            className={primaryButton}
            onClick={() => void handleImport()}
            disabled={importing || !root || paths.length === 0}
          >
            {importing ? '导入中…' : `导入 ${paths.length} 个文件`}
          </button>
        </div>
      }
    >
      <div className="space-y-3">
        {summary ? (
          <p role="status" className="rounded-[10px] border border-[color:var(--manager-success)]/40 px-3 py-2 text-xs text-[color:var(--manager-success)]">
            {summary}
          </p>
        ) : null}
        {error ? (
          <p role="alert" className="whitespace-pre-wrap rounded-[10px] border border-[color:var(--manager-danger)]/40 px-3 py-2 text-xs text-[color:var(--manager-danger)]">
            {error}
          </p>
        ) : null}
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">导入到</span>
          <select aria-label="导入到" value={root} onChange={(event) => setRoot(event.target.value)} className={inputClass}>
            {availableRoots.length === 0 ? <option value="">请先添加存储位置</option> : null}
            {availableRoots.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <button type="button" className={secondaryButton} onClick={() => void pickFiles()}>
          选择脚本文件…
        </button>
        {paths.length > 0 ? (
          <ul className="space-y-1">
            {paths.map((path) => (
              <li key={path} className="flex items-center justify-between gap-2 rounded-[8px] bg-[color:var(--manager-panel-strong)] px-3 py-1.5 text-xs text-[color:var(--manager-ink-soft)]">
                <span className="truncate font-mono">{path}</span>
                <button
                  type="button"
                  onClick={() => setPaths((current) => current.filter((item) => item !== path))}
                  className="shrink-0 text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-danger)]"
                >
                  移除
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-[color:var(--manager-ink-subtle)]">尚未选择文件</p>
        )}
      </div>
    </Modal>
  );
}
