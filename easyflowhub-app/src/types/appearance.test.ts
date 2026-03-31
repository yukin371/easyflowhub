import { describe, it, expect } from 'vitest';
import {
  DEFAULT_APPEARANCE,
  AVAILABLE_FONTS,
  THEME_OPTIONS,
} from './appearance';

describe('appearance types', () => {
  describe('DEFAULT_APPEARANCE', () => {
    it('should have correct default theme', () => {
      expect(DEFAULT_APPEARANCE.theme).toBe('dark');
    });

    it('should have correct default window settings', () => {
      expect(DEFAULT_APPEARANCE.window.opacity).toBe(0.8);
      expect(DEFAULT_APPEARANCE.window.cornerRadius).toBe(8);
      expect(DEFAULT_APPEARANCE.window.isAlwaysOnTop).toBe(true);
      expect(DEFAULT_APPEARANCE.window.isBlurEnabled).toBe(true);
    });

    it('should have correct default font settings', () => {
      expect(DEFAULT_APPEARANCE.font.fontFamily).toBe('Consolas');
      expect(DEFAULT_APPEARANCE.font.fontSize).toBe(14);
      expect(DEFAULT_APPEARANCE.font.lineHeight).toBe(1.6);
    });
  });

  describe('THEME_OPTIONS', () => {
    it('should have 3 theme options', () => {
      expect(THEME_OPTIONS).toHaveLength(3);
    });

    it('should have dark theme as first option', () => {
      expect(THEME_OPTIONS[0].id).toBe('dark');
      expect(THEME_OPTIONS[0].name).toBe('深色');
      expect(THEME_OPTIONS[0].icon).toBe('🌙');
    });

    it('should have light theme as second option', () => {
      expect(THEME_OPTIONS[1].id).toBe('light');
      expect(THEME_OPTIONS[1].name).toBe('浅色');
      expect(THEME_OPTIONS[1].icon).toBe('☀️');
    });

    it('should have system theme as third option', () => {
      expect(THEME_OPTIONS[2].id).toBe('system');
      expect(THEME_OPTIONS[2].name).toBe('系统');
      expect(THEME_OPTIONS[2].icon).toBe('⚙️');
    });
  });

  describe('AVAILABLE_FONTS', () => {
    it('should contain common monospace fonts', () => {
      expect(AVAILABLE_FONTS).toContain('Consolas');
      expect(AVAILABLE_FONTS).toContain('Monaco');
      expect(AVAILABLE_FONTS).toContain('Courier New');
    });

    it('should contain Chinese fonts', () => {
      expect(AVAILABLE_FONTS).toContain('Microsoft YaHei');
      expect(AVAILABLE_FONTS).toContain('PingFang SC');
    });

    it('should contain system fonts', () => {
      expect(AVAILABLE_FONTS).toContain('Segoe UI');
      expect(AVAILABLE_FONTS).toContain('Arial');
    });

    it('should be a readonly tuple', () => {
      expect(Array.isArray(AVAILABLE_FONTS)).toBe(true);
    });
  });
});
