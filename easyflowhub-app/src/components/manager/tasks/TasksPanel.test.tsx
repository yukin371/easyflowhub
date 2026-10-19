import '@testing-library/jest-dom/vitest';
import { act, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tasksApi } from '../../../lib/api/scriptmgr';
import type { TaskInfo } from '../../../types/scriptmgr';
import { TasksPanel } from './TasksPanel';

vi.mock('../../../lib/api/scriptmgr', () => ({
  tasksApi: { list: vi.fn(), cancel: vi.fn(), log: vi.fn() },
}));

const listMock = vi.mocked(tasksApi.list);

function runningTask(): TaskInfo {
  return {
    task_id: 'task-1',
    script_id: 'demo',
    status: 'running',
    created_at: '2026-10-19T00:00:00.000Z',
  } as TaskInfo;
}

describe('TasksPanel polling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    listMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
    Object.defineProperty(document, 'hidden', { configurable: true, value: false });
  });

  it('polls quickly while a task is running', async () => {
    listMock.mockResolvedValue({ ok: true, count: 1, tasks: [runningTask()], generated_at: 'now' });
    render(<TasksPanel />);
    await act(async () => {
      await Promise.resolve();
    });

    const initialCalls = listMock.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(listMock.mock.calls.length).toBeGreaterThan(initialCalls);
  });

  it('never polls while the window is hidden', async () => {
    Object.defineProperty(document, 'hidden', { configurable: true, value: true });
    listMock.mockResolvedValue({ ok: true, count: 1, tasks: [runningTask()], generated_at: 'now' });
    render(<TasksPanel />);
    await act(async () => {
      await Promise.resolve();
    });

    const callsWhileHidden = listMock.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(60_000);
    });
    expect(listMock.mock.calls.length).toBe(callsWhileHidden);
  });
});
