import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ActionRegistry } from './registry';
import type { Action, ActionStorage } from './types';
import { scriptSummaryToAction } from './adapters';
import { SNOW_SHOT_ACTION } from './builtins';

function storage(seed?: string): ActionStorage {
  const values = new Map<string, string>(seed ? [['actions', seed]] : []);
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

const script: Action = {
  id: 'script.format',
  name: 'Format text',
  kind: 'script',
  description: 'Format selected text with a script',
  keywords: ['pretty', 'format'],
  category: 'Text',
};
const note: Action = {
  id: 'note.capture',
  name: 'Capture note',
  kind: 'note',
  keywords: ['inbox'],
  category: 'Capture',
};
const url: Action = { id: 'url.open', name: 'Open URL', kind: 'url', enabled: true };

describe('ActionRegistry', () => {
  let registry: ActionRegistry;

  beforeEach(() => {
    registry = new ActionRegistry({ storage: storage(), storageKey: 'actions' });
    registry.register(script);
    registry.register(note);
    registry.register(url);
  });

  it('registers and searches actions by name, keywords and kind', () => {
    expect(registry.search({ query: 'pretty' })).toEqual([script]);
    expect(registry.search({ kind: 'note' })).toEqual([note]);
    expect(registry.search({ kind: ['script', 'url'], sort: 'alphabetical' })).toEqual([script, url]);
  });

  it('sorts favorites and recent actions for launcher surfaces', () => {
    expect(registry.setFavorite(url.id)).toBe(true);
    expect(registry.getFavorites()).toEqual([url]);
    registry.markRecent(note.id);
    registry.markRecent(script.id);
    expect(registry.getRecent()).toEqual([script, note]);
    expect(registry.search({ sort: 'recent' })).toEqual([script, note, url]);
  });

  it('persists only ids and restores them in a new registry', () => {
    const backing = storage();
    const first = new ActionRegistry({ storage: backing, storageKey: 'actions' });
    first.register(script);
    first.setFavorite(script.id);
    first.markRecent(script.id);

    const second = new ActionRegistry({ storage: backing, storageKey: 'actions' });
    second.register(script);
    expect(second.isFavorite(script.id)).toBe(true);
    expect(second.getRecent()).toEqual([script]);
  });

  it('normalizes executor success, missing executors and thrown errors', async () => {
    const now = vi.fn<() => Date>(() => new Date('2026-01-01T00:00:00.025Z'));
    now.mockReturnValueOnce(new Date('2026-01-01T00:00:00.000Z'));
    const executeRegistry = new ActionRegistry({ storage: undefined, now });
    executeRegistry.register(script, async (context, action) => ({
      ok: true,
      status: 'success',
      output: { kind: 'text', text: `${action.id}:${String(context.input)}` },
    }));
    const success = await executeRegistry.execute(script.id, { input: 'hello' });
    expect(success).toMatchObject({
      actionId: script.id,
      ok: true,
      status: 'success',
      output: { kind: 'text', text: 'script.format:hello' },
      durationMs: 25,
    });

    const missing = await executeRegistry.execute('unknown');
    expect(missing).toMatchObject({ ok: false, status: 'unavailable', actionId: 'unknown' });

    executeRegistry.register(note, () => {
      throw new Error('boom');
    });
    await expect(executeRegistry.execute(note.id)).resolves.toMatchObject({
      ok: false,
      status: 'error',
      error: 'boom',
    });
  });

  it('ignores malformed stored preferences and excludes disabled actions by default', () => {
    const malformed = new ActionRegistry({
      storage: {
        getItem: () => '{not-json',
        setItem: () => undefined,
      },
      storageKey: 'actions',
    });
    malformed.register({ ...url, enabled: false });
    expect(malformed.search()).toEqual([]);
    expect(malformed.search({ enabledOnly: false })).toEqual([{ ...url, enabled: false }]);
  });

  it('maps ScriptMgr metadata to a serializable script action', () => {
    expect(
      scriptSummaryToAction({
        id: 'abc',
        name: 'Compress files',
        path: 'C:/tools/compress.ps1',
        script_type: 'powershell',
        category: 'Files',
        tags: ['zip'],
      }),
    ).toMatchObject({
      id: 'script:abc',
      name: 'Compress files',
      kind: 'script',
      category: 'Files',
      metadata: { scriptId: 'abc', scriptType: 'powershell' },
    });
  });

  it('exposes Snow Shot as a searchable external capture action', () => {
    registry.register(SNOW_SHOT_ACTION);
    expect(registry.search({ query: 'snow shot' })).toEqual([SNOW_SHOT_ACTION]);
    expect(SNOW_SHOT_ACTION).toMatchObject({
      id: 'external.snow-shot.capture',
      kind: 'external',
      category: 'Capture',
      metadata: { provider: 'snow-shot' },
    });
  });
});
