import { describe, expect, it } from 'vitest';
import type { EffectiveExtensionContributions, ListedExtension } from '../../../types/scriptmgr';
import {
  getEffectiveRelayContributionSummary,
  getEffectiveRelayContributionTotals,
} from './effectiveRelayContributions';

const extension: ListedExtension = {
  manifest_path: 'extensions/sample/plugin.json',
  root: 'extensions',
  status: 'loaded',
  manifest: {
    id: 'sample-pack',
    name: 'Sample Pack',
    version: '1.0.0',
  },
};

const contributions: EffectiveExtensionContributions = {
  relay_providers: [
    {
      id: 'provider-a',
      name: 'Provider A',
      base_url: 'https://a.example.com',
      source: {
        extension_id: 'sample-pack',
        extension_name: 'Sample Pack',
        extension_version: '1.0.0',
        manifest_path: 'extensions/sample/plugin.json',
        root: 'extensions',
      },
    },
  ],
  relay_routes: [
    {
      id: 'route-a',
      provider_ids: ['provider-a'],
      source: {
        extension_id: 'sample-pack',
        extension_name: 'Sample Pack',
        extension_version: '1.0.0',
        manifest_path: 'extensions/sample/plugin.json',
        root: 'extensions',
      },
    },
    {
      id: 'route-b',
      provider_ids: ['provider-b'],
      source: {
        extension_id: 'other-pack',
        extension_name: 'Other Pack',
        extension_version: '1.0.0',
        manifest_path: 'extensions/other/plugin.json',
        root: 'extensions',
      },
    },
  ],
};

describe('effectiveRelayContributions', () => {
  it('summarizes relay contributions per extension', () => {
    expect(getEffectiveRelayContributionSummary(extension, contributions)).toEqual({
      providerCount: 1,
      routeCount: 1,
    });
  });

  it('summarizes global effective relay contribution totals', () => {
    expect(getEffectiveRelayContributionTotals(contributions)).toEqual({
      providerCount: 1,
      routeCount: 2,
    });
  });
});
