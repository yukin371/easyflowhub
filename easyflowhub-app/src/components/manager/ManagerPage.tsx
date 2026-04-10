/**
 * ManagerPage - 管理中心主页面
 * 动态渲染启用的功能模块
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ManagerSidebar } from './ManagerSidebar';
import { extensionsApi } from '../../lib/api/scriptmgr';
import type { EffectiveExtensionContributions } from '../../types/scriptmgr';
import { useEnabledModules } from '../../modules';
import {
  MANAGER_OPEN_EXTENSION_EVENT,
  MANAGER_NAVIGATE_TO_EXTENSION_EVENT,
  type ManagerExtensionNavigationDetail,
} from './shared/extensionNavigation';

export const MANAGER_ACTIVATED_EVENT = 'easyflowhub:manager-activated';
export const MANAGER_OPEN_NOTE_EVENT = 'easyflowhub:open-note';
export const MANAGER_NAVIGATE_TO_NOTE_EVENT = 'easyflowhub:navigate-to-note';

export function ManagerPage() {
  const enabledModules = useEnabledModules();
  const [activePanel, setActivePanel] = useState<string | null>(null);
  const [extensionEntries, setExtensionEntries] = useState<
    NonNullable<EffectiveExtensionContributions['manager_modules']>
  >([]);
  const [pendingExtensionNavigation, setPendingExtensionNavigation] = useState<string | null>(null);
  const [pendingNoteNavigation, setPendingNoteNavigation] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setActivePanel((currentPanel) => {
      if (enabledModules.length === 0) {
        return null;
      }

      if (!currentPanel) {
        return enabledModules[0].id;
      }

      if (!enabledModules.find((module) => module.id === currentPanel)) {
        return enabledModules[0].id;
      }

      return currentPanel;
    });
  }, [enabledModules]);

  useEffect(() => {
    let cancelled = false;

    const loadExtensionEntries = async () => {
      try {
        const response = await extensionsApi.contributions();
        if (!cancelled) {
          setExtensionEntries(response.contributions.manager_modules ?? []);
        }
      } catch (error) {
        if (!cancelled) {
          setExtensionEntries([]);
        }
        console.error('Failed to load manager extension entries:', error);
      }
    };

    void loadExtensionEntries();
    return () => {
      cancelled = true;
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
    const handleExtensionNavigation = (event: Event) => {
      const detail = (event as CustomEvent<ManagerExtensionNavigationDetail>).detail;
      if (!detail?.extensionId) {
        return;
      }

      setPendingExtensionNavigation(detail.extensionId);
      setActivePanel('extensions');
    };

    window.addEventListener(MANAGER_NAVIGATE_TO_EXTENSION_EVENT, handleExtensionNavigation as EventListener);
    return () => {
      window.removeEventListener(
        MANAGER_NAVIGATE_TO_EXTENSION_EVENT,
        handleExtensionNavigation as EventListener
      );
    };
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

  useEffect(() => {
    if (activePanel !== 'extensions' || !pendingExtensionNavigation) {
      return;
    }

    const timer = window.setTimeout(() => {
      window.dispatchEvent(
        new CustomEvent<ManagerExtensionNavigationDetail>(MANAGER_OPEN_EXTENSION_EVENT, {
          detail: { extensionId: pendingExtensionNavigation },
        })
      );
      setPendingExtensionNavigation(null);
    }, 16);

    return () => window.clearTimeout(timer);
  }, [activePanel, pendingExtensionNavigation]);

  const handlePanelChange = useCallback((panelId: string) => {
    setActivePanel(panelId);
  }, []);

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
          extensionEntries={extensionEntries}
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
