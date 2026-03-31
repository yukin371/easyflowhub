/**
 * Scriptmgr IPC Wrapper + Helper Functions
 * Provides typed frontend API for scriptmgr sidecar commands
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  ListResponse,
  DescribeResponse,
  RunResult,
  ScriptDetail,
  ScriptSummary,
} from '../../types/scriptmgr';

// ============================================================================
// IPC Functions (Tauri Sidecar)
// ============================================================================

export interface ListScriptsOptions {
  search?: string;
}

export async function listScripts(options?: ListScriptsOptions): Promise<ScriptSummary[]> {
  const response: ListResponse = await invoke('list_scripts', {
    search: options?.search ?? null,
  });

  if (!response.ok) {
    throw new Error('Failed to list scripts');
  }

  return response.scripts;
}

export async function listScriptsWithMeta(
  options?: ListScriptsOptions
): Promise<ListResponse> {
  return await invoke('list_scripts', {
    search: options?.search ?? null,
  });
}

export async function describeScript(scriptId: string): Promise<ScriptDetail> {
  const response: DescribeResponse = await invoke('describe_script', {
    scriptId,
  });

  if (!response.ok) {
    throw new Error(`Failed to describe script: ${scriptId}`);
  }

  return response.script;
}

export interface RunScriptOptions {
  args?: string[];
  dryRun?: boolean;
}

export async function runScript(
  scriptId: string,
  options?: RunScriptOptions
): Promise<RunResult> {
  return await invoke('run_script', {
    scriptId,
    args: options?.args ?? null,
    dryRun: options?.dryRun ?? false,
  });
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Format duration in milliseconds to human readable string
 */
export function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms === null) return '-';

  if (ms < 1000) {
    return `${ms}ms`;
  } else if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)}s`;
  } else {
    const minutes = Math.floor(ms / 60000);
    const seconds = Math.round((ms % 60000) / 1000);
    return `${minutes}m ${seconds}s`;
  }
}

/**
 * Get status color for display
 * @returns 'green' | 'red' | 'blue' | 'gray'
 */
export function getStatusColor(status: string): 'green' | 'red' | 'blue' | 'gray' {
  switch (status.toLowerCase()) {
    case 'success':
    case 'succeeded':
    case 'completed':
      return 'green';
    case 'running':
    case 'pending':
    case 'started':
      return 'blue';
    case 'failed':
    case 'error':
    case 'cancelled':
      return 'red';
    default:
      return 'gray';
  }
}

/**
 * Truncate output for display
 */
export function truncateOutput(output: string | undefined, maxLength: number = 500): string {
  if (!output) return '';
  if (output.length <= maxLength) return output;
  return output.slice(0, maxLength) + '...';
}

/**
 * Format timestamp to relative time
 */
export function formatRelativeTime(timestamp: string | undefined): string {
  if (!timestamp) return '-';

  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return '刚刚';
  if (diffMins < 60) return `${diffMins}分钟前`;
  if (diffHours < 24) return `${diffHours}小时前`;
  if (diffDays < 7) return `${diffDays}天前`;

  return date.toLocaleDateString('zh-CN');
}
