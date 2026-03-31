/**
 * TasksPanel - 执行监控面板
 * 显示运行中任务、完成任务历史、日志查看器
 */

import { useState, useEffect, useCallback } from 'react';
import { tasksApi, type TaskLogResponse } from '../../../lib/api/scriptmgr';
import type { TaskInfo } from '../../../types/scriptmgr';

// Status colors and labels
const STATUS_CONFIG: Record<string, { color: string; bg: string; label: string }> = {
  pending: { color: 'text-amber-600', bg: 'bg-amber-100', label: '等待中' },
  running: { color: 'text-blue-600', bg: 'bg-blue-100', label: '运行中' },
  completed: { color: 'text-green-600', bg: 'bg-green-100', label: '已完成' },
  failed: { color: 'text-red-600', bg: 'bg-red-100', label: '失败' },
  cancelled: { color: 'text-gray-500', bg: 'bg-gray-100', label: '已取消' },
};

function formatDuration(ms?: number): string {
  if (!ms) return '-';
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return '刚刚';
  if (diffMins < 60) return `${diffMins} 分钟前`;
  if (diffHours < 24) return `${diffHours} 小时前`;
  if (diffDays < 7) return `${diffDays} 天前`;
  return date.toLocaleDateString('zh-CN');
}

function formatAbsoluteTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function TasksPanel() {
  const [activeTab, setActiveTab] = useState<'all' | 'running' | 'completed'>('all');
  const [tasks, setTasks] = useState<TaskInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedTask, setSelectedTask] = useState<TaskInfo | null>(null);
  const [logContent, setLogContent] = useState<TaskLogResponse | null>(null);
  const [logLoading, setLogLoading] = useState(false);
  const [logTail, setLogTail] = useState(true);

  // Fetch tasks
  const fetchTasks = useCallback(async () => {
    setLoading(true);
    try {
      // For 'all' tab, fetch without status filter
      const status = activeTab === 'all' ? undefined : activeTab;
      const response = await tasksApi.list(status, 100);
      // Sort by created_at descending (newest first)
      const sorted = [...response.tasks].sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
      setTasks(sorted);
    } catch (err) {
      console.error('Failed to fetch tasks:', err);
      setTasks([]);
    } finally {
      setLoading(false);
    }
  }, [activeTab]);

  useEffect(() => {
    fetchTasks();
    // Auto-refresh every 5 seconds for running/all tabs
    const interval = activeTab !== 'completed' ? setInterval(fetchTasks, 5000) : null;
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [fetchTasks, activeTab]);

  // Fetch task log
  const fetchLog = async (task: TaskInfo) => {
    setLogLoading(true);
    try {
      const response = await tasksApi.log(task.task_id, {
        tail: logTail,
        limit: 100,
      });
      setLogContent(response);
    } catch (err) {
      console.error('Failed to fetch log:', err);
      setLogContent(null);
    } finally {
      setLogLoading(false);
    }
  };

  // Select task and load log
  const handleSelectTask = (task: TaskInfo) => {
    setSelectedTask(task);
    setLogContent(null);
  };

  // Cancel task
  const handleCancelTask = async (taskId: string) => {
    try {
      await tasksApi.cancel(taskId);
      fetchTasks();
    } catch (err) {
      console.error('Failed to cancel task:', err);
    }
  };

  // Load log when task selected
  useEffect(() => {
    if (selectedTask) {
      fetchLog(selectedTask);
    }
  }, [selectedTask, logTail]);

  const runningCount = tasks.filter((t) => t.status === 'running' || t.status === 'pending').length;
  const completedCount = tasks.filter((t) => t.status === 'completed' || t.status === 'failed' || t.status === 'cancelled').length;

  return (
    <section className="flex h-full gap-4 px-4 py-4">
      {/* Left: Task List */}
      <div className="flex w-[360px] flex-col gap-4">
        {/* Header */}
        <header className="space-y-1">
          <p className="manager-kicker">Execution</p>
          <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[32px] leading-[1.04] text-[color:var(--manager-ink-strong)]">
            执行监控
          </h2>
        </header>

        {/* Tabs */}
        <div className="flex gap-2">
          <button
            onClick={() => setActiveTab('all')}
            className={`flex items-center gap-2 rounded-full px-4 py-2 text-sm transition ${
              activeTab === 'all'
                ? 'border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-accent)]'
                : 'border border-[color:var(--manager-border)] bg-white/40 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)]'
            }`}
          >
            全部 ({tasks.length})
          </button>
          <button
            onClick={() => setActiveTab('running')}
            className={`flex items-center gap-2 rounded-full px-4 py-2 text-sm transition ${
              activeTab === 'running'
                ? 'border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-accent)]'
                : 'border border-[color:var(--manager-border)] bg-white/40 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)]'
            }`}
          >
            <span className={`h-2 w-2 rounded-full ${activeTab === 'running' ? 'bg-blue-500 animate-pulse' : 'bg-gray-300'}`} />
            运行中 ({runningCount})
          </button>
          <button
            onClick={() => setActiveTab('completed')}
            className={`flex items-center gap-2 rounded-full px-4 py-2 text-sm transition ${
              activeTab === 'completed'
                ? 'border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-accent)]'
                : 'border border-[color:var(--manager-border)] bg-white/40 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)]'
            }`}
          >
            已完成 ({completedCount})
          </button>
        </div>

        {/* Task List */}
        <div className="flex-1 overflow-auto rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-3">
          {loading ? (
            <div className="flex h-full items-center justify-center text-[color:var(--manager-ink-muted)]">
              加载中...
            </div>
          ) : tasks.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-[color:var(--manager-ink-muted)]">
              <span className="text-3xl">∅</span>
              <span className="text-sm">暂无{activeTab === 'all' ? '' : activeTab === 'running' ? '运行中' : '已完成'}任务</span>
            </div>
          ) : (
            <div className="space-y-2">
              {tasks.map((task) => {
                const config = STATUS_CONFIG[task.status] || STATUS_CONFIG.pending;
                const isSelected = selectedTask?.task_id === task.task_id;
                return (
                  <button
                    key={task.task_id}
                    onClick={() => handleSelectTask(task)}
                    className={`w-full rounded-[14px] border p-3 text-left transition ${
                      isSelected
                        ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)]'
                        : 'border-[color:var(--manager-border)] bg-white/40 hover:border-[color:var(--manager-accent)]'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className={`truncate font-mono text-sm text-[color:var(--manager-ink-strong)]`}>
                            {task.script_id}
                          </span>
                          <span className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium ${config.bg} ${config.color}`}>
                            {config.label}
                          </span>
                        </div>
                        <p className="mt-1 truncate text-xs text-[color:var(--manager-ink-muted)]">
                          {task.task_id.slice(0, 8)}...
                        </p>
                      </div>
                      {task.status === 'running' && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleCancelTask(task.task_id);
                          }}
                          className="shrink-0 rounded-full border border-red-200 bg-red-50 px-2 py-1 text-xs text-red-600 transition hover:bg-red-100"
                        >
                          取消
                        </button>
                      )}
                    </div>
                    <div className="mt-2 flex items-center gap-3 text-xs text-[color:var(--manager-ink-subtle)]">
                      <span className="font-mono">{formatAbsoluteTime(task.created_at)}</span>
                      <span className="text-[color:var(--manager-ink-subtle)]">({formatRelativeTime(task.created_at)})</span>
                      {task.duration_ms && <span>{formatDuration(task.duration_ms)}</span>}
                      {task.exit_code !== undefined && (
                        <span className={task.exit_code === 0 ? 'text-green-600' : 'text-red-600'}>
                          exit:{task.exit_code}
                        </span>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* Refresh Button */}
        <button
          onClick={fetchTasks}
          disabled={loading}
          className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)] disabled:opacity-50"
        >
          刷新列表
        </button>
      </div>

      {/* Right: Task Detail & Log */}
      <div className="flex flex-1 flex-col gap-4">
        {selectedTask ? (
          <>
            {/* Task Info */}
            <div className="rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
              <div className="flex items-start justify-between">
                <div>
                  <p className="text-xs uppercase tracking-wider text-[color:var(--manager-ink-muted)]">
                    任务详情
                  </p>
                  <h3 className="mt-1 font-mono text-lg text-[color:var(--manager-ink-strong)]">
                    {selectedTask.script_id}
                  </h3>
                  <p className="mt-1 font-mono text-xs text-[color:var(--manager-ink-muted)]">
                    ID: {selectedTask.task_id}
                  </p>
                </div>
                <span
                  className={`rounded-full px-3 py-1 text-sm font-medium ${
                    (STATUS_CONFIG[selectedTask.status] || STATUS_CONFIG.pending).bg
                  } ${(STATUS_CONFIG[selectedTask.status] || STATUS_CONFIG.pending).color}`}
                >
                  {(STATUS_CONFIG[selectedTask.status] || STATUS_CONFIG.pending).label}
                </span>
              </div>

              <div className="mt-4 grid grid-cols-3 gap-4 text-sm">
                <div>
                  <p className="text-[color:var(--manager-ink-muted)]">创建时间</p>
                  <p className="font-mono text-[color:var(--manager-ink-strong)]">
                    {new Date(selectedTask.created_at).toLocaleString('zh-CN')}
                  </p>
                </div>
                {selectedTask.duration_ms && (
                  <div>
                    <p className="text-[color:var(--manager-ink-muted)]">执行时长</p>
                    <p className="font-mono text-[color:var(--manager-ink-strong)]">
                      {formatDuration(selectedTask.duration_ms)}
                    </p>
                  </div>
                )}
                {selectedTask.exit_code !== undefined && (
                  <div>
                    <p className="text-[color:var(--manager-ink-muted)]">退出码</p>
                    <p
                      className={`font-mono ${
                        selectedTask.exit_code === 0 ? 'text-green-600' : 'text-red-600'
                      }`}
                    >
                      {selectedTask.exit_code}
                    </p>
                  </div>
                )}
              </div>

              {selectedTask.error && (
                <div className="mt-4 rounded-[12px] border border-red-200 bg-red-50 p-3">
                  <p className="text-xs font-medium uppercase text-red-600">错误信息</p>
                  <p className="mt-1 font-mono text-sm text-red-700">{selectedTask.error}</p>
                </div>
              )}
            </div>

            {/* Log Viewer */}
            <div className="flex-1 overflow-hidden rounded-[20px] border border-[color:var(--manager-border)] bg-white/55">
              <div className="flex items-center justify-between border-b border-[color:var(--manager-border)] px-4 py-3">
                <h4 className="text-sm font-medium uppercase tracking-wider text-[color:var(--manager-ink-muted)]">
                  输出日志
                </h4>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setLogTail(true)}
                    className={`rounded-full px-3 py-1 text-xs transition ${
                      logTail
                        ? 'bg-[color:var(--manager-accent)] text-white'
                        : 'bg-white/60 text-[color:var(--manager-ink-soft)] hover:bg-white'
                    }`}
                  >
                    Tail
                  </button>
                  <button
                    onClick={() => setLogTail(false)}
                    className={`rounded-full px-3 py-1 text-xs transition ${
                      !logTail
                        ? 'bg-[color:var(--manager-accent)] text-white'
                        : 'bg-white/60 text-[color:var(--manager-ink-soft)] hover:bg-white'
                    }`}
                  >
                    全部
                  </button>
                  <button
                    onClick={() => selectedTask && fetchLog(selectedTask)}
                    disabled={logLoading}
                    className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-3 py-1 text-xs text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] disabled:opacity-50"
                  >
                    刷新
                  </button>
                </div>
              </div>
              <div className="h-[calc(100%-52px)] overflow-auto bg-[#1e1e1e] p-4">
                {logLoading ? (
                  <div className="flex h-full items-center justify-center text-gray-400">
                    加载日志...
                  </div>
                ) : logContent ? (
                  <pre className="font-mono text-sm text-gray-300 whitespace-pre-wrap break-all">
                    {logContent.content || '(无输出)'}
                  </pre>
                ) : (
                  <div className="flex h-full items-center justify-center text-gray-400">
                    点击任务查看日志
                  </div>
                )}
              </div>
            </div>
          </>
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-[color:var(--manager-ink-muted)]">
            <span className="text-5xl">📋</span>
            <p className="text-lg">选择任务查看详情和日志</p>
            <p className="text-sm">从左侧列表选择一个任务</p>
          </div>
        )}
      </div>
    </section>
  );
}
