import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  loadAppearanceConfig,
  saveAppearanceConfig,
  resetAppearanceConfig,
  applyAlwaysOnTop,
  applyWindowOpacity,
  getWindowState,
} from './appearance';
import { DEFAULT_APPEARANCE, type AppearanceConfig } from '../../types/appearance';

// Mock Tauri invoke
const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
}));

// Store original localStorage
const originalLocalStorage = globalThis.localStorage;

describe('appearance IPC layer', () => {
  let mockLocalStorage: Storage;
  let store: Record<string, string>;

  beforeEach(() => {
    vi.clearAllMocks();

    // Create a fresh mock localStorage for each test
    store = {};
    mockLocalStorage = {
      getItem: (key: string) => store[key] ?? null,
      setItem: (key: string, value: string) => { store[key] = value; },
      removeItem: (key: string) => { delete store[key]; },
      clear: () => { store = {}; },
      get length() { return Object.keys(store).length; },
      key: (index: number) => Object.keys(store)[index] ?? null,
    };

    // Replace global localStorage
    Object.defineProperty(globalThis, 'localStorage', {
      value: mockLocalStorage,
      writable: true,
      configurable: true,
    });
  });

  afterEach(() => {
    // Restore original localStorage
    Object.defineProperty(globalThis, 'localStorage', {
      value: originalLocalStorage,
      writable: true,
      configurable: true,
    });
  });

  describe('loadAppearanceConfig', () => {
    it('should return default config when localStorage is empty', async () => {
      const result = await loadAppearanceConfig();
      expect(result).toEqual(DEFAULT_APPEARANCE);
    });

    it('should return saved config when localStorage has data', async () => {
      const savedConfig: AppearanceConfig = {
        theme: 'light',
        window: { opacity: 0.9, cornerRadius: 12, isAlwaysOnTop: false, isBlurEnabled: false },
        font: { fontFamily: 'Monaco', fontSize: 16, lineHeight: 1.8 },
      };
      store['easyflowhub-appearance'] = JSON.stringify(savedConfig);

      const result = await loadAppearanceConfig();
      expect(result.theme).toBe('light');
      expect(result.window.opacity).toBe(0.9);
      expect(result.window.cornerRadius).toBe(12);
      expect(result.font.fontFamily).toBe('Monaco');
      expect(result.font.fontSize).toBe(16);
    });

    it('should merge with defaults when localStorage has partial data', async () => {
      const partialConfig = { theme: 'light' };
      store['easyflowhub-appearance'] = JSON.stringify(partialConfig);

      const result = await loadAppearanceConfig();
      expect(result.theme).toBe('light');
      expect(result.window.opacity).toBe(DEFAULT_APPEARANCE.window.opacity);
      expect(result.window.cornerRadius).toBe(DEFAULT_APPEARANCE.window.cornerRadius);
      expect(result.font.fontFamily).toBe(DEFAULT_APPEARANCE.font.fontFamily);
    });

    it('should handle JSON parse error gracefully', async () => {
      store['easyflowhub-appearance'] = 'invalid json';

      const result = await loadAppearanceConfig();
      expect(result).toEqual(DEFAULT_APPEARANCE);
    });
  });

  describe('saveAppearanceConfig', () => {
    it('should save config to localStorage', () => {
      const config: AppearanceConfig = {
        ...DEFAULT_APPEARANCE,
        theme: 'system',
      };
      saveAppearanceConfig(config);
      expect(store['easyflowhub-appearance']).toBe(JSON.stringify(config));
    });
  });

  describe('resetAppearanceConfig', () => {
    it('should remove config from localStorage and return default', () => {
      store['easyflowhub-appearance'] = JSON.stringify({ theme: 'light' });
      const result = resetAppearanceConfig();
      expect(store['easyflowhub-appearance']).toBeUndefined();
      expect(result).toEqual(DEFAULT_APPEARANCE);
    });
  });

  describe('applyAlwaysOnTop', () => {
    it('should call invoke with correct parameters', async () => {
      mockInvoke.mockResolvedValue(true);
      const result = await applyAlwaysOnTop(true);
      expect(mockInvoke).toHaveBeenCalledWith('set_always_on_top', { enabled: true });
      expect(result).toBe(true);
    });

    it('should call invoke with false parameter', async () => {
      mockInvoke.mockResolvedValue(false);
      const result = await applyAlwaysOnTop(false);
      expect(mockInvoke).toHaveBeenCalledWith('set_always_on_top', { enabled: false });
      expect(result).toBe(false);
    });

    it('should throw on invoke error', async () => {
      mockInvoke.mockRejectedValue(new Error('Invoke failed'));
      await expect(applyAlwaysOnTop(true)).rejects.toThrow('Invoke failed');
    });
  });

  describe('applyWindowOpacity', () => {
    it('should call invoke and set CSS variable', async () => {
      mockInvoke.mockResolvedValue(undefined);
      await applyWindowOpacity(0.8);
      expect(mockInvoke).toHaveBeenCalledWith('set_window_opacity', { opacity: 0.8 });
      expect(document.documentElement.style.getPropertyValue('--window-opacity')).toBe('0.8');
    });

    it('should throw on invoke error', async () => {
      mockInvoke.mockRejectedValue(new Error('Invoke failed'));
      await expect(applyWindowOpacity(0.8)).rejects.toThrow('Invoke failed');
    });
  });

  describe('getWindowState', () => {
    it('should return window state from invoke', async () => {
      mockInvoke.mockResolvedValue({ isAlwaysOnTop: true });
      const result = await getWindowState();
      expect(mockInvoke).toHaveBeenCalledWith('get_window_state', undefined);
      expect(result.isAlwaysOnTop).toBe(true);
    });

    it('should return default state on invoke error', async () => {
      mockInvoke.mockRejectedValue(new Error('Invoke failed'));
      const result = await getWindowState();
      expect(result.isAlwaysOnTop).toBe(false);
    });
  });
});
