import { useState } from 'react';

export interface NumberSettingInputProps {
  label: string;
  value: number;
  min: number;
  max: number;
  /** 无法解析或（blur 模式下）越界时的回退值。 */
  fallback: number;
  commit: (value: number) => void;
  hint?: string;
  /**
   * blur：先允许临时编辑、失焦后提交（快速笔记 / 待办）；
   * change：即时提交，解析失败时落到 fallback（回收站 / 编辑器）。
   */
  commitOn?: 'blur' | 'change';
}

/**
 * 设置页共用的数字输入卡片：blur 模式保留“可清空后再输入”的临时编辑态
 * （原 SettingsPanel 的 editingValues），change 模式保持即时提交行为。
 */
export function NumberSettingInput({
  label,
  value,
  min,
  max,
  fallback,
  commit,
  hint,
  commitOn = 'blur',
}: NumberSettingInputProps) {
  const [draft, setDraft] = useState<string>();

  return (
    <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
      <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
        {label}
      </label>
      {commitOn === 'change' ? (
        <input
          type="number"
          min={min}
          max={max}
          value={value}
          onChange={(e) => commit(parseInt(e.target.value) || fallback)}
          className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
        />
      ) : (
        <input
          type="number"
          min={min}
          max={max}
          value={draft ?? value}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={(e) => {
            const num = parseInt(e.target.value);
            const finalValue = isNaN(num) || num < min || num > max ? fallback : num;
            commit(finalValue);
            setDraft(undefined);
          }}
          className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
        />
      )}
      {hint ? (
        <p className="mt-2 text-xs text-[color:var(--manager-ink-subtle)]">
          {hint}
        </p>
      ) : null}
    </div>
  );
}
