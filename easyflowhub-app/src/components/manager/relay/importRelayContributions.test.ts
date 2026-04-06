import { describe, expect, it } from 'vitest';
import type { ListedExtension, RelayConfig } from '../../../types/scriptmgr';
import {
  getRelayContributionSummary,
  hasRelayContributions,
  importRelayContributions,
} from './importRelayContributions';

const extension: ListedExtension = {
  manifest_path: 'extensions/sample/plugin.json',
  root: 'extensions',
  status: 'loaded',
  manifest: {
    id: 'sample-pack',
    name: 'Sample Pack',
    version: '1.0.0',
    contributions: {
      relay_providers: [
        {
          id: 'sample-provider',
          name: 'Sample Provider',
          base_url: 'https://api.example.com',
          weight: 2,
          model_patterns: ['gpt-*'],
        },
      ],
      relay_routes: [
        {
          id: 'sample-route',
          provider_ids: ['sample-provider'],
          path_prefixes: ['/v1/'],
          model_patterns: ['gpt-*'],
          strategy: 'weighted_round_robin',
        },
      ],
    },
  },
};

describe('importRelayContributions', () => {
  it('summarizes relay contributions', () => {
    expect(getRelayContributionSummary(extension)).toEqual({
      providerCount: 1,
      routeCount: 1,
    });
    expect(hasRelayContributions(extension)).toBe(true);
  });

  it('imports providers and routes with source tagging', () => {
    const config: RelayConfig = { version: 1, providers: [], routes: [] };
    const result = importRelayContributions(config, extension);

    expect(result.addedProviders).toEqual(['sample-provider']);
    expect(result.addedRoutes).toEqual(['sample-route']);
    expect(result.sourceTag).toBe('extension:sample-pack');
    expect(result.config.providers?.[0]).toMatchObject({
      id: 'sample-provider',
      enabled: false,
      source: 'extension:sample-pack',
      weight: 2,
    });
    expect(result.config.routes?.[0]).toMatchObject({
      id: 'sample-route',
      source: 'extension:sample-pack',
    });
  });

  it('rejects conflicting provider or route ids', () => {
    const config: RelayConfig = {
      version: 1,
      providers: [
        {
          id: 'sample-provider',
          name: 'Existing',
          base_url: 'https://existing.example.com',
          enabled: true,
        },
      ],
      routes: [],
    };

    expect(() => importRelayContributions(config, extension)).toThrow(/provider 冲突/);
  });
});
