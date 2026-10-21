/**
 * Experimental (preview) feature flags for capabilities that are not backed by
 * the module registry.
 *
 * 文件 / 下载 / 录音 are modules, so their enable state already lives in the
 * module registry (persisted natively). 翻译 is different: it is a Launcher
 * action plus a settings section, not a sidebar module, so it needs its own
 * flag.
 *
 * The flag is stored in localStorage and broadcast over a Tauri event so the
 * Launcher window (a separate webview) can re-register its translation action
 * the moment the user flips the switch in Manager. localStorage itself is
 * shared per-origin across the app's webviews, but it does not emit change
 * events across windows, hence the explicit event.
 */

/** localStorage key for the translation experimental flag. */
export const TRANSLATION_EXPERIMENTAL_KEY = 'easyflowhub.experimental.translation';

/** Cross-window event fired whenever any experimental flag changes. */
export const EXPERIMENTAL_FEATURES_CHANGED_EVENT = 'easyflowhub:experimental-features-changed';

function readTranslationFlag(): boolean {
  try {
    return globalThis.localStorage?.getItem(TRANSLATION_EXPERIMENTAL_KEY) === 'true';
  } catch {
    return false;
  }
}

/** Whether the experimental translation feature is currently enabled. */
export function isTranslationEnabled(): boolean {
  return readTranslationFlag();
}

/**
 * Persist the translation experimental flag. Returns the value that is now
 * stored, so callers can ignore rejected writes (e.g. storage disabled).
 */
export function setTranslationEnabled(enabled: boolean): boolean {
  try {
    if (enabled) {
      globalThis.localStorage?.setItem(TRANSLATION_EXPERIMENTAL_KEY, 'true');
    } else {
      globalThis.localStorage?.removeItem(TRANSLATION_EXPERIMENTAL_KEY);
    }
  } catch {
    // Storage may be unavailable (private mode / browser preview). Fall through
    // so the in-memory value below still reflects the request for this session.
  }
  return readTranslationFlag();
}
