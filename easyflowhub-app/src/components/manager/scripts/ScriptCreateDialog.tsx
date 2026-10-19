/**
 * Create a new script inside a configured storage root.
 */

import { useEffect, useMemo, useState } from 'react';
import { scriptsApi } from '../../../lib/api/scriptmgr';
import type { RootsDetail, ScriptSummary } from '../../../types/scriptmgr';
import { Modal, SCRIPT_EXTENSIONS, inputClass, primaryButton, secondaryButton } from './ScriptDialogShell';

const TEMPLATES: Record<string, string> = {
  ps1: '# 新脚本\nparam()\n\nWrite-Host "Hello from EasyFlowHub"\n',
  py: '#!/usr/bin/env python3\n\n\ndef main() -> None:\n    print("Hello from EasyFlowHub")\n\n\nif __name__ == "__main__":\n    main()\n',
  bat: '@echo off\n\necho Hello from EasyFlowHub\n',
  cmd: '@echo off\n\necho Hello from EasyFlowHub\n',
};

export function ScriptCreateDialog({
  roots,
  onClose,
  onCreated,
}: {
  roots: RootsDetail | null;
  onClose: () => void;
  onCreated: (script: ScriptSummary) => void;
}) {
  const availableRoots = useMemo(() => roots?.roots ?? [], [roots]);
  const [root, setRoot] = useState(availableRoots[0] ?? '');
  const [filename, setFilename] = useState('new-script.ps1');
  const [name, setName] = useState('');
  const [category, setCategory] = useState('');
  const [description, setDescription] = useState('');
  const [content, setContent] = useState(TEMPLATES.ps1);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!root && availableRoots.length > 0) setRoot(availableRoots[0]);
  }, [availableRoots, root]);

  const handleFilenameChange = (value: string) => {
    setFilename(value);
    const extension = value.split('.').pop()?.toLowerCase() ?? '';
    if (TEMPLATES[extension] && !name) {
      setContent(TEMPLATES[extension]);
    }
  };

  const handleCreate = async () => {
    setSaving(true);
    setError(undefined);
    try {
      const script = await scriptsApi.create({ root, filename, content, name, category, description });
      onCreated(script);
      onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title="新建脚本"
      subtitle="写入已配置的存储位置，保存后立即出现在脚本库中"
      onClose={onClose}
      footer={
        <div className="flex items-center justify-end gap-2">
          <button type="button" className={secondaryButton} onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className={primaryButton}
            onClick={() => void handleCreate()}
            disabled={saving || !root || !filename.trim()}
          >
            {saving ? '创建中…' : '创建脚本'}
          </button>
        </div>
      }
    >
      <div className="space-y-3">
        {error ? (
          <p role="alert" className="rounded-[10px] border border-[color:var(--manager-danger)]/40 px-3 py-2 text-xs text-[color:var(--manager-danger)]">
            {error}
          </p>
        ) : null}
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">存储位置</span>
          <select aria-label="存储位置" value={root} onChange={(event) => setRoot(event.target.value)} className={inputClass}>
            {availableRoots.length === 0 ? <option value="">请先添加存储位置</option> : null}
            {availableRoots.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">文件名</span>
          <input
            aria-label="文件名"
            value={filename}
            onChange={(event) => handleFilenameChange(event.target.value)}
            placeholder="deploy.ps1"
            className={inputClass}
          />
          <span className="mt-1 block text-[11px] text-[color:var(--manager-ink-subtle)]">
            支持 {SCRIPT_EXTENSIONS.map((ext) => `.${ext}`).join(' / ')}
          </span>
        </label>
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="block">
            <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">显示名称</span>
            <input aria-label="显示名称" value={name} onChange={(event) => setName(event.target.value)} placeholder="可选" className={inputClass} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">分类</span>
            <input aria-label="分类" value={category} onChange={(event) => setCategory(event.target.value)} placeholder="可选" className={inputClass} />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">描述</span>
          <input aria-label="描述" value={description} onChange={(event) => setDescription(event.target.value)} placeholder="可选" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">初始内容</span>
          <textarea
            aria-label="初始内容"
            value={content}
            onChange={(event) => setContent(event.target.value)}
            spellCheck={false}
            className="h-40 w-full resize-y rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-3 font-mono text-xs leading-6 text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          />
        </label>
      </div>
    </Modal>
  );
}
