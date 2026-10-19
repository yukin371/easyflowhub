/**
 * ScriptMgr HTTP API Client
 * Connects to scriptmgr-go HTTP server (default: localhost:8765)
 *
 * The native layer owns the sidecar lifecycle: `ensure_scriptmgr` starts
 * `scriptmgr serve` on demand and returns the base URL once it answers.
 * Every request first ensures the sidecar is up, so panels no longer fail
 * with "Failed to fetch" when nobody started the server manually.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  TaskInfo,
  ListResponse,
  DescribeResponse,
  RelayConfig,
  RelaySnapshot,
  RelaySnapshotResponse,
  ExtensionsResponse,
  ExtensionContributionsResponse,
  MCPServerCatalogEntry,
  MCPServerCatalogResponse,
  ScriptSource,
  RootsDetail,
  ScriptTrashEntry,
  ScriptTrashResponse,
  ImportScriptsResponse,
  ScriptSummary,
} from '../../types/scriptmgr';

// ============================================================================
// Configuration
// ============================================================================

const DEFAULT_BASE_URL = 'http://127.0.0.1:8765';
const DEFAULT_RELAY_RUNTIME_URL = 'http://127.0.0.1:8787';

let baseUrl = DEFAULT_BASE_URL;
let relayRuntimeUrl = DEFAULT_RELAY_RUNTIME_URL;

export function setBaseUrl(url: string): void {
  baseUrl = url;
}

export function getBaseUrl(): string {
  return baseUrl;
}

export function setRelayRuntimeUrl(url: string): void {
  relayRuntimeUrl = url;
}

export function getRelayRuntimeUrl(): string {
  return relayRuntimeUrl;
}

// ============================================================================
// Sidecar lifecycle
// ============================================================================

let ensurePromise: Promise<string> | undefined;

/**
 * Ensure the scriptmgr HTTP sidecar is running and return its base URL.
 * Concurrent callers share one attempt; a failed attempt clears the cached
 * promise so the next call (e.g. a panel retry button) tries again.
 */
export function ensureScriptmgr(): Promise<string> {
  if (!ensurePromise) {
    ensurePromise = invoke<string>('ensure_scriptmgr')
      .then((url) => {
        setBaseUrl(url);
        return url;
      })
      .catch((error) => {
        ensurePromise = undefined;
        throw error;
      });
  }
  return ensurePromise;
}

// ============================================================================
// Utilities
// ============================================================================

function qs(params: Record<string, unknown>): string {
  const searchParams = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== '') {
      searchParams.set(key, String(value));
    }
  }
  const str = searchParams.toString();
  return str ? `?${str}` : '';
}

/**
 * Go marshals a nil slice as `null`, so every list field can arrive as null
 * even though the TypeScript contract says array. Normalising here keeps the
 * panels from crashing on `.map` / `.filter` of a null list.
 */
function asArray<T>(value: T[] | null | undefined): T[] {
  return Array.isArray(value) ? value : [];
}

