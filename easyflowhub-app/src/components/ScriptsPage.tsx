import { useState } from 'react';

interface ScriptInfo {
  id: string;
  name: string;
  description: string;
  language: 'PowerShell' | 'Python' | 'Batch';
  path: string;
  category: string;
}

// Get icon based on script language
function getLanguageIcon(language: string): string {
  switch (language) {
    case 'PowerShell':
      return '💻';
    case 'Python':
      return '🐍';
    case 'Batch':
      return '📄';
    default:
      return '📜';
  }
}

// Mock data for scripts (will be replaced with real data from scriptmgr-go)
const mockScripts: ScriptInfo[] = [
  {
    id: 'demo_success',
    name: '成功示例脚本',
    description: '这是一个会成功执行的示例脚本',
    language: 'PowerShell',
    path: './PowerShell/demo_success.ps1',
    category: 'demo',
  },
  {
    id: 'demo_fail',
    name: '失败示例脚本',
    description: '这是一个会失败的示例脚本',
    language: 'PowerShell',
    path: './PowerShell/demo_fail.ps1',
    category: 'demo',
  },
  {
    id: 'demo_python',
    name: 'Python 示例',
    description: 'Python 脚本示例',
    language: 'Python',
    path: './Python/demo_python.py',
    category: 'demo',
  },
];

export function ScriptsPage() {
  const [scripts] = useState<ScriptInfo[]>(mockScripts);
  const [selectedScript, setSelectedScript] = useState<ScriptInfo | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [output, setOutput] = useState<string>('');
  const [searchQuery, setSearchQuery] = useState('');

  const filteredScripts = scripts.filter(
    (script) =>
      script.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      script.description.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleRunScript = async (script: ScriptInfo) => {
    setIsRunning(true);
    setOutput(`正在运行: ${script.name}...\n`);

    // Simulate script execution (will be replaced with real Tauri command)
    setTimeout(() => {
      setOutput((prev) => prev + `[完成] ${script.name} 执行完毕\n`);
      setIsRunning(false);
    }, 1500);
  };

  return (
    <div className="h-full flex flex-col bg-gray-900/80 text-gray-100">
      {/* Header with search */}
      <div className="p-3 border-b border-gray-700">
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
              className="w-full pl-9 pr-3 py-1.5 bg-gray-800 border border-gray-700 rounded-lg text-sm text-gray-100 placeholder-gray-500 focus:outline-none focus:border-blue-500"
            />
          </div>
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Script list */}
        <div className="w-64 border-r border-gray-700 overflow-y-auto">
          {filteredScripts.map((script) => (
            <button
              key={script.id}
              onClick={() => setSelectedScript(script)}
              className={`w-full p-3 text-left border-b border-gray-800 hover:bg-gray-800 transition-colors ${
                selectedScript?.id === script.id ? 'bg-gray-800' : ''
              }`}
            >
              <div className="flex items-center gap-2">
                <span className="text-lg">{getLanguageIcon(script.language)}</span>
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-sm truncate">{script.name}</div>
                  <div className="text-xs text-gray-500 truncate">
                    {script.language}
                  </div>
                </div>
              </div>
            </button>
          ))}
        </div>

        {/* Script detail / output */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {selectedScript ? (
            <>
              {/* Script info */}
              <div className="p-4 border-b border-gray-700">
                <h2 className="text-lg font-semibold">{selectedScript.name}</h2>
                <p className="text-sm text-gray-400 mt-1">
                  {selectedScript.description}
                </p>
                <div className="flex items-center gap-2 mt-3">
                  <span className="px-2 py-0.5 bg-gray-800 rounded text-xs">
                    {selectedScript.language}
                  </span>
                  <span className="text-xs text-gray-500">
                    {selectedScript.category}
                  </span>
                </div>
                <button
                  onClick={() => handleRunScript(selectedScript)}
                  disabled={isRunning}
                  className={`mt-3 px-4 py-1.5 rounded-lg text-sm font-medium transition-colors ${
                    isRunning
                      ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
                      : 'bg-blue-600 text-white hover:bg-blue-700'
                  }`}
                >
                  {isRunning ? '运行中...' : '运行脚本'}
                </button>
              </div>

              {/* Output */}
              <div className="flex-1 p-4 overflow-auto">
                <div className="text-xs text-gray-500 mb-2">输出:</div>
                <pre className="text-sm font-mono text-gray-300 whitespace-pre-wrap">
                  {output || '暂无输出'}
                </pre>
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
