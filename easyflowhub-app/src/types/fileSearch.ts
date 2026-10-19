/**
 * Types shared by the file search bridge and manager UI.
 *
 * The native index intentionally exposes one configured root for v0.2.0.
 * Paths are validated and canonicalized by Rust; the frontend never joins
 * or normalizes paths before sending them to the native commands.
 */

export type FileSearchSort = 'relevance' | 'modified' | 'recent' | 'name';

export interface FileSearchStatus {
  generation: number;
  paused: boolean;
  root: string | null;
  count: number;
}

export interface FileEntry {
  id: string;
  path: string;
  name: string;
  extension: string;
  size: number;
  modifiedAt: string | null;
  isDir: boolean;
}

export interface FileSearchRequest {
  query: string;
  extension?: string;
  sort?: FileSearchSort;
  limit?: number;
}

export interface FileSearchResponse {
  items: FileEntry[];
  total: number;
  generation: number;
}

export type FilePreviewKind = 'text' | 'image' | 'unsupported';

export interface FilePreview {
  kind: FilePreviewKind;
  path: string;
  content: string | null;
  mimeType: string | null;
  /** A data URL produced by native for images inside arbitrary user folders. */
  imageDataUrl: string | null;
  size: number;
  truncated: boolean;
}

export interface FileDeleteResult {
  ok: boolean;
  path: string;
  generation: number;
}

export interface FileOperationResponse {
  entry: FileEntry;
  generation: number;
}
