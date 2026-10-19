import { invoke } from '@tauri-apps/api/core';
import type {
  CaptureInboxItemInput,
  InboxItem,
  InboxItemResponse,
  InboxItemsResponse,
  InboxStateResponse,
} from '../../types/inbox';

export async function listInboxItems(includeArchived = false): Promise<InboxItem[]> {
  const response: InboxItemsResponse = await invoke('list_inbox_items', { includeArchived });
  return response.items;
}

export async function captureInboxItem(input: CaptureInboxItemInput): Promise<InboxItem> {
  const response: InboxItemResponse = await invoke('capture_inbox_item', { input });
  return response.item;
}

export async function archiveInboxItem(id: string): Promise<string> {
  const response: InboxStateResponse = await invoke('archive_inbox_item', { id });
  return response.itemId;
}

export async function restoreInboxItem(id: string): Promise<string> {
  const response: InboxStateResponse = await invoke('restore_inbox_item', { id });
  return response.itemId;
}

export async function convertInboxItemToNote(id: string): Promise<string> {
  const response: { ok: boolean; itemId: string; noteId: string } = await invoke('convert_inbox_item_to_note', { id });
  return response.noteId;
}

export async function openInboxAttachment(id: string): Promise<void> {
  await invoke('open_inbox_attachment', { id });
}

export async function openInboxLink(id: string): Promise<void> {
  await invoke('open_inbox_link', { id });
}

export async function deleteInboxItem(id: string): Promise<string> {
  const response: InboxStateResponse = await invoke('delete_inbox_item', { id });
  return response.itemId;
}

export async function openInboxAssetsDir(): Promise<void> {
  await invoke('open_inbox_assets_dir');
}
