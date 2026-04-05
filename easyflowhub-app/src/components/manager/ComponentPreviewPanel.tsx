/**
 * ComponentPreviewPanel - 组件样式预览面板
 * 展示可用的桌面组件，支持点击查看详情
 */

import { useState } from 'react';
import { createFolderWidget } from '../../lib/tauri/widget';
import { useAppearance } from '../../hooks/useAppearance';
import { ManagerPanelHeader } from './shared/ManagerPanelHeader';

interface ComponentInfo {
  id: string;
  name: string;
  description: string;
  icon: string;
  category: 'widget' | 'panel' | 'theme';
  preview: React.ReactNode;
  actionLabel: string;
  onAction: () => void;
}

// 组件预览子组件
function FolderWidgetPreview() {
  return (
    <div className="preview-widget">
      <div className="mini-folder">
        <div className="grid">
          {['📝', '🔢', '📁', '💻'].map((icon, i) => (
            <div key={i} className="cell">{icon}</div>
          ))}
        </div>
      </div>
    </div>
  );
}

function QuickNotePreview() {
  return (
    <div className="preview-widget">
      <div className="mini-note">
        <div className="title-bar" />
        <div className="content-lines">
          <div className="line" style={{ width: '80%' }} />
          <div className="line" style={{ width: '60%' }} />
          <div className="line" style={{ width: '70%' }} />
        </div>
      </div>
    </div>
  );
}

function ThemePreview({ type }: { type: 'dark' | 'light' }) {
  return (
    <div className="preview-widget">
      <div className={`mini-theme ${type}`}>
        <div className="header" />
        <div className="body">
          <div className="card" />
          <div className="card" />
        </div>
      </div>
    </div>
  );
}

function McpPreview() {
  return (
    <div className="preview-widget">
      <div className="mini-mcp">
        <div className="mini-mcp__header">MCP</div>
        <div className="mini-mcp__rows">
          <span />
          <span />
          <span />
        </div>
      </div>
    </div>
  );
}

const CATEGORY_LABELS: Record<string, string> = {
  widget: '小组件',
  panel: '面板',
  theme: '主题',
};

