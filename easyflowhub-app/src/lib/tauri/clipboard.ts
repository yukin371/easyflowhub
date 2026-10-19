import { invoke } from '@tauri-apps/api/core';

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Write text through the native OS clipboard boundary. */
export function copyTextToClipboard(text: string): Promise<void> {
  return invoke<void>('copy_text_to_clipboard', { text });
}

/** Read text through the native OS clipboard boundary. */
export function readTextFromClipboard(): Promise<string> {
  return invoke<string>('read_text_from_clipboard');
}

/** Native-first clipboard read for browser previews and non-Tauri test hosts. */
export async function readTextWithFallback(): Promise<string> {
  try {
    const value = await readTextFromClipboard();
    if (typeof value === 'string') return value;
    throw new Error('native clipboard returned an invalid value');
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return navigator.clipboard?.readText?.() ?? '';
  }
}

/** Native-first clipboard write with a browser fallback for previews. */
export async function copyTextWithFallback(text: string): Promise<void> {
  try {
    await copyTextToClipboard(text);
    return;
  } catch (nativeError) {
    if (isTauriRuntime()) throw nativeError;
    try {
      const writer = navigator.clipboard?.writeText;
      if (typeof writer !== 'function') throw new Error('browser clipboard unavailable');
      await writer.call(navigator.clipboard, text);
      return;
    } catch {
      throw nativeError;
    }
  }
}

