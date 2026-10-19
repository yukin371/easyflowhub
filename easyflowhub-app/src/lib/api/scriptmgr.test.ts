import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { checkRelayRuntimeHealth, setRelayRuntimeUrl } from './scriptmgr';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(command: string) => Promise<string>>(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const fetchMock = vi.fn<typeof fetch>();

beforeEach(() => {
  setRelayRuntimeUrl('http://127.0.0.1:8787');
  fetchMock.mockReset();
  invokeMock.mockReset();
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

async function loadClient() {
  vi.resetModules();
  return import('./scriptmgr');
}

describe('checkRelayRuntimeHealth', () => {
  it('checks the configured Relay base URL without duplicating slashes', async () => {
    setRelayRuntimeUrl('http://127.0.0.1:8787/');
    fetchMock.mockResolvedValue(new Response('', { status: 200 }));

    await expect(checkRelayRuntimeHealth()).resolves.toEqual({ ok: true, message: 'Relay is running' });
    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:8787/health');
  });

  it.each([
    'file:///secrets',
    'not-a-url',
    'http://user:secret@localhost:8787',
    'http://localhost:8787?key=secret',
  ])('rejects unsafe Relay URLs without fetching or echoing them (%s)', async (url) => {
    setRelayRuntimeUrl(url);

    await expect(checkRelayRuntimeHealth()).resolves.toEqual({
      ok: false,
      message: 'Relay 地址无效，请检查 Relay 配置。',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('maps transport failures to a stable recovery message', async () => {
    fetchMock.mockRejectedValue(new TypeError('Connection refused at http://user:secret@localhost:8787'));

    await expect(checkRelayRuntimeHealth()).resolves.toEqual({
      ok: false,
      message: 'Relay 未启动，请运行 scriptmgr relay serve --port 8787',
    });
  });
});

describe('sidecar lifecycle', () => {
  it('starts the sidecar once and shares the base URL across callers', async () => {
    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    const client = await loadClient();

    const [first, second] = await Promise.all([client.ensureScriptmgr(), client.ensureScriptmgr()]);
    expect(first).toBe('http://127.0.0.1:8765');
    expect(second).toBe(first);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('ensure_scriptmgr');
    expect(client.getBaseUrl()).toBe('http://127.0.0.1:8765');

    // A later call reuses the cached, already started sidecar.
    await client.ensureScriptmgr();
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('routes requests through the ensured base URL', async () => {
    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ ok: true, count: 0, scripts: [], generated_at: 'now' }), { status: 200 })
    );
    const client = await loadClient();

    await client.scriptsApi.list('demo');
    expect(invokeMock).toHaveBeenCalledWith('ensure_scriptmgr');
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:8765/api/scripts?search=demo',
      expect.objectContaining({
        headers: expect.objectContaining({ 'Content-Type': 'application/json' }),
      })
    );
  });

  it('retries a failed sidecar start on the next call', async () => {
    const client = await loadClient();

    invokeMock.mockRejectedValueOnce(new Error('spawn failed'));
    await expect(client.ensureScriptmgr()).rejects.toThrow('spawn failed');

    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    await expect(client.ensureScriptmgr()).resolves.toBe('http://127.0.0.1:8765');
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('health check ensures the sidecar and probes with GET', async () => {
    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    fetchMock.mockResolvedValue(new Response('', { status: 200 }));
    const client = await loadClient();

    await expect(client.checkServerHealth()).resolves.toEqual({ ok: true, message: 'Server is running' });
    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:8765/api/scripts?search=');
  });

  it('health check surfaces the sidecar startup error message', async () => {
    const client = await loadClient();
    invokeMock.mockRejectedValue(new Error('scriptmgr 服务启动后退出（端口 8765 可能被其他程序占用）'));

    await expect(client.checkServerHealth()).resolves.toEqual({
      ok: false,
      message: 'scriptmgr 服务启动后退出（端口 8765 可能被其他程序占用）',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe('wire format normalization', () => {
  it('turns Go null lists into empty arrays for every panel consumer', async () => {
    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    const client = await loadClient();

    // /api/extensions with zero manifests: extensions is null.
    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, count: 0, roots: null, extensions: null }), { status: 200 })
    );
    const extensions = await client.extensionsApi.list();
    expect(extensions.extensions).toEqual([]);
    expect(extensions.roots).toEqual([]);
    expect(extensions.count).toBe(0);

    // /api/extensions/contributions with no contributions: {} and null roots.
    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, roots: null, contributions: {} }), { status: 200 })
    );
    const contributions = await client.extensionsApi.contributions();
    expect(contributions.roots).toEqual([]);
    expect(contributions.contributions.manager_modules).toEqual([]);
    expect(contributions.contributions.script_roots).toEqual([]);
    expect(contributions.contributions.relay_providers).toEqual([]);
    expect(contributions.contributions.relay_routes).toEqual([]);
    expect(contributions.contributions.mcp_servers).toEqual([]);

    // /api/mcp/servers with an empty catalog.
    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, catalog: { servers: null } }), { status: 200 })
    );
    await expect(client.mcpApi.listServers()).resolves.toEqual([]);

    // /api/relay/config with null provider / root lists.
    fetchMock.mockImplementation(async () =>
      new Response(
        JSON.stringify({
          ok: true,
          snapshot: { config: { version: 1 }, providers: null, extension_roots: null },
        }),
        { status: 200 }
      )
    );
    const snapshot = await client.relayApi.getSnapshot();
    expect(snapshot.providers).toEqual([]);
    expect(snapshot.extension_roots).toEqual([]);
    expect(snapshot.config).toEqual({ version: 1 });
  });
});

