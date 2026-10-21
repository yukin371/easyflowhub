import { useCallback, useEffect, useState } from 'react';
import { emit, listen } from '@tauri-apps/api/event';
import {
  EXPERIMENTAL_FEATURES_CHANGED_EVENT,
  isTranslationEnabled,
  setTranslationEnabled,
} from '../lib/experimental';

/**
 * React binding for the experimental translation flag.
 *
 * Returns the current enabled state plus a setter that persists the flag and
 * broadcasts it so the other window (Launcher / Manager) re-reads immediately.
 * Both windows mount this hook; whichever flips the switch calls `setEnabled`,
 * and the change event keeps every mounted instance in sync.
 */
export function useTranslationExperimental(): {
  enabled: boolean;
  setEnabled: (enabled: boolean) => void;
} {
  const [enabled, setEnabledState] = useState<boolean>(() => isTranslationEnabled());

  useEffect(() => {
    let mounted = true;

    // Re-read on mount in case another window changed the flag while this one
    // was hidden (Launcher is hidden/shown, not remounted).
    setEnabledState(isTranslationEnabled());

    const unlisten = listen(EXPERIMENTAL_FEATURES_CHANGED_EVENT, () => {
      if (mounted) {
        setEnabledState(isTranslationEnabled());
      }
    }).catch(() => {
      // Browser preview / test env has no native event bridge.
    });

    return () => {
      mounted = false;
      void unlisten.then((fn) => fn?.());
    };
  }, []);

  const setEnabled = useCallback((next: boolean) => {
    const stored = setTranslationEnabled(next);
    setEnabledState(stored);
    // Best-effort broadcast; the emitting window already updated above.
    void emit(EXPERIMENTAL_FEATURES_CHANGED_EVENT, { translation: stored }).catch(() => undefined);
  }, []);

  return { enabled, setEnabled };
}
