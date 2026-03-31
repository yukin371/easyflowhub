// src/components/widget/FolderWidget.tsx
/**
 * FolderWidget - 桌面文件夹组件
 *
 * 类似手机文件夹的桌面小组件：
 * - 收起状态：显示小图标网格 (2x2)
 * - 展开状态：宫格形式 (2x2 / 3x3)
 * - 可拖动
 * - 点击应用启动
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { getFolderApps, launchApp, setWidgetSize } from '../../lib/tauri/widget';
import type { AppShortcut, WidgetMode } from '../../types/widget';

// 组件尺寸配置
const COLLAPSED_SIZE = { width: 80, height: 80 };
const GRID_2X2_SIZE = { width: 160, height: 160 };  // 4 宫格
const GRID_3X3_SIZE = { width: 240, height: 240 };  // 9 宫格

// 从 URL 参数获取组件 ID
function getWidgetId(): string {
  const params = new URLSearchParams(window.location.search);
  return params.get('id') || 'folder-widget-0';
}

// 根据应用数量决定宫格大小
function getGridSize(appCount: number): { width: number; height: number; cols: number } {
  if (appCount <= 4) {
    return { ...GRID_2X2_SIZE, cols: 2 };
  }
  return { ...GRID_3X3_SIZE, cols: 3 };
}

export function FolderWidget() {
  const [apps, setApps] = useState<AppShortcut[]>([]);
  const [mode, setMode] = useState<WidgetMode>('collapsed');
  const [loading, setLoading] = useState(true);
  const [launching, setLaunching] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const widgetId = getWidgetId();

  // 计算宫格配置
  const gridConfig = useMemo(() => getGridSize(apps.length), [apps.length]);

  // 加载应用列表
  useEffect(() => {
    getFolderApps()
      .then(setApps)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  // 切换展开/收起
  const toggleMode = useCallback(async () => {
    const newMode = mode === 'collapsed' ? 'expanded' : 'collapsed';
    const newSize = newMode === 'collapsed' ? COLLAPSED_SIZE : gridConfig;

    // 先更新状态（触发动画）
    setMode(newMode);

    // 同步窗口大小
    try {
      await setWidgetSize(widgetId, newSize.width, newSize.height);
    } catch (error) {
      console.error('Failed to resize widget:', error);
    }
  }, [mode, widgetId, gridConfig]);

  // 启动应用
  const handleAppClick = useCallback(async (app: AppShortcut, e: React.MouseEvent) => {
    e.stopPropagation(); // 防止触发收起

    if (launching) return; // 防止重复点击

    setLaunching(app.id);
    try {
      await launchApp(app.execPath);
      // 启动成功后收起组件
      if (mode === 'expanded') {
        setTimeout(() => toggleMode(), 200);
      }
    } catch (error) {
      console.error('Failed to launch app:', error);
    } finally {
      setLaunching(null);
    }
  }, [launching, mode, toggleMode]);

  // 点击外部收起
  useEffect(() => {
    if (mode === 'collapsed') return;

    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        toggleMode();
      }
    };

    // 延迟添加监听器，避免立即触发
    const timer = setTimeout(() => {
      document.addEventListener('click', handleClickOutside);
    }, 100);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('click', handleClickOutside);
    };
  }, [mode, toggleMode]);

  if (loading) {
    return (
      <div className="folder-widget loading">
        <div className="spinner" />
      </div>
    );
  }

  // 收起状态：显示前 4 个应用图标
  const collapsedApps = apps.slice(0, 4);
  // 展开状态：根据宫格数量限制
  const expandedApps = apps.slice(0, gridConfig.cols * gridConfig.cols);

  return (
    <div
      ref={containerRef}
      className={`folder-widget ${mode}`}
      data-tauri-drag-region
      onClick={mode === 'collapsed' ? toggleMode : undefined}
    >
      {mode === 'collapsed' ? (
        // 收起状态：2x2 图标网格
        <div className="folder-collapsed" data-tauri-drag-region>
          <div className="icon-grid grid-2x2">
            {collapsedApps.map((app) => (
              <div key={app.id} className="icon-cell">
                <span className="app-icon">{app.icon}</span>
              </div>
            ))}
            {collapsedApps.length < 4 &&
              Array.from({ length: 4 - collapsedApps.length }).map((_, i) => (
                <div key={`empty-${i}`} className="icon-cell empty" />
              ))}
          </div>
        </div>
      ) : (
        // 展开状态：宫格布局
        <div className={`folder-expanded grid-${gridConfig.cols}x${gridConfig.cols}`}>
          {expandedApps.map((app) => (
            <button
              key={app.id}
              className={`app-cell ${launching === app.id ? 'launching' : ''}`}
              onClick={(e) => handleAppClick(app, e)}
              disabled={!!launching}
              title={app.name}
            >
              <span className="app-icon">{app.icon}</span>
              <span className="app-name">{app.name}</span>
            </button>
          ))}
          {/* 填充空位 */}
          {expandedApps.length < gridConfig.cols * gridConfig.cols &&
            Array.from({ length: gridConfig.cols * gridConfig.cols - expandedApps.length }).map((_, i) => (
              <div key={`empty-${i}`} className="app-cell empty" />
            ))}
        </div>
      )}

      <style>{`
        .folder-widget {
          width: 100%;
          height: 100%;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
          user-select: none;
          transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
        }

        .folder-widget.loading {
          display: flex;
          align-items: center;
          justify-content: center;
          background: transparent;
        }

        .spinner {
          width: 20px;
          height: 20px;
          border: 2px solid rgba(255, 255, 255, 0.2);
          border-top-color: #3b82f6;
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        @keyframes spin {
          to { transform: rotate(360deg); }
        }

        /* 收起状态 */
        .folder-collapsed {
          width: 100%;
          height: 100%;
          background: rgba(30, 30, 30, 0.85);
          backdrop-filter: blur(20px);
          border-radius: 18px;
          border: 1px solid rgba(255, 255, 255, 0.1);
          cursor: pointer;
          display: flex;
          align-items: center;
          justify-content: center;
          transition: all 0.2s ease;
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .folder-collapsed:hover {
          background: rgba(45, 45, 45, 0.95);
          border-color: rgba(255, 255, 255, 0.2);
          transform: scale(1.05);
          box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
        }

        .icon-grid {
          display: grid;
          gap: 6px;
          padding: 10px;
        }

        .icon-grid.grid-2x2 {
          grid-template-columns: repeat(2, 1fr);
        }

        .icon-cell {
          width: 26px;
          height: 26px;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(255, 255, 255, 0.12);
          border-radius: 7px;
          transition: background 0.15s ease;
        }

        .icon-cell.empty {
          background: rgba(255, 255, 255, 0.03);
        }

        .app-icon {
          font-size: 15px;
        }

        /* 展开状态 - 宫格布局 */
        .folder-expanded {
          width: 100%;
          height: 100%;
          background: rgba(25, 25, 25, 0.92);
          backdrop-filter: blur(30px);
          border-radius: 18px;
          border: 1px solid rgba(255, 255, 255, 0.15);
          display: grid;
          gap: 8px;
          padding: 12px;
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
        }

        .folder-expanded.grid-2x2 {
          grid-template-columns: repeat(2, 1fr);
          grid-template-rows: repeat(2, 1fr);
        }

        .folder-expanded.grid-3x3 {
          grid-template-columns: repeat(3, 1fr);
          grid-template-rows: repeat(3, 1fr);
        }

        .app-cell {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 4px;
          background: rgba(255, 255, 255, 0.08);
          border: none;
          border-radius: 12px;
          cursor: pointer;
          transition: all 0.15s ease;
          padding: 8px;
        }

        .app-cell:hover {
          background: rgba(255, 255, 255, 0.15);
          transform: scale(1.05);
        }

        .app-cell:active {
          transform: scale(0.95);
        }

        .app-cell.launching {
          opacity: 0.5;
          cursor: wait;
          animation: pulse 0.6s ease infinite;
        }

        @keyframes pulse {
          0%, 100% { opacity: 0.5; }
          50% { opacity: 0.7; }
        }

        .app-cell.empty {
          background: rgba(255, 255, 255, 0.02);
          cursor: default;
        }

        .app-cell.empty:hover {
          background: rgba(255, 255, 255, 0.02);
          transform: none;
        }

        .app-cell .app-icon {
          font-size: 24px;
        }

        .app-cell .app-name {
          font-size: 10px;
          color: rgba(255, 255, 255, 0.75);
          text-align: center;
          line-height: 1.2;
          max-width: 100%;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
      `}</style>
    </div>
  );
}
