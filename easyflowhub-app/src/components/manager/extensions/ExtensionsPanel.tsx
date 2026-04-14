import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { extensionsApi, mcpApi } from '../../../lib/api/scriptmgr';
import type {
  EffectiveExtensionContributions,
  ExtensionsResponse,
  MCPServerCatalogEntry,
} from '../../../types/scriptmgr';
import {
  MANAGER_OPEN_EXTENSION_EVENT,
  type ManagerExtensionNavigationDetail,
} from '../shared/extensionNavigation';

interface ExtensionsData {
  roots: string[];
  extensions: ExtensionsResponse['extensions'];
  contributions: EffectiveExtensionContributions | null;
  servers: MCPServerCatalogEntry[];
}

export function ExtensionsPanel() {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [data, setData] = useState<ExtensionsData>({
    roots: [],
    extensions: [],
    contributions: null,
    servers: [],
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [highlightedExtensionId, setHighlightedExtensionId] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);

    const [extensionsResult, contributionsResult, serversResult] = await Promise.allSettled([
      extensionsApi.list(),
      extensionsApi.contributions(),
      mcpApi.listServers(),
    ]);

    const nextData: ExtensionsData = {
      roots: [],
      extensions: [],
      contributions: null,
      servers: [],
    };
    let nextError: string | null = null;

    if (extensionsResult.status === 'fulfilled') {
      nextData.extensions = extensionsResult.value.extensions;
      nextData.roots = extensionsResult.value.roots;
    } else {
      nextError =
        extensionsResult.reason instanceof Error ? extensionsResult.reason.message : '加载扩展列表失败';
    }

    if (contributionsResult.status === 'fulfilled') {
      nextData.contributions = contributionsResult.value.contributions;
      if (nextData.roots.length === 0) {
        nextData.roots = contributionsResult.value.roots;
      }
    } else if (!nextError) {
      nextError =
        contributionsResult.reason instanceof Error
          ? contributionsResult.reason.message
          : '加载扩展贡献失败';
    }

    if (serversResult.status === 'fulfilled') {
      nextData.servers = serversResult.value;
    }

    setError(nextError);
    setData(nextData);
    setLoading(false);
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  useEffect(() => {
    const handleOpenExtension = (event: Event) => {
      const detail = (event as CustomEvent<ManagerExtensionNavigationDetail>).detail;
      if (!detail?.extensionId) {
        return;
      }

      setHighlightedExtensionId(detail.extensionId);
      window.requestAnimationFrame(() => {
        const target = rootRef.current?.querySelector<HTMLElement>(
          `[data-extension-id="${detail.extensionId}"]`
        );
        target?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      });
    };

    window.addEventListener(MANAGER_OPEN_EXTENSION_EVENT, handleOpenExtension as EventListener);
    return () => {
      window.removeEventListener(MANAGER_OPEN_EXTENSION_EVENT, handleOpenExtension as EventListener);
    };
  }, []);

  const loadedExtensions = useMemo(
    () => data.extensions.filter((item) => item.status === 'loaded').length,
    [data.extensions]
  );
  const totalScriptRoots = data.contributions?.script_roots?.length ?? 0;
  const totalManagerEntries = data.contributions?.manager_modules?.length ?? 0;
  const totalRelayOverlay =
    (data.contributions?.relay_providers?.length ?? 0) +
    (data.contributions?.relay_routes?.length ?? 0);
  const totalExtensionMcpServers = data.servers.filter((item) => item.status === 'extension').length;
  const conflictedMcpServers = data.servers.filter((item) => item.status === 'conflicted').length;
  const managerEntryAuditItems = useMemo(
    () =>
      (data.contributions?.manager_modules ?? []).map((entry) => {
        const mappedExtension = data.extensions.find(
          (extension) => extension.manifest?.id === entry.source.extension_id
        );
        const auditStatus = mappedExtension?.status === 'loaded' ? 'loaded' : 'unresolved';

        return {
          entry,
          auditStatus,
        };
      }),
    [data.contributions?.manager_modules, data.extensions]
  );

  return (
    <section ref={rootRef} className="flex h-full flex-col gap-4 px-4 py-4">
      <header className="space-y-1">
        <p className="manager-kicker">Extensions</p>
        <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[32px] leading-[1.04] text-[color:var(--manager-ink-strong)]">
          Contributions Catalog
        </h2>
        <p className="text-sm leading-6 text-[color:var(--manager-ink-soft)]">
          统一查看 manifest 扩展、effective contribution overlay，以及它们在 relay / scripts / MCP / manager 中的当前落点。
        </p>
      </header>

      <div className="grid grid-cols-2 gap-3 xl:grid-cols-5">
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Extensions</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">
            {loadedExtensions} / {data.extensions.length}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">已加载 / 总扩展</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Script Roots</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">{totalScriptRoots}</p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">effective overlay</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Relay Overlay</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">{totalRelayOverlay}</p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">provider + route</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">MCP Servers</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">
            {totalExtensionMcpServers} / {conflictedMcpServers}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">extension / conflicted</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
            Manager Entry Audit
          </p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">{totalManagerEntries}</p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">只读目录，操作入口仅保留在侧边栏</p>
        </div>
      </div>

      {error && (
        <div className="rounded-[16px] border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {error}
        </div>
      )}

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[0.9fr_1.1fr]">
        <section className="min-h-0 overflow-auto rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
          <div className="mb-3 flex items-center justify-between">
            <div>
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                Extension Roots
              </h3>
              <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                当前 registry 扫描目录。effective contributions 来自这些根目录中的已加载 manifest。
              </p>
            </div>
            <button
              onClick={() => void loadData()}
              className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
            >
              刷新
            </button>
          </div>
          <div className="space-y-2">
            {data.roots.map((root) => (
              <div
                key={root}
                className="rounded-[14px] border border-[color:var(--manager-border)] bg-[rgba(248,244,237,0.7)] px-4 py-3"
              >
                <p className="break-all font-mono text-xs text-[color:var(--manager-ink-soft)]">{root}</p>
              </div>
            ))}
            {!loading && data.roots.length === 0 && (
              <div className="rounded-[16px] border border-dashed border-[color:var(--manager-border)] bg-white/25 px-4 py-6 text-center text-sm text-[color:var(--manager-ink-subtle)]">
                当前没有 extension roots。
              </div>
            )}
          </div>

          <div className="mt-4 space-y-3">
            <div>
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                Manager Entry Audit
              </h3>
              <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                这里只保留扩展声明、来源与受控映射审计；不再提供第二个 manager entry 可操作入口。
              </p>
            </div>
            {managerEntryAuditItems.map(({ entry, auditStatus }) => (
              <div
                key={`${entry.source.extension_id}:${entry.id}`}
                className="rounded-[14px] border border-[color:var(--manager-border)] bg-white/40 px-4 py-3"
              >
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <p className="font-medium text-[color:var(--manager-ink-strong)]">{entry.name}</p>
                    <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">
                      {entry.caption || entry.id} · source {entry.source.extension_id}
                    </p>
                  </div>
                  <span className="rounded-full border border-[color:var(--manager-border)] bg-white/70 px-3 py-1 text-xs text-[color:var(--manager-ink-soft)]">
                    {entry.icon || 'EX'}
                  </span>
                </div>
                {entry.description && (
                  <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                    {entry.description}
                  </p>
                )}
                <dl className="mt-3 grid gap-2 text-xs text-[color:var(--manager-ink-subtle)] sm:grid-cols-2">
                  <div>
                    <dt className="uppercase tracking-[0.14em]">host status</dt>
                    <dd className="mt-1 text-[color:var(--manager-ink-muted)]">只读审计，不提供直接操作</dd>
                  </div>
                  <div>
                    <dt className="uppercase tracking-[0.14em]">controlled host</dt>
                    <dd className="mt-1 text-[color:var(--manager-ink-muted)]">
                      ManagerSidebar → ManagerExtensionEntries
                    </dd>
                  </div>
                  <div>
                    <dt className="uppercase tracking-[0.14em]">audit status</dt>
                    <dd className="mt-1 text-[color:var(--manager-ink-muted)]">
                      {auditStatus === 'loaded' ? '来源扩展已加载' : '来源扩展当前未解析'}
                    </dd>
                  </div>
                  <div>
                    <dt className="uppercase tracking-[0.14em]">target policy</dt>
                    <dd className="mt-1 text-[color:var(--manager-ink-muted)]">
                      仅允许受控宿主切换到既有 builtin 面板
                    </dd>
                  </div>
                </dl>
              </div>
            ))}
            {!loading && managerEntryAuditItems.length === 0 && (
              <div className="rounded-[16px] border border-dashed border-[color:var(--manager-border)] bg-white/25 px-4 py-6 text-center text-sm text-[color:var(--manager-ink-subtle)]">
                当前没有 manager entry audit 条目。
              </div>
            )}
          </div>
        </section>

        <section className="min-h-0 overflow-auto rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
          <div className="space-y-4">
            <div>
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                Extensions
              </h3>
              <div className="mt-3 space-y-3">
                {data.extensions.map((extension) => (
                  <div
                    key={extension.manifest_path}
                    data-extension-id={extension.manifest?.id}
                    className={`rounded-[14px] border px-4 py-3 transition ${
                      highlightedExtensionId && extension.manifest?.id === highlightedExtensionId
                        ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] shadow-[0_18px_40px_rgba(46,58,49,0.12)]'
                        : 'border-[color:var(--manager-border)] bg-white/40'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <p className="font-medium text-[color:var(--manager-ink-strong)]">
                          {extension.manifest?.name ?? extension.manifest_path}
                        </p>
                        <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">
                          {extension.manifest?.id ?? 'invalid'} · {extension.manifest?.version ?? 'n/a'}
                        </p>
                      </div>
                      <span
                        className={`rounded-full px-3 py-1 text-xs ${
                          extension.status === 'loaded'
                            ? 'bg-emerald-100 text-emerald-700'
                            : 'bg-red-100 text-red-700'
                        }`}
                      >
                        {extension.status}
                      </span>
                    </div>
                    {extension.manifest?.description && (
                      <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                        {extension.manifest.description}
                      </p>
                    )}
                    {extension.error && <p className="mt-2 text-xs text-red-700">{extension.error}</p>}
                  </div>
                ))}
              </div>
            </div>

            <div>
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                Script Root Overlay
              </h3>
              <div className="mt-3 space-y-2">
                {(data.contributions?.script_roots ?? []).map((item) => (
                  <div
                    key={`${item.source.extension_id}:${item.path}`}
                    className="rounded-[14px] border border-[color:var(--manager-border)] bg-[rgba(248,244,237,0.7)] px-4 py-3"
                  >
                    <p className="break-all font-mono text-xs text-[color:var(--manager-ink-soft)]">
                      {item.path}
                    </p>
                    <p className="mt-1 text-xs text-[color:var(--manager-ink-subtle)]">
                      source {item.source.extension_id}
                    </p>
                  </div>
                ))}
              </div>
            </div>

            <div>
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                MCP Server Overlay
              </h3>
              <div className="mt-3 space-y-2">
                {data.servers.map((server) => (
                  <div
                    key={server.key}
                    className="rounded-[14px] border border-[color:var(--manager-border)] bg-white/40 px-4 py-3"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <p className="font-medium text-[color:var(--manager-ink-strong)]">{server.name}</p>
                        <p className="mt-1 break-all font-mono text-xs text-[color:var(--manager-ink-muted)]">
                          {server.command} {server.args?.join(' ') ?? ''}
                        </p>
                      </div>
                      <span
                        className={`rounded-full px-3 py-1 text-xs ${
                          server.status === 'persisted'
                            ? 'bg-emerald-100 text-emerald-700'
                            : server.status === 'extension'
                              ? 'bg-sky-100 text-sky-700'
                              : 'bg-amber-100 text-amber-700'
                        }`}
                      >
                        {server.status}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-[color:var(--manager-ink-subtle)]">
                      source {server.source}
                    </p>
                    {server.conflict_with && (
                      <p className="mt-2 text-xs text-amber-700">conflict with {server.conflict_with}</p>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </div>
        </section>
      </div>
    </section>
  );
}
