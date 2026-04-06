import type {
  ListedExtension,
  RelayConfig,
  RelayProvider,
  RelayRoute,
} from '../../../types/scriptmgr';

export interface RelayContributionSummary {
  providerCount: number;
  routeCount: number;
}

export interface RelayImportResult {
  config: RelayConfig;
  addedProviders: string[];
  addedRoutes: string[];
  sourceTag: string;
}

export function getRelayContributionSummary(extension: ListedExtension): RelayContributionSummary {
  return {
    providerCount: extension.manifest?.contributions?.relay_providers?.length ?? 0,
    routeCount: extension.manifest?.contributions?.relay_routes?.length ?? 0,
  };
}

export function hasRelayContributions(extension: ListedExtension): boolean {
  const summary = getRelayContributionSummary(extension);
  return summary.providerCount > 0 || summary.routeCount > 0;
}

export function importRelayContributions(
  config: RelayConfig,
  extension: ListedExtension
): RelayImportResult {
  const manifest = extension.manifest;
  if (!manifest) {
    throw new Error('扩展 manifest 不存在，无法导入 relay 贡献');
  }

  const relayProviders = manifest.contributions?.relay_providers ?? [];
  const relayRoutes = manifest.contributions?.relay_routes ?? [];
  if (relayProviders.length === 0 && relayRoutes.length === 0) {
    throw new Error(`扩展 ${manifest.id} 不包含 relay provider 或 route 贡献`);
  }

  const nextConfig: RelayConfig = {
    version: config.version || 1,
    default_route: config.default_route,
    providers: [...(config.providers ?? [])],
    routes: [...(config.routes ?? [])],
  };

  const existingProviderIds = new Set((nextConfig.providers ?? []).map((item) => item.id));
  const existingRouteIds = new Set((nextConfig.routes ?? []).map((item) => item.id));
  const providerConflicts = relayProviders.filter((item) => existingProviderIds.has(item.id)).map((item) => item.id);
  const routeConflicts = relayRoutes.filter((item) => existingRouteIds.has(item.id)).map((item) => item.id);

  if (providerConflicts.length > 0 || routeConflicts.length > 0) {
    const messages: string[] = [];
    if (providerConflicts.length > 0) {
      messages.push(`provider 冲突: ${providerConflicts.join(', ')}`);
    }
    if (routeConflicts.length > 0) {
      messages.push(`route 冲突: ${routeConflicts.join(', ')}`);
    }
    throw new Error(`导入已中止，存在同名配置。${messages.join('；')}`);
  }

  const sourceTag = `extension:${manifest.id}`;
  const importedProviders: RelayProvider[] = relayProviders.map((item) => ({
    id: item.id,
    name: item.name,
    base_url: item.base_url,
    source: sourceTag,
    weight: item.weight ?? 1,
    enabled: false,
    model_patterns: item.model_patterns,
    headers: item.headers,
  }));
  const importedRoutes: RelayRoute[] = relayRoutes.map((item) => ({
    id: item.id,
    name: item.name,
    source: sourceTag,
    path_prefixes: item.path_prefixes,
    model_patterns: item.model_patterns,
    provider_ids: item.provider_ids,
    strategy: item.strategy ?? 'weighted_round_robin',
  }));

  nextConfig.providers?.push(...importedProviders);
  nextConfig.routes?.push(...importedRoutes);

  return {
    config: nextConfig,
    addedProviders: importedProviders.map((item) => item.id),
    addedRoutes: importedRoutes.map((item) => item.id),
    sourceTag,
  };
}