async function fetchJSON<T>(path: string, init?: RequestInit): Promise<T> {
  const base = await ensureScriptmgr();
  const response = await fetch(`${base}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...init?.headers,
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`HTTP ${response.status}: ${text}`);
  }

  return response.json();
}

// ============================================================================
// Scripts API
// ============================================================================

export const scriptsApi = {
  /**
   * List all scripts
   */
  list: async (search?: string): Promise<ListResponse> => {
    const response = await fetchJSON<ListResponse>(`/api/scripts${qs({ search })}`);
    return {
      ...response,
      roots: asArray(response.roots),
      scripts: asArray(response.scripts),
      count: response.count ?? 0,
    };
  },

  /**
   * Get script detail by ID
   */
  get: async (scriptId: string): Promise<DescribeResponse> => {
    return fetchJSON<DescribeResponse>(`/api/scripts/${encodeURIComponent(scriptId)}`);
  },

  /**
   * Update script category
   */
  updateCategory: async (scriptId: string, category: string): Promise<DescribeResponse> => {
    return fetchJSON<DescribeResponse>(`/api/scripts/${encodeURIComponent(scriptId)}`, {
      method: 'PATCH',
      body: JSON.stringify({ category }),
    });
  },

  /**
   * Read the editable source of a script
   */
  readSource: async (scriptId: string): Promise<ScriptSource> => {
    const response = await fetchJSON<{ ok: boolean; source: ScriptSource }>(
      `/api/scripts/${encodeURIComponent(scriptId)}/source`
    );
    return response.source;
  },

  /**
   * Save the source of a script in place
   */
  saveSource: async (scriptId: string, content: string): Promise<ScriptSource> => {
    const response = await fetchJSON<{ ok: boolean; source: ScriptSource }>(
      `/api/scripts/${encodeURIComponent(scriptId)}/source`,
      { method: 'PUT', body: JSON.stringify({ content }) }
    );
    return response.source;
  },

  /**
   * Create a new script inside a configured root
   */
  create: async (input: {
    root: string;
    filename: string;
    content: string;
    name?: string;
    category?: string;
    description?: string;
  }): Promise<ScriptSummary> => {
    const response = await fetchJSON<{ ok: boolean; script: ScriptSummary }>(`/api/scripts`, {
      method: 'POST',
      body: JSON.stringify(input),
    });
    return response.script;
  },

  /**
   * Move a script to the trash
   */
  remove: async (scriptId: string): Promise<ScriptTrashEntry> => {
    const response = await fetchJSON<{ ok: boolean; entry: ScriptTrashEntry }>(
      `/api/scripts/${encodeURIComponent(scriptId)}`,
      { method: 'DELETE' }
    );
    return response.entry;
  },

  /**
   * Copy external script files into a configured root
   */
  importFiles: async (root: string, paths: string[]): Promise<ImportScriptsResponse> => {
    return fetchJSON<ImportScriptsResponse>(`/api/scripts/import`, {
      method: 'POST',
      body: JSON.stringify({ root, paths }),
    });
  },
};

// ============================================================================
// Roots API
// ============================================================================

export const rootsApi = {
  list: async (): Promise<RootsDetail> => {
    const response = await fetchJSON<RootsDetail>(`/api/roots`);
    return { ...response, roots: asArray(response.roots), custom: asArray(response.custom) };
  },

  add: async (path: string): Promise<RootsDetail> => {
    const response = await fetchJSON<RootsDetail & { root: string }>(`/api/roots`, {
      method: 'POST',
      body: JSON.stringify({ path }),
    });
    return { ...response, roots: asArray(response.roots), custom: asArray(response.custom) };
  },

  remove: async (path: string): Promise<RootsDetail> => {
    const response = await fetchJSON<RootsDetail>(
      `/api/roots?path=${encodeURIComponent(path)}`,
      { method: 'DELETE' }
    );
    return { ...response, roots: asArray(response.roots), custom: asArray(response.custom) };
  },
};

// ============================================================================
// Script trash API
// ============================================================================

export const scriptTrashApi = {
  list: async (): Promise<ScriptTrashEntry[]> => {
    const response = await fetchJSON<ScriptTrashResponse>(`/api/trash/scripts`);
    return asArray(response.entries);
  },

  restore: async (entryId: string): Promise<ScriptTrashEntry> => {
    const response = await fetchJSON<{ ok: boolean; entry: ScriptTrashEntry }>(
      `/api/trash/scripts/${encodeURIComponent(entryId)}/restore`,
      { method: 'POST' }
    );
    return response.entry;
  },

  purge: async (entryId: string): Promise<void> => {
    await fetchJSON<{ ok: boolean }>(`/api/trash/scripts/${encodeURIComponent(entryId)}`, {
      method: 'DELETE',
    });
  },

  empty: async (): Promise<number> => {
    const response = await fetchJSON<{ ok: boolean; removed: number }>(`/api/trash/scripts`, {
      method: 'DELETE',
    });
    return response.removed;
  },
};

// ============================================================================
// Tasks API
// ============================================================================

export interface TasksListResponse {
  ok: boolean;
  count: number;
  status?: string;
  tasks: TaskInfo[];
  generated_at: string;
}

export interface TaskLogResponse {
  ok: boolean;
  task_id: string;
  task: TaskInfo;
  content: string;
  offset: number;
  limit: number;
  tail: boolean;
}

export interface RunScriptRequest {
  script_id: string;
  args?: string[];
  async?: boolean;
  dry_run?: boolean;
}

export interface RunScriptResponse {
  ok: boolean;
  task_id?: string;
  status: string;
  script_id: string;
  script_name: string;
  exit_code?: number;
  output?: string;
  output_meta?: {
    truncated: boolean;
    preview: string;
    total_length: number;
    line_count: number;
    log_path?: string;
  };
  duration_ms?: number;
}

export const tasksApi = {
  /**
   * List tasks with optional status filter
   */
  list: async (status?: string, limit?: number): Promise<TasksListResponse> => {
    const response = await fetchJSON<TasksListResponse>(`/api/tasks${qs({ status, limit })}`);
    return { ...response, tasks: asArray(response.tasks), count: response.count ?? 0 };
  },

  /**
   * Get task by ID
   */
  get: async (taskId: string): Promise<TaskInfo> => {
    return fetchJSON<TaskInfo>(`/api/tasks/${encodeURIComponent(taskId)}`);
  },

  /**
   * Read task log
   */
  log: async (
    taskId: string,
    options?: { offset?: number; limit?: number; tail?: boolean }
  ): Promise<TaskLogResponse> => {
    return fetchJSON<TaskLogResponse>(
      `/api/tasks/${encodeURIComponent(taskId)}/log${qs(options ?? {})}`
    );
  },

  /**
   * Cancel a task
   */
  cancel: async (taskId: string): Promise<{ ok: boolean }> => {
    return fetchJSON<{ ok: boolean }>(`/api/cancel/${encodeURIComponent(taskId)}`, {
      method: 'POST',
    });
  },

  /**
   * Run a script
   */
  run: async (req: RunScriptRequest): Promise<RunScriptResponse> => {
    return fetchJSON<RunScriptResponse>(`/api/run`, {
      method: 'POST',
      body: JSON.stringify(req),
    });
  },
};

// ============================================================================
// Categories API (derived from scripts)
// ============================================================================

export interface CategoryInfo {
  name: string;
  count: number;
}

export const categoriesApi = {
  /**
   * List all categories with script counts
   */
  list: async (): Promise<CategoryInfo[]> => {
    const response = await scriptsApi.list();
    const counts = new Map<string, number>();

    for (const script of response.scripts) {
      const category = script.category || 'uncategorized';
      counts.set(category, (counts.get(category) || 0) + 1);
    }

    // Sort by count descending, then by name
    const categories: CategoryInfo[] = [];
    for (const [name, count] of counts) {
      categories.push({ name, count });
    }
    categories.sort((a, b) => {
      if (b.count !== a.count) return b.count - a.count;
      return a.name.localeCompare(b.name);
    });

    return categories;
  },
};

// ============================================================================
// MCP Categories API
// ============================================================================

export interface MCPCategoryInfo {
  name: string;
  count: number;
  loaded: boolean;
}

export const mcpApi = {
  /**
   * List all MCP categories with their loaded status
   */
  listCategories: async (): Promise<MCPCategoryInfo[]> => {
    const response = await fetchJSON<{ ok: boolean; categories: MCPCategoryInfo[] }>(
      `/api/mcp/categories`
    );
    return asArray(response.categories);
  },

  listServers: async (): Promise<MCPServerCatalogEntry[]> => {
    const response = await fetchJSON<MCPServerCatalogResponse>(`/api/mcp/servers`);
    return asArray(response.catalog?.servers);
  },

  /**
   * Load a category (mark as loaded for MCP)
   */
  loadCategory: async (category: string): Promise<{ ok: boolean; category: string; loaded: boolean }> => {
    return fetchJSON<{ ok: boolean; category: string; loaded: boolean }>(
      `/api/mcp/load/${encodeURIComponent(category)}`,
      { method: 'POST' }
    );
  },

  /**
   * Unload a category (mark as unloaded)
   */
  unloadCategory: async (category: string): Promise<{ ok: boolean; category: string; loaded: boolean }> => {
    return fetchJSON<{ ok: boolean; category: string; loaded: boolean }>(
      `/api/mcp/unload/${encodeURIComponent(category)}`,
      { method: 'POST' }
    );
  },
};

// ============================================================================
// Relay / Extensions API
// ============================================================================

export const relayApi = {
  getSnapshot: async (): Promise<RelaySnapshot> => {
    const response = await fetchJSON<RelaySnapshotResponse>(`/api/relay/config`);
    return normalizeSnapshot(response.snapshot);
  },

  saveConfig: async (config: RelayConfig): Promise<RelaySnapshot> => {
    const response = await fetchJSON<RelaySnapshotResponse>(`/api/relay/config`, {
      method: 'PUT',
      body: JSON.stringify(config),
    });
    return normalizeSnapshot(response.snapshot);
  },
};

/** Go can send nil slices as null; panels assume arrays. */
function normalizeSnapshot(snapshot: RelaySnapshot | null | undefined): RelaySnapshot {
  const config = snapshot?.config ?? ({ version: 1 } as RelayConfig);
  return {
    ...snapshot,
    config,
    providers: asArray(snapshot?.providers),
    extension_roots: asArray(snapshot?.extension_roots),
    extensions: asArray(snapshot?.extensions),
  };
}

export const extensionsApi = {
  list: async (): Promise<ExtensionsResponse> => {
    const response = await fetchJSON<ExtensionsResponse>(`/api/extensions`);
    return {
      ...response,
      roots: asArray(response.roots),
      // A registry with zero manifests marshals as `extensions: null`.
      extensions: asArray(response.extensions),
      count: response.count ?? 0,
    };
  },

  contributions: async (): Promise<ExtensionContributionsResponse> => {
    const response = await fetchJSON<ExtensionContributionsResponse>(`/api/extensions/contributions`);
    const contributions = response.contributions ?? {};
    return {
      ...response,
      roots: asArray(response.roots),
      contributions: {
        ...contributions,
        manager_modules: asArray(contributions.manager_modules),
        script_roots: asArray(contributions.script_roots),
        relay_providers: asArray(contributions.relay_providers),
        relay_routes: asArray(contributions.relay_routes),
        mcp_servers: asArray(contributions.mcp_servers),
      },
    };
  },
};

// ============================================================================
// Health Check
// ============================================================================

export async function checkServerHealth(): Promise<{ ok: boolean; message: string }> {
  try {
    // Ensure the sidecar first, then verify with GET: the Go router matches
    // method-specific patterns, so a HEAD probe would be answered 405 even
    // while the server is healthy.
    const base = await ensureScriptmgr();
    const response = await fetch(`${base}/api/scripts?search=`);
    return { ok: response.ok, message: response.ok ? 'Server is running' : 'Server error' };
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    if (message.includes('fetch') || message.includes('ECONNREFUSED')) {
      return {
        ok: false,
        message: 'scriptmgr 服务启动失败，请重试或检查端口占用',
      };
    }
    return { ok: false, message };
  }
}

export async function checkRelayRuntimeHealth(): Promise<{ ok: boolean; message: string }> {
  let healthUrl: string;
  try {
    const url = new URL(relayRuntimeUrl.trim());
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) {
      throw new Error('invalid Relay URL');
    }
    url.pathname = `${url.pathname.replace(/\/+$/, '')}/health`;
    healthUrl = url.toString();
  } catch {
    return { ok: false, message: 'Relay 地址无效，请检查 Relay 配置。' };
  }

  try {
    const response = await fetch(healthUrl);
    return { ok: response.ok, message: response.ok ? 'Relay is running' : 'Relay error' };
  } catch {
    // Keep transport diagnostics and any URL data out of the UI. The detailed
    // failure remains available to the native/service logs when debugging.
    return {
      ok: false,
      message: 'Relay 未启动，请运行 scriptmgr relay serve --port 8787',
    };
  }
}
