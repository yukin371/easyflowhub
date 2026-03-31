/**
 * ScriptDrawer - 脚本详情右侧抽屉
 */

import { useState } from 'react';
import { useScriptStore } from './scriptStore';
import { formatDuration, getStatusColor } from '../../../lib/tauri/scriptmgr';
import { scriptsApi } from '../../../lib/api/scriptmgr';

interface ScriptDrawerProps {
  onClose: () => void;
}

function ParameterRow({
  name,
  type,
  label,
  required,
  defaultValue,
  description,
}: {
  name: string;
  type: string;
  label: string;
  required?: boolean;
  defaultValue?: string;
  description?: string;
}) {
  return (
    <div className="rounded-[12px] border border-[color:var(--manager-border)] bg-white/40 px-3 py-2.5">
      <div className="flex items-center gap-2">
        <span className="font-medium text-[color:var(--manager-ink-strong)]">{name}</span>
        <span className="rounded-full bg-[color:var(--manager-accent-soft)] px-1.5 py-0.5 text-[10px] text-[color:var(--manager-accent)]">
          {type}
        </span>
        {required && (
          <span className="text-[10px] text-red-500">*必填</span>
        )}
      </div>
      {label && (
        <p className="mt-0.5 text-sm text-[color:var(--manager-ink-soft)]">{label}</p>
      )}
      {defaultValue !== undefined && (
        <p className="mt-0.5 text-xs text-[color:var(--manager-ink-subtle)]">
          默认: {defaultValue}
        </p>
      )}
      {description && (
        <p className="mt-1 text-xs leading-relaxed text-[color:var(--manager-ink-muted)]">
          {description}
        </p>
      )}
    </div>
  );
}

function TaskHistoryItem({
  task,
}: {
  task: { task_id: string; status: string; exit_code?: number; duration_ms?: number; created_at: string };
}) {
  const statusColor = getStatusColor(task.status);
  const colorClass =
    statusColor === 'green'
      ? 'text-green-600'
      : statusColor === 'red'
        ? 'text-red-600'
        : statusColor === 'blue'
          ? 'text-blue-600'
          : 'text-gray-500';

  const statusIcon =
    task.status === 'success' || task.status === 'succeeded' ? '✓' :
    task.status === 'failed' || task.status === 'error' ? '✗' :
    task.status === 'running' ? '●' : '○';

  return (
    <div className="flex items-center justify-between rounded-[10px] bg-white/40 px-3 py-2 text-sm">
      <div className="flex items-center gap-2">
        <span className={colorClass}>{statusIcon}</span>
        <span className="text-[color:var(--manager-ink-soft)]">
          {new Date(task.created_at).toLocaleString('zh-CN', {
            month: 'numeric',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
          })}
        </span>
      </div>
      <span className="text-[color:var(--manager-ink-muted)]">
        {formatDuration(task.duration_ms)}
      </span>
    </div>
  );
}

