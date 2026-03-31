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

export interface AppSettings {
  quick_note: QuickNoteSettings;
  trash: TrashSettings;
  editor: EditorSettings;
  todo: TodoSettings;
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
};
