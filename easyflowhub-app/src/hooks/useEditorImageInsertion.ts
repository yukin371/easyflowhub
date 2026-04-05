import { useCallback, useEffect, useRef, useState } from 'react';
import type React from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { saveImage, saveImageFromPath } from '../lib/tauri/notes';
import { insertSnippetAtCursor, type TextSelectionUpdate } from '../lib/editorTransforms';
import { buildShortAssetSrc } from '../lib/imageAssets';

interface UseEditorImageInsertionOptions {
  value: string;
  onChange: (update: TextSelectionUpdate) => void;
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
  onImageSaved?: (image: { altText: string; filename: string; path: string }) => void;
  nativeWindowDropEnabled?: boolean;
}

export function useEditorImageInsertion(options: UseEditorImageInsertionOptions) {
  const { value, onChange, textareaRef, onImageSaved, nativeWindowDropEnabled = false } = options;
  const [isDragActive, setIsDragActive] = useState(false);
  const [insertResult, setInsertResult] = useState<string | null>(null);
  const dragDepthRef = useRef(0);
  const lastNativeDropRef = useRef<{ path: string; at: number } | null>(null);

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

      onChange(insertSnippetAtCursor(value, textarea.selectionStart, textarea.selectionEnd, snippet));
    },
    [onChange, textareaRef, value]
  );

  const handleSavedImage = useCallback(
    (filename: string, path: string, altText: string) => {
      if (onImageSaved) {
        onImageSaved({ altText, filename, path });
        return;
      }

      const prefix = value.length === 0 || value.endsWith('\n') ? '' : '\n';
      const assetUrl = buildShortAssetSrc(filename);
      insertAtCursor(`${prefix}![${altText}](${assetUrl})\n`);
    },
    [insertAtCursor, onImageSaved, value]
  );

  const insertImageFromPath = useCallback(
    async (filePath: string) => {
      try {
        setInsertResult(`保存图片中...`);
        const result = await saveImageFromPath(filePath);
        const altText = filePath.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') || 'image';
        handleSavedImage(result.filename, result.path, altText);
        setInsertResult(`✅ 图片已插入`);
        setTimeout(() => setInsertResult(null), 2000);
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : String(error);
        console.error('Failed to insert image from path:', error);
        setInsertResult(`❌ 失败: ${errorMsg}`);
        setTimeout(() => setInsertResult(null), 4000);
      }
    },
    [handleSavedImage]
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
        handleSavedImage(result.filename, result.path, altText);
        setInsertResult(`✅ 图片已插入`);
        setTimeout(() => setInsertResult(null), 2000);
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : String(error);
        console.error('Failed to insert image:', error);
        setInsertResult(`❌ 失败: ${errorMsg}`);
        setTimeout(() => setInsertResult(null), 4000);
      }
    },
    [handleSavedImage]
  );

  useEffect(() => {
    if (!nativeWindowDropEnabled) {
      return;
    }

    let unlisten: (() => void) | undefined;

    const setupDragDrop = async () => {
      const webview = getCurrentWebview();
      unlisten = await webview.onDragDropEvent((event) => {
        if (event.payload.type === 'enter' || event.payload.type === 'over') {
          setIsDragActive(true);
          return;
        }

        if (event.payload.type === 'leave') {
          resetDragState();
          return;
        }

        resetDragState();
        for (const path of event.payload.paths) {
          const ext = path.split('.').pop()?.toLowerCase() || '';
          if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'].includes(ext)) {
            const now = Date.now();
            const lastDrop = lastNativeDropRef.current;
            if (lastDrop && lastDrop.path === path && now - lastDrop.at < 800) {
              continue;
            }
            lastNativeDropRef.current = { path, at: now };
            void insertImageFromPath(path);
            break;
          }
        }
      });
    };

    void setupDragDrop();
    return () => unlisten?.();
  }, [insertImageFromPath, nativeWindowDropEnabled, resetDragState]);

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