export function ScriptDrawer({ onClose }: ScriptDrawerProps) {
  const { selectedScriptDetail, recentTasks, detailLoading, categories, fetchScripts, fetchScriptDetail } = useScriptStore();
  const [categoryUpdating, setCategoryUpdating] = useState(false);

  // Handle category change
  const handleCategoryChange = async (newCategory: string) => {
    if (!selectedScriptDetail) return;
    setCategoryUpdating(true);
    try {
      await scriptsApi.updateCategory(selectedScriptDetail.id, newCategory);
      // Refresh both list and detail
      await Promise.all([fetchScripts(), fetchScriptDetail(selectedScriptDetail.id)]);
    } catch (err) {
      console.error('Failed to update category:', err);
    } finally {
      setCategoryUpdating(false);
    }
  };

  if (!selectedScriptDetail) {
    return (
      <aside className="w-80 shrink-0 rounded-[20px] border border-[color:var(--manager-border)] bg-[rgba(255,251,245,0.88)] p-4">
        <div className="flex h-full items-center justify-center">
          {detailLoading ? (
            <div className="h-6 w-6 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
          ) : (
            <p className="text-sm text-[color:var(--manager-ink-subtle)]">加载中...</p>
          )}
        </div>
      </aside>
    );
  }

  const script = selectedScriptDetail;

  return (
    <aside className="flex w-80 shrink-0 flex-col rounded-[20px] border border-[color:var(--manager-border)] bg-[rgba(255,251,245,0.88)]">
      {/* Header */}
      <header className="flex items-center justify-between border-b border-[color:var(--manager-border)] px-4 py-3">
        <div className="flex items-center gap-2">
          <span className="text-lg">📄</span>
          <h3 className="truncate font-medium text-[color:var(--manager-ink-strong)]">
            {script.name}
          </h3>
        </div>
        <button
          onClick={onClose}
          className="flex h-8 w-8 items-center justify-center rounded-full text-[color:var(--manager-ink-muted)] transition hover:bg-black/5 hover:text-[color:var(--manager-ink-strong)]"
        >
          ✕
        </button>
      </header>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        {/* Category Selector */}
        <section className="mb-4">
          <h4 className="mb-2 text-xs font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
            分类
          </h4>
          <select
            value={script.category || ''}
            onChange={(e) => handleCategoryChange(e.target.value)}
            disabled={categoryUpdating}
            className="w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white/60 px-3 py-2.5 text-sm text-[color:var(--manager-ink-strong)] outline-none transition focus:border-[color:var(--manager-accent)] disabled:opacity-50"
          >
            <option value="">未分类</option>
            {categories.map((cat) => (
              <option key={cat.name} value={cat.name}>
                {cat.name} ({cat.count})
              </option>
            ))}
          </select>
        </section>

        {/* Description */}
        {script.description && (
          <section className="mb-4">
            <p className="text-sm leading-relaxed text-[color:var(--manager-ink-soft)]">
              {script.description}
            </p>
          </section>
        )}

        {/* Parameters */}
        {script.parameters && script.parameters.length > 0 && (
          <section className="mb-4">
            <h4 className="mb-2 text-xs font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
              参数
            </h4>
            <div className="space-y-2">
              {script.parameters.map((param) => (
                <ParameterRow
                  key={param.name}
                  name={param.name}
                  type={param.type}
                  label={param.label || param.name}
                  required={param.required}
                  defaultValue={param.default}
                  description={param.description}
                />
              ))}
            </div>
          </section>
        )}

        {/* MCP Config */}
        <section className="mb-4">
          <h4 className="mb-2 text-xs font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
            MCP 配置
          </h4>
          <div className="rounded-[12px] border border-[color:var(--manager-border)] bg-white/40 px-3 py-2.5">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                defaultChecked
                className="h-4 w-4 rounded border-[color:var(--manager-border)]"
              />
              <span className="text-sm text-[color:var(--manager-ink)]">暴露给 AI</span>
            </label>
            <p className="mt-1.5 text-xs text-[color:var(--manager-ink-muted)]">
              工具名: script_{script.id.replace(/-/g, '_')}
            </p>
          </div>
        </section>

        {/* Recent Tasks */}
        <section>
          <h4 className="mb-2 text-xs font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
            最近执行
          </h4>
          {recentTasks.length === 0 ? (
            <p className="text-sm text-[color:var(--manager-ink-subtle)]">暂无执行记录</p>
          ) : (
            <div className="space-y-1.5">
              {recentTasks.slice(0, 3).map((task) => (
                <TaskHistoryItem key={task.task_id} task={task} />
              ))}
            </div>
          )}
          {recentTasks.length > 0 && (
            <button className="mt-2 w-full rounded-[10px] border border-[color:var(--manager-border)] bg-white/40 py-2 text-sm text-[color:var(--manager-ink-muted)] transition hover:bg-white/60 hover:text-[color:var(--manager-ink-strong)]">
              查看全部 →
            </button>
          )}
        </section>
      </div>

      {/* Footer */}
      <footer className="border-t border-[color:var(--manager-border)] px-4 py-3">
        <div className="flex items-center gap-2 text-xs text-[color:var(--manager-ink-subtle)]">
          <span>{script.script_type}</span>
          <span>•</span>
          <span className="truncate">{script.id}</span>
        </div>
      </footer>
    </aside>
  );
}
