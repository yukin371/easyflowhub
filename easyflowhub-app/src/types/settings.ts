/**
 * Settings Types
 * Matches the Rust settings module
 */

export interface QuickNoteSettings {
  width: number;
  height: number;
}

export interface TrashSettings {
  retention_days: number;
}

export interface EditorSettings {
  undo_steps: number;
  cursor_style: 'default' | 'accent' | 'focus';
  cursor_color: string;
  cursor_trail: boolean;
}

export interface TodoSettings {
  done_retention_hours: number;
}

export interface TranslationSettings {
  target_language: 'zh-CN' | 'en' | 'ja' | 'ko';
  model: string;
}

/**
 * Transcription preferences for post-recording speech-to-text.
 * Only preferences are persisted; provider credentials remain owned by Relay.
 */
export interface TranscriptionSettings {
  model: string;
  /** Optional language hint sent to the provider; empty means auto-detect. */
  language: string;
}

/** Native persisted representation used by the Tauri settings command. */
export interface LauncherTriggerSettings {
  enabled: boolean;
  trigger: 'keyboard' | 'mouse_middle' | 'keyboard_or_mouse_middle';
  keyboard_shortcut: string;
  excluded_apps: string[];
}

/**
 * Developer / diagnostics preferences. Debug mode only controls log verbosity
 * and file output; it never bypasses safety rules.
 */
export interface DeveloperSettings {
  debug_mode: boolean;
}

export interface AppSettings {
  quick_note: QuickNoteSettings;
  trash: TrashSettings;
  editor: EditorSettings;
  todo: TodoSettings;
  launcher_trigger: LauncherTriggerSettings;
  translation: TranslationSettings;
  transcription: TranscriptionSettings;
  developer: DeveloperSettings;
}

export interface GetSettingsResponse {
  ok: boolean;
  settings: AppSettings;
}

export interface UpdateSettingsResponse {
  ok: boolean;
  settings: AppSettings;
}

export const DEFAULT_SETTINGS: AppSettings = {
  quick_note: {
    width: 400,
    height: 300,
  },
  trash: {
    retention_days: 30,
  },
  editor: {
    undo_steps: 100,
    cursor_style: 'accent',
    cursor_color: '#4f5a43',
    cursor_trail: true,
  },
  todo: {
    done_retention_hours: 24,
  },
  launcher_trigger: {
    enabled: true,
    trigger: 'keyboard_or_mouse_middle',
    keyboard_shortcut: 'Ctrl+Alt+Space',
    excluded_apps: [],
  },
  translation: {
    target_language: 'zh-CN',
    model: '',
  },
  transcription: {
    model: '',
    language: '',
  },
  developer: {
    debug_mode: false,
  },
};
