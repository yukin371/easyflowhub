import type {
  EffectiveExtensionContributions,
  ListedExtension,
} from '../../../types/scriptmgr';

export interface EffectiveRelayContributionSummary {
  providerCount: number;
  routeCount: number;
}

export function getEffectiveRelayContributionSummary(
  extension: ListedExtension,
  contributions?: EffectiveExtensionContributions | null
): EffectiveRelayContributionSummary {
  const extensionId = extension.manifest?.id;
  if (!extensionId || !contributions) {
    return { providerCount: 0, routeCount: 0 };
  }

  return {
    providerCount:
      contributions.relay_providers?.filter((item) => item.source.extension_id === extensionId).length ?? 0,
    routeCount:
      contributions.relay_routes?.filter((item) => item.source.extension_id === extensionId).length ?? 0,
  };
}

export function getEffectiveRelayContributionTotals(
  contributions?: EffectiveExtensionContributions | null
): EffectiveRelayContributionSummary {
  return {
    providerCount: contributions?.relay_providers?.length ?? 0,
    routeCount: contributions?.relay_routes?.length ?? 0,
  };
}
