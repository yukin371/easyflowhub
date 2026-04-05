/**
 * ManagerPage - 管理中心主页面
 * 动态渲染启用的功能模块
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ManagerSidebar } from './ManagerSidebar';
import { moduleRegistry } from '../../modules';
import type { FeatureModule } from '../../modules';

export const MANAGER_ACTIVATED_EVENT = 'easyflowhub:manager-activated';
export const MANAGER_OPEN_NOTE_EVENT = 'easyflowhub:open-note';
export const MANAGER_NAVIGATE_TO_NOTE_EVENT = 'easyflowhub:navigate-to-note';

export function ManagerPage() {
  const [enabledModules, setEnabledModules] = useState<FeatureModule[]>([]);
  const [activePanel, setActivePanel] = useState<string | null>(null);
  const [pendingNoteNavigation, setPendingNoteNavigation] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  // 加载模块配置
  useEffect(() => {
    let mounted = true;

    const loadModules = async () => {
      try {
        await moduleRegistry.loadConfig();
      } catch (error) {
        console.error('Failed to load module config, using defaults:', error);
      }

      if (!mounted) return;

      const modules = moduleRegistry.getEnabledModules();
      setEnabledModules(modules);

      // 设置默认激活面板（第一个启用的模块）
      if (modules.length > 0) {
        setActivePanel((prev) => prev ?? modules[0].id);
      }
    };

    loadModules();

    // 订阅配置变更
    const unsubscribe = moduleRegistry.subscribe(() => {
      const modules = moduleRegistry.getEnabledModules();
      setEnabledModules(modules);

      // 如果当前激活的面板被禁用，切换到第一个启用的模块
      setActivePanel((currentPanel) => {
        if (currentPanel && !modules.find((m) => m.id === currentPanel)) {
          return modules.length > 0 ? modules[0].id : null;
        }
        return currentPanel;
      });
    });

    return () => {
      mounted = false;
      unsubscribe();
    };
  }, []);

  // 窗口焦点监听
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const notifyActivated = () => {
      window.dispatchEvent(new CustomEvent(MANAGER_ACTIVATED_EVENT));

      window.requestAnimationFrame(() => {
        rootRef.current?.focus();
      });
    };

    const setupWindowFocusListener = async () => {
      try {
        const win = await getCurrentWindow();
        unlisten = await win.onFocusChanged(({ payload: focused }) => {
          if (focused) {
            notifyActivated();
          }
        });
      } catch (error) {
        console.error('Failed to setup manager focus listener:', error);
      }
    };

    const handleWindowFocus = () => notifyActivated();
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        notifyActivated();
      }
    };

    void setupWindowFocusListener();
    window.addEventListener('focus', handleWindowFocus);
    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      unlisten?.();
      window.removeEventListener('focus', handleWindowFocus);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupNavigationListener = async () => {
      try {
        const win = getCurrentWindow();
        unlisten = await win.listen<{ noteId?: string }>(MANAGER_NAVIGATE_TO_NOTE_EVENT, (event) => {
          const noteId = event.payload?.noteId;
          if (!noteId) {
            return;
          }

          setPendingNoteNavigation(noteId);
          setActivePanel('notes');
        });
      } catch (error) {
        console.error('Failed to setup manager note navigation listener:', error);
      }
    };

    void setupNavigationListener();
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (activePanel !== 'notes' || !pendingNoteNavigation) {
      return;
    }

    const timer = window.setTimeout(() => {
      window.dispatchEvent(
        new CustomEvent(MANAGER_OPEN_NOTE_EVENT, {
          detail: { noteId: pendingNoteNavigation },
        })
      );
      setPendingNoteNavigation(null);
    }, 16);

    return () => window.clearTimeout(timer);
  }, [activePanel, pendingNoteNavigation]);

  const handlePanelChange = useCallback((panelId: string) => {
    setActivePanel(panelId);
  }, []);

  // 渲染当前激活的面板
  const renderPanel = () => {
    if (!activePanel) {
      return (
        <div className="flex h-full items-center justify-center text-[color:var(--manager-ink-muted)]">
          加载中...
        </div>
      );
    }

    const module = enabledModules.find((m) => m.id === activePanel);
    if (!module) {
      return (
        <div className="flex h-full items-center justify-center text-[color:var(--manager-ink-muted)]">
          模块未找到
        </div>
      );
    }

    const Component = module.component;
    return <Component />;
  };

  return (
    <div
      ref={rootRef}
      tabIndex={-1}
      className="manager-root h-full overflow-hidden px-2 py-2 text-[color:var(--manager-ink)] outline-none sm:px-3 sm:py-3"
    >
      <div className="flex h-full gap-3">
        <ManagerSidebar
          modules={enabledModules}
          activePanel={activePanel || ''}
          onPanelChange={handlePanelChange}
        />
        <main className="flex-1 overflow-hidden rounded-[20px] bg-[rgba(255,251,245,0.26)]">
          {renderPanel()}
        </main>
      </div>
    </div>
  );
}
