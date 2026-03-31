import { useState, useEffect, useCallback } from 'react';
import {
  listScripts,
  describeScript,
  runScript,
  formatDuration,
  truncateOutput,
} from '../../lib/tauri/scriptmgr';
import type {
  ScriptSummary,
  ScriptDetail,
  RunResult,
} from '../../types/scriptmgr';

// Get icon based on script type
function getScriptTypeIcon(scriptType: string): string {
  switch (scriptType.toLowerCase()) {
    case 'powershell':
    case 'ps1':
      return '💻';
    case 'python':
    case 'py':
      return '🐍';
    case 'batch':
    case 'cmd':
    case 'bat':
      return '📄';
    case 'shell':
    case 'sh':
    case 'bash':
      return '🐚';
    case 'go':
      return '🔵';
    case 'javascript':
    case 'js':
    case 'ts':
      return '🟨';
    default:
      return '📜';
  }
}

export function ScriptsPage() {
  const [scripts, setScripts] = useState<ScriptSummary[]>([]);
  const [selectedScript, setSelectedScript] = useState<ScriptSummary | null>(null);
  const [scriptDetail, setScriptDetail] = useState<ScriptDetail | null>(null);
  const [isLoadingList, setIsLoadingList] = useState(true);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [isRunning, setIsRunning] = useState(false);
  const [runResult, setRunResult] = useState<RunResult | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [error, setError] = useState<string | null>(null);

  // Load scripts on mount
  useEffect(() => {
    loadScripts();
  }, []);

  const loadScripts = async () => {
    setIsLoadingList(true);
    setError(null);
    try {
      const result = await listScripts();
      setScripts(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load scripts');
      console.error('Failed to load scripts:', err);
    } finally {
      setIsLoadingList(false);
    }
  };

  // Load script detail when selection changes
  useEffect(() => {
    if (selectedScript) {
      loadScriptDetail(selectedScript.id);
    } else {
      setScriptDetail(null);
    }
  }, [selectedScript]);

  const loadScriptDetail = async (scriptId: string) => {
    setIsLoadingDetail(true);
    try {
      const detail = await describeScript(scriptId);
      setScriptDetail(detail);
    } catch (err) {
      console.error('Failed to load script detail:', err);
    } finally {
      setIsLoadingDetail(false);
    }
  };

  const handleRunScript = useCallback(async () => {
    if (!selectedScript) return;

    setIsRunning(true);
    setRunResult(null);
    setError(null);

    try {
      const result = await runScript(selectedScript.id);
      setRunResult(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to run script');
      console.error('Failed to run script:', err);
    } finally {
      setIsRunning(false);
    }
  }, [selectedScript]);

  // Filter scripts by search query
  const filteredScripts = scripts.filter(
    (script) =>
      script.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (script.description?.toLowerCase() || '').includes(searchQuery.toLowerCase()) ||
      (script.category?.toLowerCase() || '').includes(searchQuery.toLowerCase())
  );

  return (
    <div className="h-full flex flex-col">
      {/* Header with search */}
      <div className="p-3 border-b border-black/10">
        <div className="flex items-center gap-2">
          <div className="flex-1 relative">
            <svg
              className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
            <input
              type="text"
              placeholder="搜索脚本..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-9 pr-3 py-1.5 bg-black/5 border border-black/10 rounded-lg text-sm placeholder-black/40 focus:outline-none focus:border-blue-500"
            />
          </div>
          <button
            onClick={loadScripts}
            disabled={isLoadingList}
            className="px-3 py-1.5 bg-black/5 hover:bg-black/10 rounded-lg text-sm transition-colors disabled:opacity-50"
            title="刷新列表"
          >
            {isLoadingList ? '⏳' : '🔄'}
          </button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div className="px-3 py-2 bg-red-100 border-b border-red-200 text-red-700 text-sm flex items-center justify-between">
          <span>{error}</span>
          <button onClick={() => setError(null)} className="text-red-400 hover:text-red-300">
            ✕
          </button>
        </div>
      )}

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Script list */}
        <div className="w-64 border-r border-black/10 overflow-y-auto">
          {isLoadingList && scripts.length === 0 ? (
            <div className="p-4 text-center opacity-50">
              <div className="animate-pulse">加载中...</div>
            </div>
          ) : filteredScripts.length === 0 ? (
            <div className="p-4 text-center opacity-50">
              {searchQuery ? '没有匹配的脚本' : '没有找到脚本'}
            </div>
          ) : (
            filteredScripts.map((script) => (
              <button
                key={script.id}
                onClick={() => setSelectedScript(script)}
                className={`w-full p-3 text-left border-b border-gray-800 hover:bg-gray-800 transition-colors ${
                  selectedScript?.id === script.id ? 'bg-gray-800' : ''
                }`}
              >
                <div className="flex items-center gap-2">
                  <span className="text-lg">{getScriptTypeIcon(script.script_type)}</span>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-sm truncate">{script.name}</div>
                    <div className="text-xs text-gray-500 truncate">
                      {script.script_type}
                      {script.category && ` · ${script.category}`}
                    </div>
                  </div>
                </div>
              </button>
            ))
          )}
        </div>

        {/* Script detail / output */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {selectedScript ? (
            <>
              {/* Script info */}
              <div className="p-4 border-b border-gray-700">
                <h2 className="text-lg font-semibold">{selectedScript.name}</h2>
                {isLoadingDetail ? (
                  <div className="text-sm text-gray-400 mt-1 animate-pulse">加载详情...</div>
                ) : (
                  <p className="text-sm text-gray-400 mt-1">
                    {scriptDetail?.description || selectedScript.description || '暂无描述'}
                  </p>
                )}

                {/* Tags and metadata */}
                <div className="flex flex-wrap items-center gap-2 mt-3">
                  <span className="px-2 py-0.5 bg-gray-800 rounded text-xs">
                    {selectedScript.script_type}
                  </span>
                  {scriptDetail?.category && (
                    <span className="px-2 py-0.5 bg-blue-900/50 rounded text-xs text-blue-300">
                      {scriptDetail.category}
                    </span>
                  )}
                  {scriptDetail?.tags?.map((tag) => (
                    <span
                      key={tag}
                      className="px-2 py-0.5 bg-gray-700 rounded text-xs text-gray-400"
                    >
                      {tag}
                    </span>
                  ))}
                </div>

                {/* Path */}
                {scriptDetail && (
                  <div className="mt-2 text-xs text-gray-500 font-mono truncate">
                    📁 {scriptDetail.path}
                  </div>
                )}

                {/* Run button */}
                <button
                  onClick={handleRunScript}
                  disabled={isRunning}
                  className={`mt-3 px-4 py-1.5 rounded-lg text-sm font-medium transition-colors ${
                    isRunning
                      ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
                      : 'bg-blue-600 text-white hover:bg-blue-700'
                  }`}
                >
                  {isRunning ? '运行中...' : '▶ 运行脚本'}
                </button>
              </div>

              {/* Output */}
              <div className="flex-1 p-4 overflow-auto">
                {runResult ? (
                  <div className="space-y-3">
                    {/* Status header */}
                    <div className="flex items-center gap-2">
                      <span
                        className={`px-2 py-0.5 rounded text-xs font-medium ${
                          runResult.succeeded
                            ? 'bg-green-900/50 text-green-300'
                            : 'bg-red-900/50 text-red-300'
                        }`}
                      >
                        {runResult.succeeded ? '✓ 成功' : '✗ 失败'}
                      </span>
                      <span className="text-xs text-gray-500">
                        退出码: {runResult.exit_code}
                      </span>
                      {runResult.duration_ms !== undefined && (
                        <span className="text-xs text-gray-500">
                          耗时: {formatDuration(runResult.duration_ms)}
                        </span>
                      )}
                    </div>

                    {/* Output */}
                    {runResult.output && (
                      <div>
                        <div className="text-xs text-gray-500 mb-1">输出:</div>
                        <pre className="text-sm font-mono text-gray-300 whitespace-pre-wrap bg-gray-800/50 p-3 rounded-lg max-h-96 overflow-auto">
                          {runResult.output_meta?.truncated
                            ? truncateOutput(runResult.output, 2000) +
                              `\n\n... (输出已截断，共 ${runResult.output_meta.total_length} 字符)`
                            : runResult.output}
                        </pre>
                      </div>
                    )}

                    {/* Working dir */}
                    {runResult.working_dir && (
                      <div className="text-xs text-gray-500">
                        工作目录: {runResult.working_dir}
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="text-sm text-gray-500">
                    {isRunning ? (
                      <div className="flex items-center gap-2">
                        <div className="animate-spin">⏳</div>
                        <span>正在运行脚本...</span>
                      </div>
                    ) : (
                      '点击"运行脚本"执行'
                    )}
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="h-full flex items-center justify-center text-gray-500">
              <p>选择一个脚本查看详情</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
