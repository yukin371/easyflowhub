/**
 * ScriptMgr HTTP API Client
 * Connects to scriptmgr-go HTTP server (default: localhost:8765)
 */

import type {
  TaskInfo,
  ListResponse,
  DescribeResponse,
} from '../../types/scriptmgr';

// ============================================================================
// Configuration
// ============================================================================

const DEFAULT_BASE_URL = 'http://localhost:8765';

let baseUrl = DEFAULT_BASE_URL;

export function setBaseUrl(url: string): void {
  baseUrl = url;
}

export function getBaseUrl(): string {
  return baseUrl;
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

async function fetchJSON<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, {
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
    return fetchJSON<ListResponse>(`${baseUrl}/api/scripts${qs({ search })}`);
  },

  /**
   * Get script detail by ID
   */
  get: async (scriptId: string): Promise<DescribeResponse> => {
    return fetchJSON<DescribeResponse>(`${baseUrl}/api/scripts/${encodeURIComponent(scriptId)}`);
  },

  /**
   * Update script category
   */
  updateCategory: async (scriptId: string, category: string): Promise<DescribeResponse> => {
    return fetchJSON<DescribeResponse>(`${baseUrl}/api/scripts/${encodeURIComponent(scriptId)}`, {
      method: 'PATCH',
      body: JSON.stringify({ category }),
    });
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
    return fetchJSON<TasksListResponse>(`${baseUrl}/api/tasks${qs({ status, limit })}`);
  },

  /**
   * Get task by ID
   */
  get: async (taskId: string): Promise<TaskInfo> => {
    return fetchJSON<TaskInfo>(`${baseUrl}/api/tasks/${encodeURIComponent(taskId)}`);
  },

  /**
   * Read task log
   */
  log: async (
    taskId: string,
    options?: { offset?: number; limit?: number; tail?: boolean }
  ): Promise<TaskLogResponse> => {
    return fetchJSON<TaskLogResponse>(
      `${baseUrl}/api/tasks/${encodeURIComponent(taskId)}/log${qs(options ?? {})}`
    );
  },

  /**
   * Cancel a task
   */
  cancel: async (taskId: string): Promise<{ ok: boolean }> => {
    return fetchJSON<{ ok: boolean }>(`${baseUrl}/api/cancel/${encodeURIComponent(taskId)}`, {
      method: 'POST',
    });
  },

  /**
   * Run a script
   */
  run: async (req: RunScriptRequest): Promise<RunScriptResponse> => {
    return fetchJSON<RunScriptResponse>(`${baseUrl}/api/run`, {
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
    return fetchJSON<{ ok: boolean; categories: MCPCategoryInfo[] }>(
      `${baseUrl}/api/mcp/categories`
    ).then((r) => r.categories);
  },

  /**
   * Load a category (mark as loaded for MCP)
   */
  loadCategory: async (category: string): Promise<{ ok: boolean; category: string; loaded: boolean }> => {
    return fetchJSON<{ ok: boolean; category: string; loaded: boolean }>(
      `${baseUrl}/api/mcp/load/${encodeURIComponent(category)}`,
      { method: 'POST' }
    );
  },

  /**
   * Unload a category (mark as unloaded)
   */
  unloadCategory: async (category: string): Promise<{ ok: boolean; category: string; loaded: boolean }> => {
    return fetchJSON<{ ok: boolean; category: string; loaded: boolean }>(
      `${baseUrl}/api/mcp/unload/${encodeURIComponent(category)}`,
      { method: 'POST' }
    );
  },
};

// ============================================================================
// Health Check
// ============================================================================

export async function checkServerHealth(): Promise<{ ok: boolean; message: string }> {
  try {
    const response = await fetch(`${baseUrl}/api/scripts?search=`, {
      method: 'HEAD',
    });
    return { ok: response.ok, message: response.ok ? 'Server is running' : 'Server error' };
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    if (message.includes('fetch') || message.includes('ECONNREFUSED')) {
      return {
        ok: false,
        message: 'ScriptMgr 服务未启动，请先运行 scriptmgr serve',
      };
    }
    return { ok: false, message };
  }
}
