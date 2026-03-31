/**
 * Script Store - React Context based state management
 */

import { createContext, useContext, useState, useCallback, useMemo, type ReactNode } from 'react';
import type { ScriptSummary, ScriptDetail, TaskInfo } from '../../../types/scriptmgr';
import { scriptsApi, tasksApi, categoriesApi, type CategoryInfo } from '../../../lib/api/scriptmgr';

// ============================================================================
// Types
// ============================================================================

interface ScriptStoreState {
  // Data
  scripts: ScriptSummary[];
  categories: CategoryInfo[];
  selectedCategory: string | null;
  selectedScriptId: string | null;
  selectedScriptDetail: ScriptDetail | null;

  // UI State
  drawerOpen: boolean;
  searchQuery: string;
  loading: boolean;
  detailLoading: boolean;
  error: string | null;

  // Recent tasks for selected script
  recentTasks: TaskInfo[];
}

interface ScriptStoreActions {
  // Data fetching
  fetchScripts: () => Promise<void>;
  fetchScriptDetail: (scriptId: string) => Promise<void>;
  fetchRecentTasks: (scriptId: string, limit?: number) => Promise<void>;

  // Selection
  setSelectedCategory: (category: string | null) => void;
  selectScript: (scriptId: string | null) => void;

  // UI
  openDrawer: (scriptId: string) => void;
  closeDrawer: () => void;
  setSearchQuery: (query: string) => void;
  clearError: () => void;
}

interface ScriptStore extends ScriptStoreState, ScriptStoreActions {
  // Computed
  filteredScripts: ScriptSummary[];
}

// ============================================================================
// Context
// ============================================================================

const ScriptStoreContext = createContext<ScriptStore | null>(null);

// ============================================================================
// Provider
// ============================================================================

export function ScriptStoreProvider({ children }: { children: ReactNode }) {
  // State
  const [scripts, setScripts] = useState<ScriptSummary[]>([]);
  const [categories, setCategories] = useState<CategoryInfo[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [selectedScriptId, setSelectedScriptId] = useState<string | null>(null);
  const [selectedScriptDetail, setSelectedScriptDetail] = useState<ScriptDetail | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recentTasks, setRecentTasks] = useState<TaskInfo[]>([]);

  // Computed: filtered scripts
  const filteredScripts = useMemo(() => {
    let result = scripts;

    // Filter by category
    if (selectedCategory) {
      result = result.filter((s) => {
        if (selectedCategory === 'uncategorized') {
          return !s.category;
        }
        return s.category === selectedCategory;
      });
    }

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter(
        (s) =>
          s.name.toLowerCase().includes(query) ||
          s.description?.toLowerCase().includes(query) ||
          s.tags?.some((t) => t.toLowerCase().includes(query))
      );
    }

    return result;
  }, [scripts, selectedCategory, searchQuery]);

  // Actions
  const fetchScripts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [scriptsResponse, categoriesData] = await Promise.all([
        scriptsApi.list(),
        categoriesApi.list(),
      ]);
      setScripts(scriptsResponse.scripts);
      setCategories(categoriesData);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch scripts';
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchScriptDetail = useCallback(async (scriptId: string) => {
    setDetailLoading(true);
    try {
      const response = await scriptsApi.get(scriptId);
      setSelectedScriptDetail(response.script);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch script detail';
      console.error(message);
    } finally {
      setDetailLoading(false);
    }
  }, []);

  const fetchRecentTasks = useCallback(async (scriptId: string, limit = 5) => {
    try {
      const response = await tasksApi.list(undefined, 100);
      // Filter by script_id and take recent ones
      const scriptTasks = response.tasks
        .filter((t) => t.script_id === scriptId)
        .slice(0, limit);
      setRecentTasks(scriptTasks);
    } catch (err) {
      console.error('Failed to fetch recent tasks:', err);
      setRecentTasks([]);
    }
  }, []);

  const selectScript = useCallback((scriptId: string | null) => {
    setSelectedScriptId(scriptId);
    if (!scriptId) {
      setSelectedScriptDetail(null);
      setRecentTasks([]);
    }
  }, []);

  const openDrawer = useCallback(
    (scriptId: string) => {
      setSelectedScriptId(scriptId);
      setDrawerOpen(true);
      // Fetch detail and recent tasks
      fetchScriptDetail(scriptId);
      fetchRecentTasks(scriptId);
    },
    [fetchScriptDetail, fetchRecentTasks]
  );

  const closeDrawer = useCallback(() => {
    setDrawerOpen(false);
  }, []);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const value: ScriptStore = {
    // State
    scripts,
    categories,
    selectedCategory,
    selectedScriptId,
    selectedScriptDetail,
    drawerOpen,
    searchQuery,
    loading,
    detailLoading,
    error,
    recentTasks,
    // Computed
    filteredScripts,
    // Actions
    fetchScripts,
    fetchScriptDetail,
    fetchRecentTasks,
    setSelectedCategory,
    selectScript,
    openDrawer,
    closeDrawer,
    setSearchQuery,
    clearError,
  };

  return <ScriptStoreContext.Provider value={value}>{children}</ScriptStoreContext.Provider>;
}

// ============================================================================
// Hook
// ============================================================================

export function useScriptStore(): ScriptStore {
  const context = useContext(ScriptStoreContext);
  if (!context) {
    throw new Error('useScriptStore must be used within ScriptStoreProvider');
  }
  return context;
}
