import type { Capture } from '../../types/capture';

/**
 * Shared contract for actions exposed by the EasyFlowHub Launcher.
 *
 * Metadata is serializable. Runtime executors are registered separately on
 * ActionRegistry so actions can be persisted and restored safely.
 */
export type ActionKind =
  | 'script'
  | 'note'
  | 'todo'
  | 'url'
  | 'text'
  | 'file'
  | 'command'
  | 'external';

/** Where an action implementation is owned, for auditing and capability UI. */
export type ActionSource = 'builtin' | 'script' | 'external' | 'provider' | 'localService';

export type ActionInputKind = 'text' | 'url' | 'file' | 'files' | 'number' | 'boolean';
export type ActionInputSource =
  | 'explicit'
  | 'clipboard'
  | 'selection'
  | 'files'
  | 'currentFile'
  | 'screenshot'
  | 'window';

export type ActionPermission =
  | 'clipboard.read'
  | 'clipboard.write'
  | 'selection.read'
  | 'selection.write'
  | 'files.read'
  | 'files.write'
  | 'window.read'
  | 'network.request'
  | 'microphone.capture';

export type ActionOutputKind = 'clipboard' | 'inbox' | 'file' | 'notification' | 'next-action';

/** Serializable constraints for a user-supplied action parameter. */
export interface ActionParameterSchema {
  enum?: readonly (string | number | boolean)[];
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
}

export interface ActionInputSpec {
  id: string;
  label: string;
  kind: ActionInputKind;
  required?: boolean;
  placeholder?: string;
  source?: ActionInputSource;
  permissions?: ActionPermission[];
  schema?: ActionParameterSchema;
}

/** Serializable action metadata shared by built-in and contributed actions. */
export interface Action {
  id: string;
  name: string;
  kind: ActionKind;
  source?: ActionSource;
  description?: string;
  keywords?: string[];
  tags?: string[];
  category?: string;
  icon?: string;
  inputs?: ActionInputSpec[];
  outputs?: ActionOutputKind[];
  enabled?: boolean;
  /** Optional action-specific data (for example a script id or URL template). */
  metadata?: Record<string, unknown>;
}

export interface ActionFileContext {
  path: string;
  name?: string;
  mimeType?: string;
  sizeBytes?: number;
  modifiedAt?: string;
}

export interface ActionImageContext {
  mimeType: string;
  sizeBytes?: number;
  width?: number;
  height?: number;
}

export interface ActionScreenshotContext {
  source: 'clipboard' | 'external' | 'native';
  capturedAt: string;
  filePath?: string;
  image?: ActionImageContext;
}

export interface ActionWindowContext {
  processId?: number;
  processName?: string;
  executablePath?: string;
  title?: string;
}

export interface ActionClipboardContext {
  text?: string;
  image?: ActionImageContext;
}

/** Read-only snapshot of the desktop context available to an action. */
export interface ActionContextSnapshot {
  capturedAt?: string;
  capture?: Capture;
  clipboard?: ActionClipboardContext;
  selectedText?: string;
  currentFile?: ActionFileContext;
  files?: string[];
  screenshot?: ActionScreenshotContext;
  window?: ActionWindowContext;
}

/** Context automatically available to a Launcher invocation. */
export interface ActionExecutionContext extends ActionContextSnapshot {
  input?: unknown;
  params?: Record<string, unknown>;
  /** @deprecated Prefer `clipboard.text` in new executors. */
  clipboardText?: string;
  signal?: AbortSignal;
}

export type ActionExecutionStatus = 'success' | 'error' | 'cancelled' | 'unavailable';
export type ActionExecutionState = 'running' | ActionExecutionStatus;

/** Portable text result that Launcher can copy or collect without knowing the executor. */
export interface ActionTextOutput {
  kind: 'text';
  text: string;
  title?: string;
  sourceText?: string;
  targetLanguage?: string;
}

export interface ActionFileOutput {
  kind: 'file';
  path: string;
  mimeType?: string;
}

export interface ActionInboxOutput {
  kind: 'inbox';
  itemId: string;
}

export interface ActionNotificationOutput {
  kind: 'notification';
  title?: string;
  message: string;
}

export interface ActionNextOutput {
  kind: 'next-action';
  actionId: string;
  params?: Record<string, unknown>;
}

export type ActionOutput =
  | ActionTextOutput
  | ActionFileOutput
  | ActionInboxOutput
  | ActionNotificationOutput
  | ActionNextOutput;

export function isActionTextOutput(output: unknown): output is ActionTextOutput {
  if (!output || typeof output !== 'object') return false;
  const value = output as Partial<ActionTextOutput>;
  return value.kind === 'text' && typeof value.text === 'string' && Boolean(value.text.trim())
    && (value.title === undefined || typeof value.title === 'string')
    && (value.sourceText === undefined || typeof value.sourceText === 'string')
    && (value.targetLanguage === undefined || typeof value.targetLanguage === 'string');
}

/** Result returned by an action executor before registry timing fields are added. */
export interface ActionHandlerResult {
  ok: boolean;
  status: ActionExecutionStatus;
  output?: ActionOutput;
  message?: string;
  error?: string;
  retryable?: boolean;
}

/** Normalized result returned by ActionRegistry.execute. */
export interface ActionExecutionResult extends ActionHandlerResult {
  actionId: string;
  startedAt: string;
  finishedAt: string;
  durationMs: number;
}

export type ActionExecutor = (
  context: ActionExecutionContext,
  action: Action
) => ActionHandlerResult | Promise<ActionHandlerResult>;

export type ActionSort = 'relevance' | 'recent' | 'alphabetical' | 'favorites';

export interface ActionSearchOptions {
  query?: string;
  kind?: ActionKind | ActionKind[];
  category?: string;
  favoritesOnly?: boolean;
  enabledOnly?: boolean;
  sort?: ActionSort;
}

export interface ActionRegistrySnapshot {
  version: 1;
  favorites: string[];
  recent: string[];
}

export interface ActionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem?(key: string): void;
}

export interface ActionRegistryOptions {
  storage?: ActionStorage;
  storageKey?: string;
  recentLimit?: number;
  now?: () => Date;
}
