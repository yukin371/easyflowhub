import type {
  Action,
  ActionExecutionContext,
  ActionExecutionResult,
  ActionExecutor,
  ActionHandlerResult,
  ActionKind,
  ActionRegistryOptions,
  ActionRegistrySnapshot,
  ActionSearchOptions,
  ActionSort,
  ActionStorage,
} from './types';

export const ACTION_REGISTRY_STORAGE_KEY = 'easyflowhub.action-registry.v1';
const DEFAULT_RECENT_LIMIT = 20;

function browserStorage(): ActionStorage | undefined {
  try {
    return typeof globalThis.localStorage === 'undefined' ? undefined : globalThis.localStorage;
  } catch {
    // Access to localStorage can throw in a restricted web view or when storage
    // is disabled. The registry remains useful as an in-memory catalog.
    return undefined;
  }
}

function cleanIds(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((id): id is string => typeof id === 'string' && id.length > 0);
}

function normalize(value: string): string {
  return value.trim().toLocaleLowerCase();
}

function matchScore(action: Action, query: string): number {
  if (!query) return 0;
  const name = normalize(action.name);
  if (name === query) return 1000;
  if (name.startsWith(query)) return 800;

  const fields = [
    action.description,
    ...(action.keywords ?? []),
    ...(action.tags ?? []),
    action.category,
  ]
    .filter((field): field is string => Boolean(field))
    .map(normalize);
  if (fields.some((field) => field === query)) return 650;
  if (name.includes(query)) return 600;
  if (fields.some((field) => field.startsWith(query))) return 450;
  if (fields.some((field) => field.includes(query))) return 300;
  return -1;
}

interface RegisteredAction {
  action: Action;
  executor?: ActionExecutor;
}

/**
 * In-memory action catalog with a tiny persisted usage index.
 *
 * Only ids are written to storage. Executors can therefore safely be native
 * functions, Tauri wrappers, or adapters supplied by a feature module.
 */
export class ActionRegistry {
  private readonly actions = new Map<string, RegisteredAction>();
  private readonly favorites = new Set<string>();
  private recent: string[] = [];
  private readonly storage?: ActionStorage;
  private readonly storageKey: string;
  private readonly recentLimit: number;
  private readonly now: () => Date;

  constructor(options: ActionRegistryOptions = {}) {
    this.storage = options.storage ?? browserStorage();
    this.storageKey = options.storageKey ?? ACTION_REGISTRY_STORAGE_KEY;
    this.recentLimit = Math.max(1, options.recentLimit ?? DEFAULT_RECENT_LIMIT);
    this.now = options.now ?? (() => new Date());
    this.load();
  }

  register(action: Action, executor?: ActionExecutor): void {
    if (!action.id.trim()) throw new Error('Action id cannot be empty');
    if (!action.name.trim()) throw new Error('Action name cannot be empty');
    this.actions.set(action.id, { action: { ...action }, executor });
  }

  registerMany(actions: Array<{ action: Action; executor?: ActionExecutor }>): void {
    actions.forEach(({ action, executor }) => this.register(action, executor));
  }

  unregister(actionId: string): boolean {
    const removed = this.actions.delete(actionId);
    if (removed) {
      this.favorites.delete(actionId);
      this.recent = this.recent.filter((id) => id !== actionId);
      this.persist();
    }
    return removed;
  }

  clear(): void {
    this.actions.clear();
    this.favorites.clear();
    this.recent = [];
    this.persist();
  }

  get(actionId: string): Action | undefined {
    return this.actions.get(actionId)?.action;
  }

  getAll(): Action[] {
    return Array.from(this.actions.values(), ({ action }) => action);
  }

  setFavorite(actionId: string, favorite = true): boolean {
    if (!this.actions.has(actionId)) return false;
    if (favorite) this.favorites.add(actionId);
    else this.favorites.delete(actionId);
    this.persist();
    return true;
  }

  toggleFavorite(actionId: string): boolean {
    if (!this.actions.has(actionId)) return false;
    const favorite = !this.favorites.has(actionId);
    this.setFavorite(actionId, favorite);
    return favorite;
  }

  isFavorite(actionId: string): boolean {
    return this.favorites.has(actionId);
  }

  getFavorites(): Action[] {
    return this.recentActions(this.favorites);
  }

  getRecent(limit = this.recentLimit): Action[] {
    return this.recent
      .slice(0, Math.max(0, limit))
      .map((id) => this.get(id))
      .filter((action): action is Action => action !== undefined);
  }

