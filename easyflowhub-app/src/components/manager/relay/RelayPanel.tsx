import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  checkRelayRuntimeHealth,
  checkServerHealth,
  extensionsApi,
  relayApi,
} from '../../../lib/api/scriptmgr';
import type { ListedExtension, RelayConfig, RelaySnapshot } from '../../../types/scriptmgr';
import {
  getRelayContributionSummary,
  hasRelayContributions,
  importRelayContributions,
} from './importRelayContributions';
import {
  getEffectiveRelayContributionSummary,
  getEffectiveRelayContributionTotals,
} from './effectiveRelayContributions';
import type { EffectiveExtensionContributions } from '../../../types/scriptmgr';
import { navigateToManagerExtension } from '../shared/extensionNavigation';

type ServiceStatus = 'checking' | 'online' | 'offline';

function formatDate(value?: string): string {
  if (!value) {
    return '未记录';
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

export function RelayPanel() {
  const [scriptmgrStatus, setScriptmgrStatus] = useState<ServiceStatus>('checking');
  const [relayStatus, setRelayStatus] = useState<ServiceStatus>('checking');
  const [snapshot, setSnapshot] = useState<RelaySnapshot | null>(null);
  const [extensions, setExtensions] = useState<ListedExtension[]>([]);
  const [effectiveContributions, setEffectiveContributions] =
    useState<EffectiveExtensionContributions | null>(null);
  const [extensionRoots, setExtensionRoots] = useState<string[]>([]);
  const [configText, setConfigText] = useState('{\n  "version": 1\n}');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  const loadData = useCallback(async (preserveEditor = false) => {
    setLoading(true);
    setError(null);

    const [scriptmgrHealth, relayHealth, snapshotResult, extensionsResult, contributionsResult] = await Promise.allSettled([
      checkServerHealth(),
      checkRelayRuntimeHealth(),
      relayApi.getSnapshot(),
      extensionsApi.list(),
      extensionsApi.contributions(),
    ]);

    if (scriptmgrHealth.status === 'fulfilled') {
      setScriptmgrStatus(scriptmgrHealth.value.ok ? 'online' : 'offline');
    } else {
      setScriptmgrStatus('offline');
    }

    if (relayHealth.status === 'fulfilled') {
      setRelayStatus(relayHealth.value.ok ? 'online' : 'offline');
    } else {
      setRelayStatus('offline');
    }

    if (snapshotResult.status === 'fulfilled') {
      setSnapshot(snapshotResult.value);
      if (!preserveEditor) {
        setConfigText(JSON.stringify(snapshotResult.value.config, null, 2));
      }
    } else {
      setError(snapshotResult.reason instanceof Error ? snapshotResult.reason.message : '加载 relay 配置失败');
    }

    if (extensionsResult.status === 'fulfilled') {
      setExtensions(extensionsResult.value.extensions);
      setExtensionRoots(extensionsResult.value.roots);
    } else if (snapshotResult.status === 'fulfilled') {
      setExtensions(snapshotResult.value.extensions ?? []);
      setExtensionRoots(snapshotResult.value.extension_roots ?? []);
    }

    if (contributionsResult.status === 'fulfilled') {
      setEffectiveContributions(contributionsResult.value.contributions);
      if (extensionsResult.status !== 'fulfilled') {
        setExtensionRoots(contributionsResult.value.roots);
      }
    } else {
      setEffectiveContributions(null);
    }

    setLoading(false);
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setSaveMessage(null);
    setError(null);

    try {
      const parsed = JSON.parse(configText) as RelayConfig;
      const nextSnapshot = await relayApi.saveConfig(parsed);
      setSnapshot(nextSnapshot);
      setConfigText(JSON.stringify(nextSnapshot.config, null, 2));
      setSaveMessage('relay 配置已保存');
    } catch (err) {
      setError(err instanceof Error ? err.message : '保存 relay 配置失败');
    } finally {
      setSaving(false);
    }
  }, [configText]);

  const handleImportRelayPreset = useCallback(
    (extension: ListedExtension) => {
      setSaveMessage(null);
      setError(null);

      try {
        let parsedConfig: RelayConfig;
        try {
          parsedConfig = JSON.parse(configText) as RelayConfig;
        } catch {
          throw new Error('当前编辑器中的 relay JSON 非法，请先修复后再导入扩展预设');
        }

        const result = importRelayContributions(parsedConfig, extension);
        const extensionLabel =
          extension.manifest?.name ?? extension.manifest?.id ?? extension.manifest_path;
        setConfigText(JSON.stringify(result.config, null, 2));
        setSaveMessage(
          `已从 ${extensionLabel} 导入 ${result.addedProviders.length} 个 provider、${result.addedRoutes.length} 个 route 到编辑器，点击保存配置生效`
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : '导入扩展 relay 预设失败');
      }
    },
    [configText]
  );

  const healthyProviders = useMemo(
    () => snapshot?.providers.filter((item) => item.status.healthy).length ?? 0,
    [snapshot]
  );

  const configuredRoutesCount = snapshot?.config.routes?.length ?? 0;
  const configuredProvidersCount = snapshot?.config.providers?.length ?? 0;
  const runtimeRoutesCount =
    snapshot?.effective_config?.routes?.length ?? snapshot?.config.routes?.length ?? 0;
  const runtimeProvidersCount =
    snapshot?.effective_config?.providers?.length ?? snapshot?.config.providers?.length ?? 0;
  const effectiveRelayTotals = useMemo(
    () => getEffectiveRelayContributionTotals(effectiveContributions),
    [effectiveContributions]
  );
  const loadedExtensions = extensions.filter((item) => item.status === 'loaded').length;

  return (
    <section className="flex h-full flex-col gap-4 px-4 py-4">
      <header className="space-y-1">
        <p className="manager-kicker">Relay</p>
        <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[32px] leading-[1.04] text-[color:var(--manager-ink-strong)]">
          API Relay & Extensions
        </h2>
        <p className="text-sm leading-6 text-[color:var(--manager-ink-soft)]">
          管理 OpenAI 兼容 relay 的 provider、route、基础 failover 配置，以及 manifest 扩展清单。
        </p>
      </header>

      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">ScriptMgr</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">
            {scriptmgrStatus === 'online' ? '在线' : scriptmgrStatus === 'offline' ? '离线' : '检测中'}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">配置接口来自 `scriptmgr serve`</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Relay Runtime</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">
            {relayStatus === 'online' ? '运行中' : relayStatus === 'offline' ? '未启动' : '检测中'}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">默认检查 `localhost:8787/health`</p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Providers / Routes</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">
            {runtimeProvidersCount} / {runtimeRoutesCount}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">
            运行时视图，健康 provider {healthyProviders} 个
          </p>
        </div>
        <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Extensions</p>
          <p className="mt-2 text-2xl text-[color:var(--manager-accent)]">
            {loadedExtensions} / {extensions.length}
          </p>
          <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">已加载 / 总扩展</p>
        </div>
      </div>

      {error && (
        <div className="rounded-[16px] border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">{error}</div>
      )}

      {saveMessage && (
        <div className="rounded-[16px] border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
          {saveMessage}
        </div>
      )}

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[1.2fr_0.8fr]">
        <div className="flex min-h-0 flex-col gap-4">
          <section className="rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
            <div className="flex items-start justify-between gap-4">
              <div>
                <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                  Relay Config
                </h3>
                <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                  直接编辑 `relay.json` 的结构。当前只支持 OpenAI 兼容路径、加权轮询和基础 failover；敏感密钥优先使用 `api_key_env`，避免继续写入明文 `api_key`。
                </p>
              </div>
              <div className="flex shrink-0 gap-2">
                <button
                  onClick={() => void loadData()}
                  className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                >
                  刷新
                </button>
                <button
                  onClick={() => void handleSave()}
                  disabled={saving}
                  className="rounded-full bg-[color:var(--manager-accent)] px-4 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)] disabled:opacity-60"
                >
                  {saving ? '保存中...' : '保存配置'}
                </button>
              </div>
            </div>

            <div className="mt-4 rounded-[16px] border border-[color:var(--manager-border)] bg-[rgba(246,241,233,0.72)] p-4">
              <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Quick Start</p>
              <pre className="mt-3 overflow-auto whitespace-pre-wrap break-all text-xs leading-6 text-[color:var(--manager-ink-soft)]">
{`scriptmgr serve
scriptmgr relay serve --port 8787`}
              </pre>
            </div>

            <textarea
              value={configText}
              onChange={(event) => setConfigText(event.target.value)}
              spellCheck={false}
              className="mt-4 min-h-[340px] w-full rounded-[18px] border border-[color:var(--manager-border)] bg-[rgba(248,244,237,0.86)] p-4 font-mono text-xs leading-6 text-[color:var(--manager-ink)] outline-none transition focus:border-[color:var(--manager-accent)]"
            />
          </section>

          <section className="min-h-0 flex-1 overflow-auto rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
            <div className="mb-3 flex items-center justify-between">
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                Provider Health
              </h3>
              <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                {loading ? '加载中...' : `${healthyProviders} / ${runtimeProvidersCount} healthy`}
              </p>
            </div>

            <div className="space-y-3">
              {(snapshot?.providers ?? []).map((item) => (
                <div
                  key={item.provider.id}
                  className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 px-4 py-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <p className="font-medium text-[color:var(--manager-ink-strong)]">{item.provider.name}</p>
                      <p className="mt-1 font-mono text-xs text-[color:var(--manager-ink-muted)]">
                        {item.provider.base_url}
                      </p>
                    </div>
                    <span
                      className={`rounded-full px-3 py-1 text-xs ${
                        item.status.healthy ? 'bg-emerald-100 text-emerald-700' : 'bg-amber-100 text-amber-700'
                      }`}
                    >
                      {item.status.healthy ? 'healthy' : 'degraded'}
                    </span>
                  </div>

                  <div className="mt-3 grid gap-2 text-xs text-[color:var(--manager-ink-soft)] md:grid-cols-2 xl:grid-cols-4">
                    <div>weight: {item.provider.weight ?? 1}</div>
                    <div>failures: {item.status.consecutive_failures}</div>
                    <div>last status: {item.status.last_status_code ?? 'n/a'}</div>
                    <div>picked: {formatDate(item.status.last_picked_at)}</div>
                  </div>

                  {item.status.last_error && <p className="mt-2 text-xs text-amber-700">{item.status.last_error}</p>}
                </div>
              ))}

              {!loading && (snapshot?.providers?.length ?? 0) === 0 && (
                <div className="rounded-[16px] border border-dashed border-[color:var(--manager-border)] bg-white/25 px-4 py-6 text-center text-sm text-[color:var(--manager-ink-subtle)]">
                  尚未配置 provider。先在上方 JSON 中补齐 `providers` 与 `routes`。
                </div>
              )}
            </div>
          </section>
        </div>

        <section className="min-h-0 overflow-auto rounded-[20px] border border-[color:var(--manager-border)] bg-white/55 p-5">
          <div className="mb-3 flex items-center justify-between">
            <div>
              <h3 className="text-sm font-medium uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
                Extensions
              </h3>
              <p className="mt-2 text-sm leading-6 text-[color:var(--manager-ink-soft)]">
                v1 只加载 manifest，不执行任意第三方代码。
              </p>
            </div>
            <button
              onClick={() => void loadData(true)}
              className="rounded-full border border-[color:var(--manager-border)] bg-white/60 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
            >
              重新扫描
            </button>
          </div>

          <div className="rounded-[16px] border border-[color:var(--manager-border)] bg-[rgba(248,244,237,0.7)] px-4 py-3">
            <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">Extension Roots</p>
            <div className="mt-2 space-y-1">
              {extensionRoots.map((root) => (
                <p key={root} className="break-all font-mono text-xs text-[color:var(--manager-ink-soft)]">
                  {root}
                </p>
              ))}
            </div>
          </div>

          <div className="mt-3 rounded-[16px] border border-[color:var(--manager-border)] bg-[rgba(248,244,237,0.7)] px-4 py-3">
            <p className="text-xs uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
              Effective Relay Overlay
            </p>
            <div className="mt-2 grid gap-2 text-xs text-[color:var(--manager-ink-soft)] md:grid-cols-2">
              <div>static config: provider {configuredProvidersCount} / route {configuredRoutesCount}</div>
              <div>
                extension overlay: provider {effectiveRelayTotals.providerCount} / route {effectiveRelayTotals.routeCount}
              </div>
            </div>
          </div>

          <div className="mt-4 space-y-3">
            {extensions.map((extension) => {
              const relaySummary = getRelayContributionSummary(extension);
              const effectiveRelaySummary = getEffectiveRelayContributionSummary(
                extension,
                effectiveContributions
              );
              const canImportRelay = hasRelayContributions(extension);

              return (
                <div
                  key={extension.manifest_path}
                  className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/40 px-4 py-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="font-medium text-[color:var(--manager-ink-strong)]">
                        {extension.manifest?.name ?? extension.manifest_path}
                      </p>
                      <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">
                        {extension.manifest?.id ?? 'invalid-extension'} · {extension.manifest?.version ?? 'n/a'}
                      </p>
                    </div>
                    <span
                      className={`rounded-full px-3 py-1 text-xs ${
                        extension.status === 'loaded' ? 'bg-emerald-100 text-emerald-700' : 'bg-red-100 text-red-700'
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

                  <div className="mt-3 grid gap-2 text-xs text-[color:var(--manager-ink-soft)] md:grid-cols-2">
                    <div>root: {extension.root}</div>
                    <div>
                      effective relay: provider {effectiveRelaySummary.providerCount} / route{' '}
                      {effectiveRelaySummary.routeCount}
                    </div>
                  </div>

                  {canImportRelay && (
                    <div className="mt-3 rounded-[14px] border border-[color:var(--manager-border)] bg-[rgba(248,244,237,0.75)] px-3 py-3">
                      <div className="flex items-start justify-between gap-3">
                        <div className="text-xs leading-6 text-[color:var(--manager-ink-soft)]">
                          <p className="uppercase tracking-[0.14em] text-[color:var(--manager-ink-subtle)]">
                            Relay Preset
                          </p>
                          <p className="mt-1">
                            manifest 中 provider {relaySummary.providerCount} 个，route {relaySummary.routeCount} 个。当前 effective overlay 为 provider {effectiveRelaySummary.providerCount} 个、route {effectiveRelaySummary.routeCount} 个；导入后只写入上方编辑器，默认新增 provider 为禁用状态。
                          </p>
                        </div>
                        <button
                          onClick={() => handleImportRelayPreset(extension)}
                          className="shrink-0 rounded-full border border-[color:var(--manager-accent)] bg-white px-4 py-2 text-sm text-[color:var(--manager-accent)] transition hover:bg-[color:var(--manager-accent)] hover:text-white"
                        >
                          导入到配置
                        </button>
                      </div>
                    </div>
                  )}

                  {extension.manifest?.id && (
                    <button
                      onClick={() => navigateToManagerExtension(extension.manifest!.id)}
                      className="mt-3 rounded-full border border-[color:var(--manager-border)] bg-white/70 px-4 py-2 text-xs text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                    >
                      查看扩展详情
                    </button>
                  )}

                  {extension.error && <p className="mt-2 text-xs text-red-700">{extension.error}</p>}
                </div>
              );
            })}

            {!loading && extensions.length === 0 && (
              <div className="rounded-[16px] border border-dashed border-[color:var(--manager-border)] bg-white/25 px-4 py-6 text-center text-sm text-[color:var(--manager-ink-subtle)]">
                当前未发现扩展 manifest。
              </div>
            )}
          </div>
        </section>
      </div>
    </section>
  );
}