describe('script file management endpoints', () => {
  it('routes source read/write and script CRUD to the file endpoints', async () => {
    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    const client = await loadClient();
    const source = {
      id: 's1',
      path: 'E:/scripts/demo.py',
      content: 'print(1)',
      language: 'python',
      size_bytes: 8,
      modified_at: '2026-10-19T00:00:00Z',
    };

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, source }), { status: 200 })
    );
    await expect(client.scriptsApi.readSource('s1')).resolves.toEqual(source);
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/scripts/s1/source',
      expect.anything()
    );

    await client.scriptsApi.saveSource('s1', 'print(2)');
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/scripts/s1/source',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify({ content: 'print(2)' }) })
    );

    const script = { id: 's2', name: 'demo', path: 'E:/scripts/demo2.py', script_type: 'python' };
    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, script }), { status: 200 })
    );
    await expect(
      client.scriptsApi.create({ root: 'E:/scripts', filename: 'demo2.py', content: '' })
    ).resolves.toEqual(script);
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/scripts',
      expect.objectContaining({ method: 'POST' })
    );

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, entry: { id: 'trash-1' } }), { status: 200 })
    );
    await expect(client.scriptsApi.remove('s2')).resolves.toEqual({ id: 'trash-1' });
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/scripts/s2',
      expect.objectContaining({ method: 'DELETE' })
    );

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, imported: [], failed: [] }), { status: 200 })
    );
    await client.scriptsApi.importFiles('E:/scripts', ['E:/incoming/a.py']);
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/scripts/import',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('manages storage roots and the script trash', async () => {
    invokeMock.mockResolvedValue('http://127.0.0.1:8765');
    const client = await loadClient();

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, roots: ['E:/scripts'], custom: ['E:/scripts'] }), {
        status: 200,
      })
    );
    await expect(client.rootsApi.list()).resolves.toMatchObject({ roots: ['E:/scripts'] });
    expect(fetchMock).toHaveBeenLastCalledWith('http://127.0.0.1:8765/api/roots', expect.anything());

    await client.rootsApi.add('E:/other');
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/roots',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ path: 'E:/other' }) })
    );

    await client.rootsApi.remove('E:/other');
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/roots?path=E%3A%2Fother',
      expect.objectContaining({ method: 'DELETE' })
    );

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, count: 1, entries: [{ id: 't1', name: 'demo' }] }), {
        status: 200,
      })
    );
    await expect(client.scriptTrashApi.list()).resolves.toEqual([{ id: 't1', name: 'demo' }]);

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, entry: { id: 't1' } }), { status: 200 })
    );
    await client.scriptTrashApi.restore('t1');
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/trash/scripts/t1/restore',
      expect.objectContaining({ method: 'POST' })
    );

    await client.scriptTrashApi.purge('t1');
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:8765/api/trash/scripts/t1',
      expect.objectContaining({ method: 'DELETE' })
    );

    fetchMock.mockImplementation(async () =>
      new Response(JSON.stringify({ ok: true, removed: 2 }), { status: 200 })
    );
    await expect(client.scriptTrashApi.empty()).resolves.toBe(2);
  });
});
