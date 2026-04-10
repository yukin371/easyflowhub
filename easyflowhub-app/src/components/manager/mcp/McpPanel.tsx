/**
 * McpPanel - MCP 配置面板
 * 显示服务状态、静态工具、动态类别管理
 */

import { useState, useEffect, useCallback } from 'react';
import {
  checkServerHealth,
  mcpApi,
  type MCPCategoryInfo,
} from '../../../lib/api/scriptmgr';
import type { MCPServerCatalogEntry } from '../../../types/scriptmgr';
import { navigateToManagerExtension } from '../shared/extensionNavigation';

// Types
interface StaticTool {
  name: string;
  description: string;
}

// Static tools list (from MCP schema)
const STATIC_TOOLS: StaticTool[] = [
  { name: 'list_scripts', description: '列出所有可用脚本' },
  { name: 'run_script', description: '执行指定脚本' },
  { name: 'list_categories', description: '列出所有脚本类别' },
  { name: 'load_category', description: '加载类别下的脚本为工具' },
  { name: 'unload_category', description: '卸载类别的工具' },
  { name: 'search_scripts', description: '搜索脚本' },
  { name: 'get_task_result', description: '获取异步任务结果' },
  { name: 'read_log', description: '读取任务日志' },
  { name: 'get_notes_repo', description: '获取笔记仓库路径' },
  { name: 'set_notes_repo', description: '设置笔记仓库路径' },
  { name: 'sync_notes', description: '同步笔记' },
  { name: 'list_notes', description: '列出笔记' },
  { name: 'get_note', description: '获取单个笔记' },
];

