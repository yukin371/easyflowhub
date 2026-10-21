import { beforeEach, describe, expect, it } from 'vitest';
import {
  isTranslationEnabled,
  setTranslationEnabled,
  TRANSLATION_EXPERIMENTAL_KEY,
} from './experimental';

describe('experimental translation flag', () => {
  beforeEach(() => {
    localStorage.removeItem(TRANSLATION_EXPERIMENTAL_KEY);
  });

  it('defaults to disabled', () => {
    expect(isTranslationEnabled()).toBe(false);
  });

  it('persists enabling to localStorage', () => {
    expect(setTranslationEnabled(true)).toBe(true);
    expect(isTranslationEnabled()).toBe(true);
    expect(localStorage.getItem(TRANSLATION_EXPERIMENTAL_KEY)).toBe('true');
  });

  it('clears the key when disabled so the default stays off', () => {
    setTranslationEnabled(true);
    expect(setTranslationEnabled(false)).toBe(false);
    expect(isTranslationEnabled()).toBe(false);
    expect(localStorage.getItem(TRANSLATION_EXPERIMENTAL_KEY)).toBeNull();
  });
});