export function ComponentPreviewPanel() {
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [selectedComponent, setSelectedComponent] = useState<ComponentInfo | null>(null);
  const [actionLoading, setActionLoading] = useState(false);
  const [actionMessage, setActionMessage] = useState<string | null>(null);

  const { setTheme } = useAppearance();

  // 定义组件列表（包含操作）
  const getComponents = (): ComponentInfo[] => [
    {
      id: 'folder-widget',
      name: '文件夹组件',
      description: '类似手机文件夹，可收纳多个应用快捷方式。点击展开显示应用网格，点击应用快速启动。',
      icon: '📁',
      category: 'widget',
      preview: <FolderWidgetPreview />,
      actionLabel: '创建组件',
      onAction: async () => {
        await createFolderWidget();
        setActionMessage('组件已创建到桌面！');
      },
    },
    {
      id: 'quick-note',
      name: '快速笔记',
      description: '轻量级笔记窗口，支持 Markdown 编辑与图片预览。Ctrl+Alt+N 新建，Ctrl+Alt+D 关闭全部，Ctrl+Alt+H 显隐全部。',
      icon: '📝',
      category: 'panel',
      preview: <QuickNotePreview />,
      actionLabel: '新建笔记',
      onAction: async () => {
        // 快速笔记通过快捷键或托盘菜单创建
        setActionMessage('请使用 Ctrl+Alt+N 或托盘菜单创建快速笔记');
      },
    },
    {
      id: 'dark-theme',
      name: '深色主题',
      description: '护眼深色模式，适合夜间使用。降低屏幕亮度，减少眼睛疲劳。',
      icon: '🌙',
      category: 'theme',
      preview: <ThemePreview type="dark" />,
      actionLabel: '应用主题',
      onAction: async () => {
        setTheme('dark');
        setActionMessage('已切换到深色主题');
      },
    },
    {
      id: 'light-theme',
      name: '浅色主题',
      description: '明亮清新的界面风格，适合日间使用。',
      icon: '☀️',
      category: 'theme',
      preview: <ThemePreview type="light" />,
      actionLabel: '应用主题',
      onAction: async () => {
        setTheme('light');
        setActionMessage('已切换到浅色主题');
      },
    },
    {
      id: 'agent-mcp',
      name: 'Agent / MCP',
      description: '为脚本管理与 Agent 接入预留统一入口。当前先保留迁移位，后续接入能力时复用现有 scriptmgr-go MCP 实现。',
      icon: '🤖',
      category: 'panel',
      preview: <McpPreview />,
      actionLabel: '查看迁移位',
      onAction: async () => {
        setActionMessage('已预留 Agent / MCP 入口，建议下一步复用现有 scriptmgr-go MCP 服务。');
      },
    },
  ];

  const components = getComponents();

  const filteredComponents = selectedCategory === 'all'
    ? components
    : components.filter(c => c.category === selectedCategory);

  // 执行组件操作
  const handleAction = async (component: ComponentInfo) => {
    if (actionLoading) return;

    setActionLoading(true);
    setActionMessage(null);

    try {
      await component.onAction();
    } catch (error) {
      console.error('Action failed:', error);
      setActionMessage('操作失败，请重试');
    } finally {
      setActionLoading(false);
      // 3秒后清除消息
      setTimeout(() => setActionMessage(null), 3000);
    }
  };

  // 关闭详情面板
  const closeDetail = () => setSelectedComponent(null);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="border-b border-[color:var(--manager-border)] px-6 py-4">
        <ManagerPanelHeader
          kicker="Components"
          title="组件预览"
          description="浏览可用的桌面组件和主题样式，点击卡片查看详情"
        />
      </header>

      {/* Category Filter */}
      <div className="border-b border-[color:var(--manager-border)] px-6 py-3">
        <div className="flex gap-2">
          {['all', 'widget', 'panel', 'theme'].map((cat) => (
            <button
              key={cat}
              onClick={() => setSelectedCategory(cat)}
              className={`rounded-full px-4 py-1.5 text-sm transition-colors ${
                selectedCategory === cat
                  ? 'bg-[color:var(--manager-accent)] text-white'
                  : 'bg-white/50 text-[color:var(--manager-ink-soft)] hover:bg-white/70'
              }`}
            >
              {cat === 'all' ? '全部' : CATEGORY_LABELS[cat]}
            </button>
          ))}
        </div>
      </div>

      {/* Component Grid */}
      <main className="flex-1 overflow-auto p-6">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredComponents.map((component) => (
            <button
              key={component.id}
              onClick={() => setSelectedComponent(component)}
              className="group cursor-pointer rounded-xl border border-[color:var(--manager-border)] bg-white/40 p-4 text-left transition-all hover:border-[color:var(--manager-accent)] hover:bg-white/60 hover:shadow-lg"
            >
              {/* Preview Area */}
              <div className="mb-3 flex h-32 items-center justify-center rounded-lg bg-gray-100/80 transition-transform group-hover:scale-105">
                {component.preview}
              </div>

              {/* Info */}
              <div className="flex items-start gap-3">
                <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[color:var(--manager-accent-soft)] text-lg transition-transform group-hover:scale-110">
                  {component.icon}
                </span>
                <div className="min-w-0 flex-1">
                  <h3 className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    {component.name}
                  </h3>
                  <p className="mt-0.5 line-clamp-2 text-xs text-[color:var(--manager-ink-soft)]">
                    {component.description}
                  </p>
                  <span className="mt-2 inline-block rounded bg-gray-200/70 px-2 py-0.5 text-[10px] uppercase tracking-wider text-[color:var(--manager-ink-muted)]">
                    {CATEGORY_LABELS[component.category]}
                  </span>
                </div>
              </div>
            </button>
          ))}
        </div>
      </main>

      {/* Footer */}
      <footer className="border-t border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-6 py-3">
        <p className="text-center text-xs text-[color:var(--manager-ink-muted)]">
          共 {filteredComponents.length} 个组件可用
        </p>
      </footer>

      {/* Detail Modal */}
      {selectedComponent && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
          onClick={closeDetail}
        >
          <div
            className="w-[90%] max-w-md rounded-2xl border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-6 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div className="mb-4 flex items-start justify-between">
              <div className="flex items-center gap-3">
                <span className="flex h-12 w-12 items-center justify-center rounded-xl bg-[color:var(--manager-accent-soft)] text-2xl">
                  {selectedComponent.icon}
                </span>
                <div>
                  <h2 className="text-lg font-semibold text-[color:var(--manager-ink-strong)]">
                    {selectedComponent.name}
                  </h2>
                  <span className="text-xs text-[color:var(--manager-ink-muted)]">
                    {CATEGORY_LABELS[selectedComponent.category]}
                  </span>
                </div>
              </div>
              <button
                onClick={closeDetail}
                className="rounded-lg p-2 text-[color:var(--manager-ink-muted)] hover:bg-black/5 hover:text-[color:var(--manager-ink)]"
              >
                ✕
              </button>
            </div>

            {/* Preview */}
            <div className="mb-4 flex h-40 items-center justify-center rounded-xl bg-gray-100/80">
              {selectedComponent.preview}
            </div>

            {/* Description */}
            <p className="mb-6 text-sm leading-relaxed text-[color:var(--manager-ink-soft)]">
              {selectedComponent.description}
            </p>

            {/* Action Message */}
            {actionMessage && (
              <div className="mb-4 rounded-lg bg-[color:var(--manager-accent-soft)] px-4 py-2 text-sm text-[color:var(--manager-accent)]">
                {actionMessage}
              </div>
            )}

            {/* Action Button */}
            <button
              onClick={() => handleAction(selectedComponent)}
              disabled={actionLoading}
              className={`w-full rounded-xl py-3 text-sm font-medium transition-all ${
                actionLoading
                  ? 'bg-gray-200 text-gray-400 cursor-wait'
                  : 'bg-[color:var(--manager-accent)] text-white hover:opacity-90 active:scale-[0.98]'
              }`}
            >
              {actionLoading ? '处理中...' : selectedComponent.actionLabel}
            </button>
          </div>
        </div>
      )}

      <style>{`
        .preview-widget {
          transform: scale(0.8);
        }

        /* Folder Widget Preview */
        .mini-folder {
          width: 60px;
          height: 60px;
          background: rgba(30, 30, 30, 0.9);
          backdrop-filter: blur(10px);
          border-radius: 12px;
          padding: 6px;
        }
        .mini-folder .grid {
          display: grid;
          grid-template-columns: repeat(2, 1fr);
          gap: 3px;
        }
        .mini-folder .cell {
          width: 22px;
          height: 22px;
          background: rgba(255, 255, 255, 0.15);
          border-radius: 4px;
          display: flex;
          align-items: center;
          justify-content: center;
          font-size: 10px;
        }

        /* Quick Note Preview */
        .mini-note {
          width: 70px;
          height: 50px;
          background: rgba(255, 255, 255, 0.95);
          border-radius: 6px;
          border: 1px solid rgba(0, 0, 0, 0.1);
          overflow: hidden;
        }
        .mini-note .title-bar {
          height: 10px;
          background: linear-gradient(90deg, #3b82f6, #8b5cf6);
        }
        .mini-note .content-lines {
          padding: 4px;
        }
        .mini-note .line {
          height: 3px;
          background: rgba(0, 0, 0, 0.1);
          margin-bottom: 3px;
          border-radius: 1px;
        }

        /* Theme Preview */
        .mini-theme {
          width: 60px;
          height: 45px;
          border-radius: 6px;
          overflow: hidden;
        }
        .mini-theme.dark {
          background: #1f2937;
        }
        .mini-theme.light {
          background: #f9fafb;
        }
        .mini-theme .header {
          height: 12px;
          background: rgba(59, 130, 246, 0.5);
        }
        .mini-theme .body {
          padding: 4px;
          display: flex;
          gap: 4px;
        }
        .mini-theme .card {
          flex: 1;
          height: 20px;
          border-radius: 3px;
        }
        .mini-theme.dark .card {
          background: rgba(255, 255, 255, 0.1);
        }
        .mini-theme.light .card {
          background: rgba(0, 0, 0, 0.05);
        }

        .mini-mcp {
          width: 74px;
          border-radius: 10px;
          border: 1px solid rgba(101, 114, 85, 0.2);
          background: linear-gradient(180deg, rgba(255,255,255,0.95), rgba(239, 232, 219, 0.9));
          overflow: hidden;
        }

        .mini-mcp__header {
          padding: 6px 8px;
          background: rgba(101, 114, 85, 0.14);
          font-size: 9px;
          letter-spacing: 0.24em;
          color: #4f5a43;
        }

        .mini-mcp__rows {
          padding: 8px;
          display: flex;
          flex-direction: column;
          gap: 5px;
        }

        .mini-mcp__rows span {
          display: block;
          height: 5px;
          border-radius: 999px;
          background: linear-gradient(90deg, rgba(101, 114, 85, 0.55), rgba(101, 114, 85, 0.16));
        }

        .line-clamp-2 {
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }
      `}</style>
    </div>
  );
}