  markRecent(actionId: string): boolean {
    if (!this.actions.has(actionId)) return false;
    this.recent = [actionId, ...this.recent.filter((id) => id !== actionId)].slice(
      0,
      this.recentLimit
    );
    this.persist();
    return true;
  }

  search(options: ActionSearchOptions = {}): Action[] {
    const query = normalize(options.query ?? '');
    const kinds = options.kind
      ? new Set<ActionKind>(Array.isArray(options.kind) ? options.kind : [options.kind])
      : undefined;
    const recentIndex = new Map(this.recent.map((id, index) => [id, index]));

    const matches = this.getAll()
      .map((action) => ({ action, score: matchScore(action, query) }))
      .filter(({ action, score }) => {
        if (query && score < 0) return false;
        if (kinds && !kinds.has(action.kind)) return false;
        if (options.category && action.category !== options.category) return false;
        if (options.favoritesOnly && !this.favorites.has(action.id)) return false;
        if (options.enabledOnly !== false && action.enabled === false) return false;
        return true;
      });

    const sort = options.sort ?? 'relevance';
    matches.sort((a, b) => this.compare(a.action, b.action, a.score, b.score, sort, recentIndex));
    return matches.map(({ action }) => action);
  }

  async execute(actionId: string, context: ActionExecutionContext = {}): Promise<ActionExecutionResult> {
    const started = this.now();
    const registered = this.actions.get(actionId);
    let result: ActionHandlerResult;

    if (!registered) {
      result = { ok: false, status: 'unavailable', error: `Action "${actionId}" was not found.` };
    } else if (registered.action.enabled === false) {
      result = { ok: false, status: 'unavailable', error: `Action "${actionId}" is disabled.` };
    } else if (!registered.executor) {
      result = { ok: false, status: 'unavailable', error: `Action "${actionId}" has no executor.` };
    } else {
      this.markRecent(actionId);
      try {
        result = await registered.executor(context, registered.action);
      } catch (error) {
        result = {
          ok: false,
          status: 'error',
          error: error instanceof Error ? error.message : String(error),
        };
      }
    }

    const finished = this.now();
    return {
      ...result,
      actionId,
      startedAt: started.toISOString(),
      finishedAt: finished.toISOString(),
      durationMs: Math.max(0, finished.getTime() - started.getTime()),
    };
  }

  snapshot(): ActionRegistrySnapshot {
    return {
      version: 1,
      favorites: Array.from(this.favorites),
      recent: [...this.recent],
    };
  }

  restore(snapshot: Partial<ActionRegistrySnapshot>): void {
    this.favorites.clear();
    cleanIds(snapshot.favorites).forEach((id) => this.favorites.add(id));
    this.recent = cleanIds(snapshot.recent).slice(0, this.recentLimit);
    this.persist();
  }

  private recentActions(ids: Set<string>): Action[] {
    const recent = this.recent.filter((id) => ids.has(id));
    const rest = Array.from(ids).filter((id) => !recent.includes(id));
    return [...recent, ...rest]
      .map((id) => this.get(id))
      .filter((action): action is Action => action !== undefined);
  }

  private compare(
    a: Action,
    b: Action,
    scoreA: number,
    scoreB: number,
    sort: ActionSort,
    recentIndex: Map<string, number>
  ): number {
    if (sort === 'recent') {
      return (recentIndex.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (recentIndex.get(b.id) ?? Number.MAX_SAFE_INTEGER) ||
        a.name.localeCompare(b.name);
    }
    if (sort === 'favorites') {
      return Number(this.isFavorite(b.id)) - Number(this.isFavorite(a.id)) || a.name.localeCompare(b.name);
    }
    if (sort === 'alphabetical') return a.name.localeCompare(b.name);
    return scoreB - scoreA || Number(this.isFavorite(b.id)) - Number(this.isFavorite(a.id)) || a.name.localeCompare(b.name);
  }

  private load(): void {
    if (!this.storage) return;
    try {
      const raw = this.storage.getItem(this.storageKey);
      if (!raw) return;
      const parsed = JSON.parse(raw) as Partial<ActionRegistrySnapshot>;
      this.favorites.clear();
      cleanIds(parsed.favorites).forEach((id) => this.favorites.add(id));
      this.recent = cleanIds(parsed.recent).slice(0, this.recentLimit);
    } catch {
      // Corrupt user preferences should never prevent the launcher from opening.
      this.favorites.clear();
      this.recent = [];
    }
  }

  private persist(): void {
    if (!this.storage) return;
    try {
      this.storage.setItem(this.storageKey, JSON.stringify(this.snapshot()));
    } catch {
      // Storage can be full or unavailable. Keep the in-memory state usable.
    }
  }
}

export const actionRegistry = new ActionRegistry();
