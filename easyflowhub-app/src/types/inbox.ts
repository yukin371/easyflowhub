export type InboxItemKind = 'text' | 'link' | 'image' | 'file' | 'audio';
export type InboxAttachmentKind = 'image' | 'file' | 'audio';

export interface InboxAttachment {
  id: string;
  itemId: string;
  kind: InboxAttachmentKind;
  name: string;
  mimeType: string;
  filename: string;
  sizeBytes: number;
  createdAt: string;
}

export interface InboxItem {
  id: string;
  kind: InboxItemKind;
  title: string;
  content: string;
  source: string;
  context: string;
  capturedAt: string;
  archivedAt: string | null;
  convertedNoteId: string | null;
  attachments: InboxAttachment[];
}

export interface CaptureInboxItemInput {
  title?: string;
  content?: string;
  source?: string;
  context?: string;
  imageDataUrl?: string;
  filePaths?: string[];
  audioFilePath?: string;
}

export interface InboxItemsResponse {
  ok: boolean;
  count: number;
  items: InboxItem[];
}

export interface InboxItemResponse {
  ok: boolean;
  item: InboxItem;
}

export interface InboxStateResponse {
  ok: boolean;
  itemId: string;
}