export function McpPanel() {
  const [serverStatus, setServerStatus] = useState<'checking' | 'online' | 'offline'>('checking');
  const [categories, setCategories] = useState<MCPCategoryInfo[]>([]);
  const [servers, setServers] = useState<MCPServerCatalogEntry[]>([]);
  const [loading, setLoading] = useState<string | null>(null);

  // Check server status
  const checkStatus = useCallback(async () => {
    setServerStatus('checking');
    const { ok } = await checkServerHealth();
    setServerStatus(ok ? 'online' : 'offline');
  }, []);

  // Fetch MCP categories
  const fetchCategories = useCallback(async () => {
    try {
      const cats = await mcpApi.listCategories();
      setCategories(cats);
    } catch (err) {
      console.error('Failed to fetch MCP categories:', err);
    }
  }, []);

  const fetchServers = useCallback(async () => {
    try {
      const nextServers = await mcpApi.listServers();
      setServers(nextServers);
    } catch (err) {
      console.error('Failed to fetch MCP servers:', err);
    }
  }, []);

  useEffect(() => {
    checkStatus();
    fetchCategories();
    fetchServers();
  }, [checkStatus, fetchCategories, fetchServers]);

  // Toggle category loaded state
  const handleToggleCategory = async (categoryName: string, currentlyLoaded: boolean) => {
    setLoading(categoryName);
    try {
      if (currentlyLoaded) {
        await mcpApi.unloadCategory(categoryName);
      } else {
        await mcpApi.loadCategory(categoryName);
      }
      await fetchCategories();
    } catch (err) {
      console.error('Failed to toggle category:', err);
    } finally {
      setLoading(null);
    }
  };

  // Count loaded categories
  const loadedCount = categories.filter((c) => c.loaded).length;
  const totalScripts = categories.reduce((sum, c) => sum + c.count, 0);
  const extensionServers = servers.filter((server) => server.status === 'extension').length;
  const conflictedServers = servers.filter((server) => server.status === 'conflicted').length;

  const extensionIdFromSource = (source: string): string | null => {
    if (!source.startsWith('extension:')) {
      return null;
    }
    return source.slice('extension:'.length) || null;
  };

  return (
    <section className="flex h-full flex-col gap-4 px-4 py-4">
      {/* Header */}
      <header className="space-y-1">
        <p className="manager-kicker">MCP Server</p>
        <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[32px] leading-[1.04] text-[color:var(--manager-ink-strong)]">
          MCP 配置
        </h2>
        <p className="text-sm leading-6 text-[color:var(--manager-ink-soft)]">
          管理 AI 可调用的脚本工具
        </p>
      </header>

      {/* Service Status */}
      <div className="rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
        <h3 className="mb-3 text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
          服务状态
        </h3>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span
              className={`flex h-10 w-10 items-center justify-center rounded-full text-lg ${
                serverStatus === 'online'
                  ? 'bg-green-100 text-green-600'
                  : serverStatus === 'offline'
                    ? 'bg-red-100 text-red-600'
                    : 'bg-gray-100 text-gray-400'
              }`}
            >
              {serverStatus === 'checking' ? '◯' : serverStatus === 'online' ? '●' : '✗'}
            </span>
            <div>
              <p className="font-medium text-[color:var(--manager-ink-strong)]">
                {serverStatus === 'checking'
                  ? '检测中...'
                  : serverStatus === 'online'
                    ? '运行中'
                    : '未运行'}
              </p>
              <p className="text-sm text-[color:var(--manager-ink-soft)]">
                {serverStatus === 'online' ? 'localhost:8765' : 'scriptmgr serve'}
              </p>
            </div>
          </div>
          <button
            onClick={checkStatus}
            disabled={serverStatus === 'checking'}
            className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)] disabled:opacity-50"
          >
            刷新
          </button>
        </div>
      </div>

      {/* Stats Row */}
      <div className="grid grid-cols-4 gap-3">
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4 text-center">
          <p className="text-3xl font-light text-[color:var(--manager-accent)]">{STATIC_TOOLS.length}</p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">静态工具</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4 text-center">
          <p className="text-3xl font-light text-[color:var(--manager-accent)]">{loadedCount}</p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">已加载类别</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4 text-center">
          <p className="text-3xl font-light text-[color:var(--manager-accent)]">{totalScripts}</p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">可用脚本</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4 text-center">
          <p className="text-3xl font-light text-[color:var(--manager-accent)]">
            {extensionServers} / {conflictedServers}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">扩展 MCP / 冲突</p>
        </div>
      </div>

      {/* Static Tools */}
      <div className="rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
        <h3 className="mb-3 text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
          静态工具
        </h3>
        <div className="grid grid-cols-2 gap-2 lg:grid-cols-3">
          {STATIC_TOOLS.map((tool) => (
            <div
              key={tool.name}
              className="rounded-[12px] border border-[color:var(--manager-border)] bg-white/40 px-3 py-2"
            >
              <p className="truncate font-mono text-sm text-[color:var(--manager-ink-strong)]">
                {tool.name}
              </p>
              <p className="mt-0.5 line-clamp-1 text-xs text-[color:var(--manager-ink-muted)]">
                {tool.description}
              </p>
            </div>
          ))}
        </div>
      </div>

      <div className="rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
              External MCP Servers
            </h3>
            <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
              persisted config 与 extension `mcp_servers` 的只读 merged view。extension 条目不会自动写回本地配置。
            </p>
          </div>
          <button
            onClick={fetchServers}
            className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
          >
            刷新目录
          </button>
        </div>
        <div className="space-y-2">
          {servers.map((server) => (
            <div
              key={server.key}
              className="rounded-[14px] border border-[color:var(--manager-border)] bg-white/40 px-4 py-3"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="font-medium text-[color:var(--manager-ink-strong)]">{server.name}</p>
                  <p className="mt-1 break-all font-mono text-xs text-[color:var(--manager-ink-muted)]">
                    {server.command} {server.args?.join(' ') ?? ''}
                  </p>
                  <p className="mt-1 text-xs text-[color:var(--manager-ink-soft)]">source: {server.source}</p>
                </div>
                <span
                  className={`rounded-full px-3 py-1 text-xs ${
                    server.status === 'persisted'
                      ? 'bg-emerald-100 text-emerald-700'
                      : server.status === 'extension'
                        ? 'bg-sky-100 text-sky-700'
                        : 'bg-amber-100 text-amber-700'
                  }`}
                >
                  {server.status}
                </span>
              </div>
              {server.description && (
                <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                  {server.description}
                </p>
              )}
              {extensionIdFromSource(server.source) && (
                <button
                  onClick={() => navigateToManagerExtension(extensionIdFromSource(server.source)!)}
                  className="mt-3 rounded-full border border-[color:var(--manager-border)] bg-white/70 px-4 py-2 text-xs text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                >
                  查看扩展详情
                </button>
              )}
              {server.conflict_with && (
                <p className="mt-2 text-xs text-amber-700">conflict with {server.conflict_with}</p>
              )}
            </div>
          ))}
          {servers.length === 0 && (
            <div className="rounded-[16px] border border-dashed border-[color:var(--manager-border)] bg-white/25 px-4 py-6 text-center text-sm text-[color:var(--manager-ink-subtle)]">
              当前未发现外部 MCP server 配置或扩展 overlay。
            </div>
          )}
        </div>
      </div>

      {/* Category Management */}
      <div className="flex-1 overflow-auto rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
            动态类别
          </h3>
          <p className="text-xs text-[color:var(--manager-ink-subtle)]">
            AI 可按需加载脚本类别
          </p>
        </div>
        <div className="space-y-2">
          {categories.map((category) => (
            <div
              key={category.name}
              className="flex items-center justify-between rounded-[14px] border border-[color:var(--manager-border)] bg-white/40 px-4 py-3 transition hover:border-[color:var(--manager-accent)]"
            >
              <div className="flex items-center gap-3">
                <span
                  className={`flex h-8 w-8 items-center justify-center rounded-[10px] border text-sm ${
                    category.loaded
                      ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
                      : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)]'
                  }`}
                >
                  {category.name.slice(0, 2)}
                </span>
                <div>
                  <p className="font-medium text-[color:var(--manager-ink-strong)]">{category.name}</p>
                  <p className="text-xs text-[color:var(--manager-ink-muted)]">
                    {category.count} 个脚本
                  </p>
                </div>
              </div>
              <button
                onClick={() => handleToggleCategory(category.name, category.loaded)}
                disabled={loading === category.name}
                className={`rounded-full px-4 py-1.5 text-sm transition ${
                  category.loaded
                    ? 'border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-accent)] hover:bg-[color:var(--manager-accent)] hover:text-white'
                    : 'border border-[color:var(--manager-border)] bg-white/60 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]'
                } disabled:opacity-50`}
              >
                {loading === category.name ? '...' : category.loaded ? '已加载' : '加载'}
              </button>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
