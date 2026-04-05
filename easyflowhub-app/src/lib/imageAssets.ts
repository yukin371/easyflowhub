import { convertFileSrc } from '@tauri-apps/api/core';
import { appDataDir, join } from '@tauri-apps/api/path';

export interface MarkdownImageRef {
  alt: string;
  src: string;
}

export interface AttachedImageAsset {
  alt: string;
  filename: string;
}

const MARKDOWN_IMAGE_REGEX = /!\[([^\]]*)]\(([^)\s]+)\)/g;
const SHORT_ASSET_PREFIX = 'asset:';

export function buildShortAssetSrc(filename: string): string {
  return `${SHORT_ASSET_PREFIX}${filename}`;
}

export function isShortAssetSrc(src: string): boolean {
  return src.startsWith(SHORT_ASSET_PREFIX);
}

export function getAssetFilenameFromSrc(src: string): string | null {
  if (isShortAssetSrc(src)) {
    return src.slice(SHORT_ASSET_PREFIX.length).trim() || null;
  }

  try {
    const decoded = decodeURIComponent(src);
    const normalized = decoded.replace(/\\/g, '/');
    const marker = '/assets/';
    const markerIndex = normalized.lastIndexOf(marker);
    if (markerIndex >= 0) {
      const filename = normalized.slice(markerIndex + marker.length).trim();
      return filename || null;
    }
  } catch {
    return null;
  }

  return null;
}

export function normalizeMarkdownAssetSources(content: string): string {
  return content.replace(MARKDOWN_IMAGE_REGEX, (fullMatch, alt: string, src: string) => {
    const filename = getAssetFilenameFromSrc(src);
    if (!filename) {
      return fullMatch;
    }

    return `![${alt}](${buildShortAssetSrc(filename)})`;
  });
}

export function extractMarkdownImages(content: string): MarkdownImageRef[] {
  const refs: MarkdownImageRef[] = [];
  for (const match of content.matchAll(MARKDOWN_IMAGE_REGEX)) {
    const [, alt, src] = match;
    refs.push({ alt, src });
  }
  return refs;
}

export function extractAttachedImageAssets(content: string): {
  textContent: string;
  images: AttachedImageAsset[];
} {
  const lines = content.split('\n');
  const images: AttachedImageAsset[] = [];
  const textLines: string[] = [];

  for (const line of lines) {
    const match = /^\s*!\[([^\]]*)]\(([^)\s]+)\)\s*$/.exec(line);
    if (!match) {
      textLines.push(line);
      continue;
    }

    const filename = getAssetFilenameFromSrc(match[2]);
    if (!filename) {
      textLines.push(line);
      continue;
    }

    images.push({
      alt: match[1],
      filename,
    });
  }

  return {
    textContent: textLines.join('\n').replace(/\n{3,}/g, '\n\n'),
    images,
  };
}

export function appendAttachedImageAssets(
  textContent: string,
  images: AttachedImageAsset[]
): string {
  const normalizedText = textContent.replace(/\s+$/, '');
  const imageBlock = images
    .map((image) => `![${image.alt}](${buildShortAssetSrc(image.filename)})`)
    .join('\n');

  if (!imageBlock) {
    return textContent;
  }

  if (!normalizedText) {
    return imageBlock;
  }

  return `${normalizedText}\n\n${imageBlock}`;
}

export function summarizeContentWithoutImageMarkdown(content: string): string {
  const { textContent, images } = extractAttachedImageAssets(content);
  const summary = textContent.trim();

  if (summary) {
    return summary;
  }

  if (images.length > 0) {
    return `包含 ${images.length} 张图片`;
  }

  return '';
}

export async function resolveAssetFilenameToUrl(filename: string): Promise<string> {
  const baseDir = await appDataDir();
  const fullPath = await join(baseDir, 'assets', filename);
  return convertFileSrc(fullPath);
}

export async function resolveMarkdownImageSrc(src: string): Promise<string> {
  const filename = getAssetFilenameFromSrc(src);
  if (!filename) {
    return src;
  }

  return resolveAssetFilenameToUrl(filename);
}

export async function resolveMarkdownAssetSources(content: string): Promise<string> {
  const matches = Array.from(content.matchAll(MARKDOWN_IMAGE_REGEX));
  if (matches.length === 0) {
    return content;
  }

  let resolved = content;
  for (const match of matches) {
    const [fullMatch, alt, src] = match;
    const nextSrc = await resolveMarkdownImageSrc(src);
    const nextMatch = `![${alt}](${nextSrc})`;
    resolved = resolved.replace(fullMatch, nextMatch);
  }

  return resolved;
}
