import '@testing-library/jest-dom/vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useHistory } from './useHistory';

function HistoryHarness() {
  const { value, setValue, undo, redo, canUndo, canRedo } = useHistory('', {
    maxSteps: 10,
    debounceMs: 200,
  });

  return (
    <div>
      <input
        aria-label="history-input"
        value={value}
        onChange={(event) => setValue(event.target.value)}
      />
      <button type="button" onClick={undo} disabled={!canUndo}>
        Undo
      </button>
      <button type="button" onClick={redo} disabled={!canRedo}>
        Redo
      </button>
    </div>
  );
}

describe('useHistory', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('records debounced snapshots so undo and redo work again', () => {
    vi.useFakeTimers();

    render(<HistoryHarness />);

    const input = screen.getByLabelText('history-input');
    const undoButton = screen.getByRole('button', { name: 'Undo' });
    const redoButton = screen.getByRole('button', { name: 'Redo' });

    fireEvent.change(input, { target: { value: 'first' } });
    act(() => {
      vi.advanceTimersByTime(250);
    });

    fireEvent.change(input, { target: { value: 'second' } });
    act(() => {
      vi.advanceTimersByTime(250);
    });

    expect(undoButton).toBeEnabled();

    fireEvent.click(undoButton);
    expect(input).toHaveValue('first');

    fireEvent.click(undoButton);
    expect(input).toHaveValue('');

    expect(redoButton).toBeEnabled();
    fireEvent.click(redoButton);
    expect(input).toHaveValue('first');
  });
});
