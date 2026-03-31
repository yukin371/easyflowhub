import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useAppearance } from './useAppearance';
import { DEFAULT_APPEARANCE } from '../types/appearance';

// Mock matchMedia
const mockMatchMedia = vi.fn().mockImplementation((query: string) => ({
  matches: false,
  media: query,
  onchange: null,
  addListener: vi.fn(),
  removeListener: vi.fn(),
  addEventListener: vi.fn(),
  removeEventListener: vi.fn(),
  dispatchEvent: vi.fn(),
}));

// Hoisted mock functions
const mockLoadAppearanceConfig = vi.hoisted(() => vi.fn());
const mockSaveAppearanceConfig = vi.hoisted(() => vi.fn());
const mockResetAppearanceConfig = vi.hoisted(() => vi.fn());
const mockApplyAlwaysOnTop = vi.hoisted(() => vi.fn());
const mockApplyWindowOpacity = vi.hoisted(() => vi.fn());

vi.mock('../lib/tauri/appearance', () => ({
  loadAppearanceConfig: mockLoadAppearanceConfig,
  saveAppearanceConfig: mockSaveAppearanceConfig,
  resetAppearanceConfig: mockResetAppearanceConfig,
  applyAlwaysOnTop: mockApplyAlwaysOnTop,
  applyWindowOpacity: mockApplyWindowOpacity,
}));

describe('useAppearance hook', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockLoadAppearanceConfig.mockResolvedValue({ ...DEFAULT_APPEARANCE });
    mockResetAppearanceConfig.mockReturnValue({ ...DEFAULT_APPEARANCE });
    mockApplyAlwaysOnTop.mockResolvedValue(true);
    mockApplyWindowOpacity.mockResolvedValue(undefined);
    window.matchMedia = mockMatchMedia;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should initialize with default config', async () => {
    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    expect(result.current.config).toEqual(DEFAULT_APPEARANCE);
  });

  it('should set theme correctly', async () => {
    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    act(() => {
      result.current.setTheme('light');
    });

    expect(result.current.config?.theme).toBe('light');
    expect(mockSaveAppearanceConfig).toHaveBeenCalled();
  });

  it('should set window appearance', async () => {
    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    act(() => {
      result.current.setWindowAppearance({ opacity: 0.5, cornerRadius: 12 });
    });

    expect(result.current.config?.window.opacity).toBe(0.5);
    expect(result.current.config?.window.cornerRadius).toBe(12);
    expect(mockSaveAppearanceConfig).toHaveBeenCalled();
  });

  it('should set font settings', async () => {
    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    act(() => {
      result.current.setFontSettings({ fontFamily: 'Monaco', fontSize: 16 });
    });

    expect(result.current.config?.font.fontFamily).toBe('Monaco');
    expect(result.current.config?.font.fontSize).toBe(16);
    expect(mockSaveAppearanceConfig).toHaveBeenCalled();
  });

  it('should reset to default config', async () => {
    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    // First modify config
    act(() => {
      result.current.setTheme('light');
    });

    // Then reset
    act(() => {
      result.current.reset();
    });

    expect(result.current.config).toEqual(DEFAULT_APPEARANCE);
    expect(mockResetAppearanceConfig).toHaveBeenCalled();
  });

  it('should resolve system theme correctly', async () => {
    mockMatchMedia.mockImplementation((query: string) => ({
      matches: true, // Dark mode
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    // Set to system mode
    act(() => {
      result.current.setTheme('system');
    });

    expect(result.current.resolvedTheme).toBe('dark');
  });

  it('should resolve light theme', async () => {
    mockMatchMedia.mockImplementation((query: string) => ({
      matches: false, // Light mode
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    // Set to system mode
    act(() => {
      result.current.setTheme('system');
    });

    expect(result.current.resolvedTheme).toBe('light');
  });

  it('should call applyAlwaysOnTop when setting isAlwaysOnTop', async () => {
    mockApplyAlwaysOnTop.mockResolvedValue(true);

    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    act(() => {
      result.current.updateAlwaysOnTop(true);
    });

    expect(mockApplyAlwaysOnTop).toHaveBeenCalledWith(true);
  });

  it('should call applyWindowOpacity when setting opacity', async () => {
    mockApplyWindowOpacity.mockResolvedValue(undefined);

    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    act(() => {
      result.current.updateOpacity(0.5);
    });

    expect(mockApplyWindowOpacity).toHaveBeenCalledWith(0.5);
  });

  it('should provide convenience update methods', async () => {
    const { result } = renderHook(() => useAppearance());

    await waitFor(() => {
      expect(result.current.isLoaded).toBe(true);
    });

    // Test all convenience methods
    act(() => {
      result.current.updateOpacity(0.7);
    });
    expect(result.current.config?.window.opacity).toBe(0.7);

    act(() => {
      result.current.updateCornerRadius(16);
    });
    expect(result.current.config?.window.cornerRadius).toBe(16);

    act(() => {
      result.current.updateBlur(false);
    });
    expect(result.current.config?.window.isBlurEnabled).toBe(false);
  });
});
