import '@testing-library/jest-dom/vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { vi } from 'vitest';
import { afterEach, describe, expect, it } from 'vitest';
import { useRef } from 'react';
import { useShortcutEngine } from './useShortcutEngine';
import { useHistory } from './useHistory';
import { DEFAULT_SHORTCUT_CONFIG } from '../types/shortcut';

function ShortcutUndoHarness() {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const {
    value,
    setValue,
    undo,
    redo,
    canUndo,
    canRedo,
  } = useHistory('', { maxSteps: 20, debounceMs: 200 });

  useShortcutEngine({
    enabled: true,
    textareaRef,
    config: DEFAULT_SHORTCUT_CONFIG,
    onSave: () => undefined,
    onClose: () => undefined,
    onChange: ({ value: nextValue }) => setValue(nextValue),
    onUndo: undo,
    onRedo: redo,
    canUndo,
    canRedo,
  });

  return (
    <textarea
      ref={textareaRef}
      aria-label="manager-editor"
      value={value}
      onChange={(event) => setValue(event.target.value)}
    />
  );
}

describe('useShortcutEngine', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('supports undo and redo on focused textarea editors', () => {
    vi.useFakeTimers();

    render(<ShortcutUndoHarness />);

    const textarea = screen.getByLabelText('manager-editor');
    textarea.focus();

    fireEvent.change(textarea, { target: { value: 'first' } });
    act(() => {
      vi.advanceTimersByTime(250);
    });

    fireEvent.change(textarea, { target: { value: 'second' } });
    act(() => {
      vi.advanceTimersByTime(250);
    });

    fireEvent.keyDown(textarea, { key: 'z', ctrlKey: true });
    expect(textarea).toHaveValue('first');

    fireEvent.keyDown(textarea, { key: 'z', ctrlKey: true, shiftKey: true });
    expect(textarea).toHaveValue('second');
  });
});
