export interface ShortcutConfig {
  save: string;
  close: string;
  quickSwitcher: string;
  toggleAlwaysOnTop: string;
  cutLine: string;
  deleteLine: string;
  duplicateLine: string;
  insertLineBelow: string;
  insertLineAbove: string;
  heading1: string;
  heading2: string;
  heading3: string;
  clearHeading: string;
}

export const DEFAULT_SHORTCUT_CONFIG: ShortcutConfig = {
  save: 'Ctrl+S',
  close: 'Escape',
  quickSwitcher: 'Ctrl+K',
  toggleAlwaysOnTop: 'Ctrl+T',
  cutLine: 'Ctrl+Shift+X',
  deleteLine: 'Ctrl+Shift+Backspace',
  duplicateLine: 'Ctrl+Shift+D',
  insertLineBelow: 'Ctrl+Enter',
  insertLineAbove: 'Ctrl+Shift+Enter',
  heading1: 'Ctrl+Alt+1',
  heading2: 'Ctrl+Alt+2',
  heading3: 'Ctrl+Alt+3',
  clearHeading: 'Ctrl+Alt+0',
};

export const SHORTCUT_STORAGE_KEY = 'easyflowhub_shortcut_config_v2';

function formatKeyName(key: string): string {
  const lower = key.toLowerCase();
  if (lower === ' ') return 'Space';
  if (lower === 'escape') return 'Escape';
  if (lower === 'enter') return 'Enter';
  if (lower === 'arrowup') return 'ArrowUp';
  if (lower === 'arrowdown') return 'ArrowDown';
  if (lower === 'arrowleft') return 'ArrowLeft';
  if (lower === 'arrowright') return 'ArrowRight';
  if (lower.length === 1) return lower.toUpperCase();
  return key.length > 1 ? key[0].toUpperCase() + key.slice(1) : key;
}

export function formatShortcutFromKeyboardEvent(event: KeyboardEvent): string | null {
  const key = event.key;
  if (!key || key === 'Control' || key === 'Shift' || key === 'Alt' || key === 'Meta') {
    return null;
  }

  const tokens: string[] = [];
  if (event.ctrlKey || event.metaKey) tokens.push('Ctrl');
  if (event.shiftKey) tokens.push('Shift');
  if (event.altKey) tokens.push('Alt');
  tokens.push(formatKeyName(key));
  return tokens.join('+');
}

function normalizeShortcutKey(key: string): string {
  const lower = key.toLowerCase();
  if (lower === 'esc') return 'escape';
  if (lower === 'return') return 'enter';
  return lower;
}

export function matchesShortcutEvent(event: KeyboardEvent, shortcut: string): boolean {
  const tokens = shortcut
    .split('+')
    .map((token) => token.trim())
    .filter(Boolean)
    .map(normalizeShortcutKey);

  if (tokens.length === 0) {
    return false;
  }

  const key = tokens[tokens.length - 1];
  const ctrl = tokens.includes('ctrl') || tokens.includes('cmd') || tokens.includes('meta');
  const shift = tokens.includes('shift');
  const alt = tokens.includes('alt');
  const ctrlPressed = event.ctrlKey || event.metaKey;

  return (
    ctrlPressed === ctrl &&
    event.shiftKey === shift &&
    event.altKey === alt &&
    normalizeShortcutKey(event.key) === key
  );
}

export function loadShortcutConfig(): ShortcutConfig {
  try {
    const raw =
      localStorage.getItem(SHORTCUT_STORAGE_KEY) ??
      localStorage.getItem('easyflowhub_shortcut_config_v1');
    if (!raw) {
      return DEFAULT_SHORTCUT_CONFIG;
    }

    const parsed = JSON.parse(raw) as Partial<ShortcutConfig>;
    const merged = {
      ...DEFAULT_SHORTCUT_CONFIG,
      ...parsed,
    };

    // Migrate legacy defaults that hijacked native editor shortcuts.
    if (parsed.cutLine?.trim() === 'Ctrl+X') {
      merged.cutLine = DEFAULT_SHORTCUT_CONFIG.cutLine;
    }
    if (parsed.deleteLine?.trim() === 'Ctrl+D') {
      merged.deleteLine = DEFAULT_SHORTCUT_CONFIG.deleteLine;
    }

    return {
      ...merged,
    };
  } catch {
    return DEFAULT_SHORTCUT_CONFIG;
  }
}
