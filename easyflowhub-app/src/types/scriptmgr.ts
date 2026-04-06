// ============================================================================
// Scriptmgr Types - Matching scriptmgr-go output
// ============================================================================

export interface ScriptSummary {
  id: string;
  name: string;
  path: string;
  script_type: string;
  source_root?: string;
  description?: string;
  category?: string;
  tags?: string[];
}

export interface ScriptParameter {
  name: string;
  type: string;
  label: string;
  default?: string;
  required: boolean;
  description?: string;
}

export interface ScriptDetail {
  id: string;
  name: string;
  path: string;
  script_type: string;
  source_root?: string;
  description?: string;
  category?: string;
  tags?: string[];
  author?: string;
  version?: string;
  icon?: string;
  parameters?: ScriptParameter[];
}

export interface OutputMeta {
  truncated: boolean;
  preview: string;
  total_length: number;
  line_count: number;
  log_path?: string;
}

export interface RunResult {
  script_id: string;
  script_name: string;
  command?: string[];
  exit_code: number;
  status: string;
  succeeded: boolean;
  output?: string;
  output_meta?: OutputMeta;
  working_dir?: string;
  started_at?: string;
  finished_at?: string;
  duration_ms?: number;
}

export interface HistoryEntry {
  history_id: string;
  script_id: string;
  script_name: string;
  command?: string[];
  status: string;
  exit_code?: number;
  working_dir?: string;
  started_at: string;
  finished_at?: string;
  duration_ms?: number;
  output_path?: string;
  output?: string;
}

export interface SessionInfo {
  session_id: string;
  script_id: string;
  script_name: string;
  command?: string[];
  pid: number;
  status: string;
  working_dir?: string;
  output_path?: string;
  started_at: string;
  finished_at?: string;
  exit_code?: number;
  duration_ms?: number;
  last_checked?: string;
}

export interface TaskInfo {
  task_id: string;
  script_id: string;
  status: string;
  created_at: string;
  started_at?: string;
  completed_at?: string;
  exit_code?: number;
  duration_ms?: number;
  input_json?: string;
  output_path?: string;
  output_summary?: string;
  error?: string;
}

// ============================================================================
// API Response Types
// ============================================================================

export interface ListResponse {
  ok: boolean;
  count: number;
  search?: string;
  roots: string[];
  scripts: ScriptSummary[];
  generated_at: string;
}

export interface DescribeResponse {
  ok: boolean;
  script: ScriptDetail;
  generated_at: string;
}

export interface HistoryResponse {
  ok: boolean;
  count: number;
  entries: HistoryEntry[];
  generated_at: string;
}

export interface SessionsResponse {
  ok: boolean;
  count: number;
  sessions: SessionInfo[];
  generated_at: string;
}

export interface FavoritesResponse {
  ok: boolean;
  count: number;
  favorites: ScriptSummary[];
  generated_at: string;
}

export interface RootsResponse {
  ok: boolean;
  count: number;
  roots: string[];
  generated_at: string;
}

// ============================================================================
// Relay / Extensions
// ============================================================================

export interface RelayProvider {
  id: string;
  name: string;
  base_url: string;
  api_key?: string;
  api_key_env?: string;
  source?: string;
  weight?: number;
  enabled: boolean;
  model_patterns?: string[];
  headers?: Record<string, string>;
  timeout_ms?: number;
}

export interface RelayRoute {
  id: string;
  name?: string;
  source?: string;
  path_prefixes?: string[];
  model_patterns?: string[];
  provider_ids?: string[];
  strategy?: string;
}

export interface RelayConfig {
  version: number;
  default_route?: string;
  providers?: RelayProvider[];
  routes?: RelayRoute[];
}

export interface RelayProviderStatus {
  provider_id: string;
  healthy: boolean;
  consecutive_failures: number;
  last_status_code?: number;
  last_error?: string;
  last_picked_at?: string;
  last_success_at?: string;
  last_failure_at?: string;
}

export interface RelayProviderSnapshot {
  provider: RelayProvider;
  status: RelayProviderStatus;
}

export interface ExtensionContributions {
  relay_providers?: Array<{
    id: string;
    name: string;
    base_url: string;
    weight?: number;
    model_patterns?: string[];
    headers?: Record<string, string>;
  }>;
  relay_routes?: Array<{
    id: string;
    name?: string;
    model_patterns?: string[];
    path_prefixes?: string[];
    provider_ids?: string[];
    strategy?: string;
  }>;
  script_roots?: Array<{
    path: string;
    description?: string;
  }>;
  mcp_servers?: Array<{
    id: string;
    name: string;
    command: string;
    args?: string[];
    description?: string;
  }>;
  manager_modules?: Array<{
    id: string;
    name: string;
    caption?: string;
    icon?: string;
    description?: string;
  }>;
}

export interface ExtensionManifest {
  id: string;
  name: string;
  version: string;
  description?: string;
  author?: string;
  homepage?: string;
  contributions?: ExtensionContributions;
}

export interface ListedExtension {
  manifest_path: string;
  root: string;
  status: string;
  error?: string;
  manifest?: ExtensionManifest;
}

export interface RelaySnapshot {
  config: RelayConfig;
  providers: RelayProviderSnapshot[];
  extension_roots?: string[];
  extensions?: ListedExtension[];
  extension_error?: string;
}

export interface RelaySnapshotResponse {
  ok: boolean;
  snapshot: RelaySnapshot;
}

export interface ExtensionsResponse {
  ok: boolean;
  roots: string[];
  count: number;
  extensions: ListedExtension[];
}
