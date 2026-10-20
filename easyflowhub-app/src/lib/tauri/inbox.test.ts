import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { archiveInboxItem, captureInboxItem, convertInboxItemToNote, deleteInboxItem, emptyInboxTrash, listInboxItems, listInboxTrash, openInboxAssetsDir, openInboxAttachment, openInboxLink, permanentlyDeleteInboxItem, restoreInboxItem, restoreInboxItemFromTrash } from './inbox';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe('Inbox Tauri bridge', () => {
  beforeEach(() => invokeMock.mockReset());

  it('sends capture data through the single native capture command', async () => {
    const item = { id: 'item-1' } as never;
    invokeMock.mockResolvedValue({ item });
    const input = { content: 'captured', source: 'launcher', context: 'clipboard' };

    await expect(captureInboxItem(input)).resolves.toBe(item);
    expect(invokeMock).toHaveBeenCalledWith('capture_inbox_item', { input });
  });

  it('lists active or archived captures and forwards state/open commands', async () => {
    invokeMock.mockResolvedValue({ items: [] });
    await listInboxItems();
    expect(invokeMock).toHaveBeenLastCalledWith('list_inbox_items', { includeArchived: false });
    await listInboxItems(true);
    expect(invokeMock).toHaveBeenLastCalledWith('list_inbox_items', { includeArchived: true });

    invokeMock.mockResolvedValue({ itemId: 'item-1' });
    await archiveInboxItem('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('archive_inbox_item', { id: 'item-1' });
    await restoreInboxItem('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('restore_inbox_item', { id: 'item-1' });
    invokeMock.mockResolvedValue({ noteId: 'note-1' });
    await convertInboxItemToNote('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('convert_inbox_item_to_note', { id: 'item-1' });
    await openInboxAttachment('attachment-1');
    expect(invokeMock).toHaveBeenLastCalledWith('open_inbox_attachment', { id: 'attachment-1' });
    await openInboxLink('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('open_inbox_link', { id: 'item-1' });
  });

  it('forwards move-to-trash and the resource folder commands', async () => {
    invokeMock.mockResolvedValue({ itemId: 'item-1' });
    await expect(deleteInboxItem('item-1')).resolves.toBe('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('delete_inbox_item', { id: 'item-1' });

    invokeMock.mockResolvedValue(undefined);
    await openInboxAssetsDir();
    expect(invokeMock).toHaveBeenLastCalledWith('open_inbox_assets_dir');
  });

  it('forwards Inbox trash list, restore, purge, and empty commands', async () => {
    invokeMock.mockResolvedValue({ items: [] });
    await listInboxTrash();
    expect(invokeMock).toHaveBeenLastCalledWith('list_inbox_trash');

    invokeMock.mockResolvedValue({ itemId: 'item-1' });
    await restoreInboxItemFromTrash('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('restore_inbox_item_from_trash', { id: 'item-1' });
    await permanentlyDeleteInboxItem('item-1');
    expect(invokeMock).toHaveBeenLastCalledWith('permanently_delete_inbox_item', { id: 'item-1' });

    invokeMock.mockResolvedValue({ deletedCount: 2 });
    await expect(emptyInboxTrash()).resolves.toBe(2);
    expect(invokeMock).toHaveBeenLastCalledWith('empty_inbox_trash');
  });
});
