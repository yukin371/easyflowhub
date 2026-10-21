/**
 * Update check against the project's GitHub releases.
 *
 * This is a plain HTTP comparison against GitHub's `releases/latest` (which
 * already excludes pre-releases and drafts), so it answers "is there a newer
 * published version than the one I'm running?" independently of the Tauri
 * updater's install flow. Pure helpers are separated from the network call so
 * they can be unit-tested without fetch.
 */

/** Repository that hosts the releases. */
export const RELEASES_REPO = 'yukin371/easyflowhub';
const RELEASES_API = `https://api.github.com/repos/${RELEASES_REPO}/releases/latest`;
const RELEASES_PAGE = `https://github.com/${RELEASES_REPO}/releases/latest`;

export interface LatestRelease {
  /** Tag name as published, e.g. "v0.2.1". */
  tag: string;
  /** Normalized version without a leading "v", e.g. "0.2.1". */
  version: string;
  /** Human-readable release name, if any. */
  name?: string;
  /** Browser URL for the release page. */
  url: string;
}

export type UpdateStatus =
  | { kind: 'up-to-date'; current: string }
  | { kind: 'update-available'; current: string; latest: string; url: string; name?: string }
  | { kind: 'ahead'; current: string; latest: string };

/**
 * Normalize a tag or version to a bare "X.Y.Z[-prerelease]" string. Returns
 * null when the input is not a recognizable version.
 */
export function normalizeVersion(input: string): string | null {
  const trimmed = input.trim().replace(/^v/i, '');
  return /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(trimmed) ? trimmed : null;
}

interface ParsedVersion {
  core: [number, number, number];
  prerelease: string[];
}

function parseVersion(input: string): ParsedVersion | null {
  const normalized = normalizeVersion(input);
  if (!normalized) return null;
  const [core, prerelease = ''] = normalized.split('-', 2);
  const parts = core.split('.').map((part) => Number.parseInt(part, 10));
  if (parts.length !== 3 || parts.some((n) => Number.isNaN(n))) return null;
  return {
    core: [parts[0], parts[1], parts[2]],
    prerelease: prerelease ? prerelease.split('.') : [],
  };
}

/** SemVer precedence: core first, then no-prerelease > prerelease, then ids. */
export function compareVersions(a: string, b: string): number {
  const left = parseVersion(a);
  const right = parseVersion(b);
  if (!left || !right) return 0;

  for (let i = 0; i < 3; i++) {
    if (left.core[i] !== right.core[i]) return left.core[i] > right.core[i] ? 1 : -1;
  }

  if (left.prerelease.length === 0 && right.prerelease.length > 0) return 1;
  if (left.prerelease.length > 0 && right.prerelease.length === 0) return -1;

  const len = Math.max(left.prerelease.length, right.prerelease.length);
  for (let i = 0; i < len; i++) {
    const l = left.prerelease[i];
    const r = right.prerelease[i];
    if (l === undefined) return -1;
    if (r === undefined) return 1;
    const lNum = /^\d+$/.test(l) ? Number.parseInt(l, 10) : null;
    const rNum = /^\d+$/.test(r) ? Number.parseInt(r, 10) : null;
    if (lNum !== null && rNum !== null) {
      if (lNum !== rNum) return lNum > rNum ? 1 : -1;
    } else if (lNum !== null) {
      return -1;
    } else if (rNum !== null) {
      return 1;
    } else if (l !== r) {
      return l > r ? 1 : -1;
    }
  }
  return 0;
}

/** Decide the update status for a running version against the latest release. */
export function describeUpdate(current: string, latest: LatestRelease): UpdateStatus {
  const cmp = compareVersions(current, latest.version);
  if (cmp === 0) return { kind: 'up-to-date', current };
  if (cmp > 0) return { kind: 'ahead', current, latest: latest.version };
  return { kind: 'update-available', current, latest: latest.version, url: latest.url, name: latest.name };
}

function pickReleaseUrl(payload: Record<string, unknown>): string {
  const htmlUrl = typeof payload.html_url === 'string' ? payload.html_url : undefined;
  return htmlUrl ?? RELEASES_PAGE;
}

/**
 * Fetch the latest published release. Throws on network failure or a
 * non-success status so callers can surface an error instead of guessing.
 */
export async function fetchLatestRelease(
  fetchImpl: typeof fetch = fetch,
): Promise<LatestRelease> {
  const response = await fetchImpl(RELEASES_API, {
    headers: { Accept: 'application/vnd.github+json' },
  });
  if (!response.ok) {
    throw new Error(`GitHub 返回 ${response.status}`);
  }
  const payload = (await response.json()) as Record<string, unknown>;
  const tag = typeof payload.tag_name === 'string' ? payload.tag_name : '';
  const version = normalizeVersion(tag);
  if (!version) {
    throw new Error('无法解析最新版本号');
  }
  return {
    tag,
    version,
    name: typeof payload.name === 'string' ? payload.name : undefined,
    url: pickReleaseUrl(payload),
  };
}
