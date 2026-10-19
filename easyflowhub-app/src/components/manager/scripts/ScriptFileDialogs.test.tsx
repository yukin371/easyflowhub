import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { scriptsApi, tasksApi } from '../../../lib/api/scriptmgr';
import type { ScriptSummary } from '../../../types/scriptmgr';
import { ScriptWorkbenchDialog } from './ScriptFileDialogs';

vi.mock('../../../lib/api/scriptmgr', () => ({
  scriptsApi: {
    readSource: vi.fn(),
    saveSource: vi.fn(),
    remove: vi.fn(),
    create: vi.fn(),
    importFiles: vi.fn(),
  },
  tasksApi: { run: vi.fn() },
  rootsApi: { list: vi.fn(), add: vi.fn(), remove: vi.fn() },
  scriptTrashApi: { list: vi.fn(), restore: vi.fn(), purge: vi.fn(), empty: vi.fn() },
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const script: ScriptSummary = {
  id: 'demo',
  name: 'Demo',
  path: 'E:/scripts/demo.py',
  script_type: 'python',
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(scriptsApi.readSource).mockResolvedValue({
    id: 'demo',
    path: script.path,
    content: 'print(1)\n',
    language: 'python',
    size_bytes: 9,
    modified_at: '2026-10-19T00:00:00Z',
  });
  vi.mocked(scriptsApi.saveSource).mockResolvedValue({
    id: 'demo',
    path: script.path,
    content: 'print(2)\n',
    language: 'python',
    size_bytes: 9,
    modified_at: '2026-10-19T00:00:01Z',
  });
  vi.mocked(tasksApi.run).mockResolvedValue({
    ok: true,
    status: 'success',
    script_id: 'demo',
    script_name: 'Demo',
    exit_code: 0,
    output: 'hello\n',
    duration_ms: 12,
  });
});

describe('ScriptWorkbenchDialog', () => {
  it('loads the source and saves edits', async () => {
    const onChanged = vi.fn();
    render(<ScriptWorkbenchDialog script={script} onClose={vi.fn()} onChanged={onChanged} />);

    const editor = (await screen.findByLabelText('脚本源码')) as HTMLTextAreaElement;
    expect(editor.value).toBe('print(1)\n');

    fireEvent.change(editor, { target: { value: 'print(2)\n' } });
    fireEvent.click(screen.getByRole('button', { name: '保存修改' }));

    await waitFor(() => expect(scriptsApi.saveSource).toHaveBeenCalledWith('demo', 'print(2)\n'));
    expect(onChanged).toHaveBeenCalled();
    expect(await screen.findByRole('status')).toHaveTextContent('已保存');
  });

  it('runs the script with args and shows the captured output', async () => {
    render(<ScriptWorkbenchDialog script={script} onClose={vi.fn()} onChanged={vi.fn()} />);
    await screen.findByLabelText('脚本源码');

    fireEvent.click(screen.getByRole('tab', { name: '运行' }));
    fireEvent.change(screen.getByLabelText('脚本参数'), { target: { value: '--dry-run  input.txt' } });
    fireEvent.click(screen.getByRole('button', { name: '执行脚本' }));

    await waitFor(() =>
      expect(tasksApi.run).toHaveBeenCalledWith({
        script_id: 'demo',
        args: ['--dry-run', 'input.txt'],
        async: false,
      })
    );
    expect(await screen.findByLabelText('执行结果')).toHaveTextContent('hello');
    expect(screen.getByText('执行成功')).toBeInTheDocument();
    expect(screen.getByText('退出码 0')).toBeInTheDocument();
  });

  it('moves the script to the trash after confirmation', async () => {
    const onClose = vi.fn();
    const onChanged = vi.fn();
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(scriptsApi.remove).mockResolvedValue({
      id: 'trash-1',
      name: 'Demo',
      original_path: script.path,
      script_type: 'python',
      deleted_at: '2026-10-19T00:00:02Z',
      size_bytes: 9,
    });

    render(<ScriptWorkbenchDialog script={script} onClose={onClose} onChanged={onChanged} />);
    await screen.findByLabelText('脚本源码');
    fireEvent.click(screen.getByRole('button', { name: '删除到回收站' }));

    await waitFor(() => expect(scriptsApi.remove).toHaveBeenCalledWith('demo'));
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(onChanged).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it('keeps the script when the delete confirmation is cancelled', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<ScriptWorkbenchDialog script={script} onClose={vi.fn()} onChanged={vi.fn()} />);
    await screen.findByLabelText('脚本源码');

    fireEvent.click(screen.getByRole('button', { name: '删除到回收站' }));
    expect(scriptsApi.remove).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
