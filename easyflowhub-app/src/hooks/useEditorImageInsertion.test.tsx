import '@testing-library/jest-dom/vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorImageInsertion } from './useEditorImageInsertion';
import { saveImage } from '../lib/tauri/notes';

const onDragDropEventMock = vi.hoisted(() => vi.fn(async () => () => {}));

vi.mock('../lib/tauri/notes', () => ({
  saveImage: vi.fn(),
  saveImageFromPath: vi.fn(),
}));

vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: vi.fn(() => ({
    onDragDropEvent: onDragDropEventMock,
  })),
}));

const saveImageMock = vi.mocked(saveImage);

class MockFileReader {
  result: string | ArrayBuffer | null = null;
  error: Error | null = null;
  onload: null | (() => void) = null;
  onerror: null | (() => void) = null;

  readAsDataURL() {
    this.result = 'data:image/png;base64,ZmFrZQ==';
    this.onload?.();
  }
}

describe('useEditorImageInsertion', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    saveImageMock.mockResolvedValue({
      ok: true,
      filename: 'saved.png',
      path: 'assets/saved.png',
    });
    vi.stubGlobal('FileReader', MockFileReader);
  });

  it('粘贴图片时保存并通过 onImageSaved 返回图片元数据', async () => {
    const onImageSaved = vi.fn();
    const onChange = vi.fn();
    const textareaRef = { current: null };
    const file = new File(['fake'], 'pasted-image.png', { type: 'image/png' });
    const preventDefault = vi.fn();

    const { result } = renderHook(() =>
      useEditorImageInsertion({
        value: '',
        onChange,
        textareaRef,
        onImageSaved,
      })
    );

    act(() => {
      result.current.handlePaste({
        clipboardData: { files: [file] },
        preventDefault,
      } as unknown as React.ClipboardEvent<HTMLTextAreaElement>);
    });

    await waitFor(() => {
      expect(saveImageMock).toHaveBeenCalledWith('data:image/png;base64,ZmFrZQ==');
    });
    await waitFor(() => {
      expect(onImageSaved).toHaveBeenCalledWith({
        altText: 'pasted-image',
        filename: 'saved.png',
        path: 'assets/saved.png',
      });
    });
    expect(preventDefault).toHaveBeenCalledTimes(1);
  });
});
