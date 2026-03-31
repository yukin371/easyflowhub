import { useCallback, useEffect, useRef, useState } from 'react';
import type React from 'react';
import { listen } from '@tauri-apps/api/event';
import { readFile } from '@tauri-apps/plugin-fs';
import { saveImage } from '../lib/tauri/notes';
import { insertSnippetAtCursor, type TextSelectionUpdate } from '../lib/editorTransforms';

interface UseEditorImageInsertionOptions {
  value: string;
  onChange: (update: TextSelectionUpdate) => void;
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
}

export function useEditorImageInsertion(options: UseEditorImageInsertionOptions) {
  const { value, onChange, textareaRef } = options;
  const [isDragActive, setIsDragActive] = useState(false);
  const [insertResult, setInsertResult] = useState<string | null>(null);
  const dragDepthRef = useRef(0);

  const hasImageFile = useCallback((files: FileList | File[]) => {
    return Array.from(files).some((item) => item.type.startsWith('image/'));
  }, []);

  const resetDragState = useCallback(() => {
    dragDepthRef.current = 0;
    setIsDragActive(false);
  }, []);

  const insertAtCursor = useCallback(
    (snippet: string) => {
      const textarea = textareaRef.current;
      if (!textarea) {
        const prefix = value.length === 0 || value.endsWith('\n') ? '' : '\n';
        onChange({
          value: `${value}${prefix}${snippet}`,
          selectionStart: value.length + prefix.length + snippet.length,
        });
        return;
      }

      onChange(
        insertSnippetAtCursor(value, textarea.selectionStart, textarea.selectionEnd, snippet)
      );
    },
    [onChange, textareaRef, value]
  );

  const insertImageFile = useCallback(
    async (file: File) => {
      try {
        setInsertResult(`读取文件: ${file.name}`);
        const dataUrl = await new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(String(reader.result ?? ''));
          reader.onerror = () => reject(reader.error ?? new Error('Failed to read image file.'));
          reader.readAsDataURL(file);
        });

        setInsertResult(`保存图片中...`);
        const result = await saveImage(dataUrl);
        const altText = file.name.replace(/\.[^.]+$/, '') || 'image';
        const prefix = value.length === 0 || value.endsWith('\n') ? '' : '\n';
        // 使用 asset 协议的相对路径格式，避免 Windows 路径编码问题
        const assetUrl = `asset://localhost/assets/${result.filename}`;
        insertAtCursor(`${prefix}![${altText}](${assetUrl})\n`);
        setInsertResult(`✅ 图片已插入`);
        setTimeout(() => setInsertResult(null), 2000);
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : String(error);
        console.error('Failed to insert image:', error);
        setInsertResult(`❌ 失败: ${errorMsg}`);
        setTimeout(() => setInsertResult(null), 4000);
      }
    },
    [insertAtCursor, value]
  );

  const insertImageFromPath = useCallback(
    async (filePath: string) => {
      try {
        setInsertResult(`读取文件...`);
        const fileData = await readFile(filePath);
        const ext = filePath.split('.').pop()?.toLowerCase() || 'png';
        const mimeTypes: Record<string, string> = {
          png: 'image/png',
          jpg: 'image/jpeg',
          jpeg: 'image/jpeg',
          gif: 'image/gif',
          webp: 'image/webp',
          bmp: 'image/bmp',
          svg: 'image/svg+xml',
        };
        const mimeType = mimeTypes[ext] || 'image/png';
        const base64 = btoa(
          new Uint8Array(fileData).reduce((data, byte) => data + String.fromCharCode(byte), '')
        );
        const dataUrl = `data:${mimeType};base64,${base64}`;

        setInsertResult(`保存图片中...`);
        const result = await saveImage(dataUrl);
        const altText = filePath.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') || 'image';
        const prefix = value.length === 0 || value.endsWith('\n') ? '' : '\n';
        // 使用 asset 协议的相对路径格式，避免 Windows 路径编码问题
        const assetUrl = `asset://localhost/assets/${result.filename}`;
        insertAtCursor(`${prefix}![${altText}](${assetUrl})\n`);
        setInsertResult(`✅ 图片已插入`);
        setTimeout(() => setInsertResult(null), 2000);
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : String(error);
        console.error('Failed to insert image from path:', error);
        setInsertResult(`❌ 失败: ${errorMsg}`);
        setTimeout(() => setInsertResult(null), 4000);
      }
    },
    [insertAtCursor, value]
  );

  useEffect(() => {
    let unlistenDragDrop: (() => void) | undefined;
    let unlistenDragEnter: (() => void) | undefined;
    let unlistenDragLeave: (() => void) | undefined;

    const setupDragDropListener = async () => {
      unlistenDragEnter = await listen('tauri://drag-enter', () => {
        setIsDragActive(true);
      });

      unlistenDragLeave = await listen('tauri://drag-leave', () => {
        resetDragState();
      });

      const unlisten = await listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
        resetDragState();
        const { paths } = event.payload;
        const imageExtensions = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'];

        for (const path of paths) {
          const ext = path.split('.').pop()?.toLowerCase() || '';
          if (imageExtensions.includes(ext)) {
            await insertImageFromPath(path);
          }
        }
      });
      unlistenDragDrop = unlisten;
    };

    void setupDragDropListener();
    return () => {
      unlistenDragDrop?.();
      unlistenDragEnter?.();
      unlistenDragLeave?.();
    };
  }, [insertImageFromPath, resetDragState]);

  return {
    isDragActive,
    insertResult,
    handlePaste: (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const file = Array.from(event.clipboardData.files).find((item) => item.type.startsWith('image/'));
      if (!file) {
        return;
      }
      event.preventDefault();
      void insertImageFile(file);
    },
    handleDragOver: (event: React.DragEvent<HTMLTextAreaElement>) => {
      if (hasImageFile(event.dataTransfer.files)) {
        event.preventDefault();
        setIsDragActive(true);
      }
    },
    handleDragEnter: (event: React.DragEvent<HTMLTextAreaElement>) => {
      if (!hasImageFile(event.dataTransfer.files)) {
        return;
      }
      dragDepthRef.current += 1;
      event.preventDefault();
      setIsDragActive(true);
    },
    handleDragLeave: (event: React.DragEvent<HTMLTextAreaElement>) => {
      if (!hasImageFile(event.dataTransfer.files)) {
        return;
      }
      dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
      if (dragDepthRef.current === 0) {
        event.preventDefault();
        setIsDragActive(false);
      }
    },
    handleDrop: (event: React.DragEvent<HTMLTextAreaElement>) => {
      resetDragState();
      const file = Array.from(event.dataTransfer.files).find((item) => item.type.startsWith('image/'));
      if (!file) {
        return;
      }
      event.preventDefault();
      void insertImageFile(file);
    },
  };
}
