import { describe, expect, it } from 'vitest';
import {
  compareVersions,
  describeUpdate,
  normalizeVersion,
  type LatestRelease,
} from './updateCheck';

function release(version: string, extra: Partial<LatestRelease> = {}): LatestRelease {
  return {
    tag: `v${version}`,
    version,
    url: `https://github.com/yukin371/easyflowhub/releases/tag/v${version}`,
    ...extra,
  };
}

describe('normalizeVersion', () => {
  it('strips a leading v and accepts X.Y.Z', () => {
    expect(normalizeVersion('v0.2.1')).toBe('0.2.1');
    expect(normalizeVersion('0.2.1')).toBe('0.2.1');
  });

  it('accepts a pre-release suffix', () => {
    expect(normalizeVersion('v0.3.0-beta.1')).toBe('0.3.0-beta.1');
  });

  it('rejects non-version input', () => {
    expect(normalizeVersion('latest')).toBeNull();
    expect(normalizeVersion('1.2')).toBeNull();
    expect(normalizeVersion('')).toBeNull();
  });
});

describe('compareVersions', () => {
  it('compares core numbers', () => {
    expect(compareVersions('0.2.1', '0.2.0')).toBe(1);
    expect(compareVersions('0.2.0', '0.2.1')).toBe(-1);
    expect(compareVersions('1.0.0', '0.9.9')).toBe(1);
    expect(compareVersions('0.2.0', '0.2.0')).toBe(0);
  });

  it('treats a release as newer than its own pre-release', () => {
    expect(compareVersions('0.3.0', '0.3.0-beta.1')).toBe(1);
    expect(compareVersions('0.3.0-beta.1', '0.3.0')).toBe(-1);
  });

  it('orders pre-release identifiers', () => {
    expect(compareVersions('0.3.0-beta.2', '0.3.0-beta.1')).toBe(1);
    expect(compareVersions('0.3.0-beta.1', '0.3.0-rc.1')).toBe(-1);
  });
});

describe('describeUpdate', () => {
  it('reports up-to-date when versions match', () => {
    expect(describeUpdate('0.2.0', release('0.2.0'))).toEqual({
      kind: 'up-to-date',
      current: '0.2.0',
    });
  });

  it('reports an available update when latest is newer', () => {
    const status = describeUpdate('0.2.0', release('0.2.1', { name: 'Fix' }));
    expect(status).toEqual({
      kind: 'update-available',
      current: '0.2.0',
      latest: '0.2.1',
      url: release('0.2.1').url,
      name: 'Fix',
    });
  });

  it('reports ahead when the running build is newer than the latest release', () => {
    expect(describeUpdate('0.3.0', release('0.2.1'))).toEqual({
      kind: 'ahead',
      current: '0.3.0',
      latest: '0.2.1',
    });
  });
});
