import { useCallback, useEffect, useState } from 'react';
import { getSettings } from '../lib/tauri/settings';
import { DEFAULT_SETTINGS, type EditorSettings } from '../types/settings';
import { loadShortcutConfig, type ShortcutConfig } from '../types/shortcut';

interface UseEditorPreferencesOptions {
  refreshEvent?: string;
}

export function useEditorPreferences(options: UseEditorPreferencesOptions = {}) {
  const { refreshEvent } = options;
  const [shortcutConfig, setShortcutConfig] = useState<ShortcutConfig>(loadShortcutConfig);
  const [editorSettings, setEditorSettings] = useState<EditorSettings>(DEFAULT_SETTINGS.editor);

  const refreshEditorPreferences = useCallback(async () => {
    setShortcutConfig(loadShortcutConfig());
    try {
      const settings = await getSettings();
      setEditorSettings(settings.editor);
    } catch (error) {
      console.error('Failed to refresh editor settings:', error);
    }
  }, []);

  useEffect(() => {
    void refreshEditorPreferences();
  }, [refreshEditorPreferences]);

  useEffect(() => {
    const handleRefresh = () => {
      void refreshEditorPreferences();
    };

    window.addEventListener('focus', handleRefresh);
    if (refreshEvent) {
      window.addEventListener(refreshEvent, handleRefresh);
    }

    return () => {
      window.removeEventListener('focus', handleRefresh);
      if (refreshEvent) {
        window.removeEventListener(refreshEvent, handleRefresh);
      }
    };
  }, [refreshEditorPreferences, refreshEvent]);

  return {
    shortcutConfig,
    editorSettings,
    refreshEditorPreferences,
  };
}
