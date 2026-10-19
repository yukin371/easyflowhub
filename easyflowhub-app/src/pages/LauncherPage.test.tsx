import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { LauncherPage } from './LauncherPage';
import { translateText } from '../features/translation';
import { actionRegistry } from '../features/actions';
import { DEFAULT_SETTINGS } from '../types/settings';
import { describeScript, listScripts, runScript } from '../lib/tauri/scriptmgr';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock('../lib/tauri/clipboard', () => ({
  readTextWithFallback: vi.fn(async () => navigator.clipboard?.readText?.() ?? ''),
  copyTextWithFallback: vi.fn(async (text: string) => navigator.clipboard?.writeText?.(text)),
}));
vi.mock('../lib/tauri/scriptmgr', () => ({
  listScripts: vi.fn(async () => []),
  describeScript: vi.fn(),
  runScript: vi.fn(),
}));
vi.mock('../features/translation', () => ({ translateText: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe('LauncherPage native window interactions', () => {
  beforeEach(() => {
    actionRegistry.clear();
    vi.mocked(translateText).mockReset();
    vi.mocked(listScripts).mockReset().mockResolvedValue([]);
    vi.mocked(describeScript).mockReset();
    vi.mocked(runScript).mockReset();
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { readText: vi.fn(async () => ''), writeText: vi.fn(async () => undefined) } });
    invokeMock.mockReset();
    vi.mocked(listen).mockClear();
    invokeMock.mockImplementation(async (command) => {
      if (command === 'get_launcher_always_on_top') return false;
      if (command === 'get_settings') return { ok: true, settings: { ...DEFAULT_SETTINGS, translation: { target_language: 'en', model: 'my-relay-model' } } };
      if (command === 'capture_inbox_item') return { ok: true, item: { id: 'test-inbox-item' } };
      return undefined;
    });
  });

  it('native focus events restore search focus without clearing the query', async () => {
    render(<LauncherPage />);
    const input = await screen.findByRole('combobox', { name: '搜索动作、脚本或工具' });
    fireEvent.change(input, { target: { value: 'snow' } });
    screen.getByRole('button', { name: '搜索动作' }).focus();

    await waitFor(() => expect(listen).toHaveBeenCalledWith('launcher-focused', expect.any(Function)));
    const focusListener = vi.mocked(listen).mock.calls.find(([event]) => event === 'launcher-focused')?.[1];
    focusListener?.({ event: 'launcher-focused', id: 0, payload: undefined } as never);

    await waitFor(() => expect(input).toHaveFocus());
    expect(input).toHaveValue('snow');
    expect(document.documentElement.dataset.window).toBe('launcher');
  });

  it('标题拖动只请求移动窗口；失焦不关闭，关闭按钮才请求隐藏', async () => {
    render(<LauncherPage />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('get_launcher_always_on_top'));
    fireEvent.mouseDown(document.querySelector('.launcher-header')!, { button: 0 });
    fireEvent.blur(window);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('start_launcher_dragging'));
    expect(invokeMock.mock.calls.filter(([command]) => command === 'start_launcher_dragging')).toHaveLength(1);
    expect(invokeMock).not.toHaveBeenCalledWith('hide_launcher_window');

    fireEvent.click(screen.getByRole('button', { name: '关闭快速动作面板' }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('hide_launcher_window'));
  });

  it('从原生窗口同步置顶状态，页面重载后取消置顶仍发送正确值', async () => {
    invokeMock.mockImplementation(async (command, args) => {
      if (command === 'get_launcher_always_on_top') return true;
      if (command === 'set_launcher_always_on_top') return (args as { enabled: boolean }).enabled;
      return undefined;
    });
    render(<LauncherPage />);
    const pin = await screen.findByRole('button', { name: '取消置顶快速动作面板' });
    fireEvent.click(pin);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('set_launcher_always_on_top', { enabled: false }));
    expect(await screen.findByRole('button', { name: '置顶快速动作面板' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('置顶失败时保留原状态并展示错误，避免按钮与原生窗口状态不一致', async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === 'get_launcher_always_on_top') return false;
      throw new Error('native command unavailable');
    });
    render(<LauncherPage />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('get_launcher_always_on_top'));
    fireEvent.click(screen.getByRole('button', { name: '置顶快速动作面板' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('无法切换置顶状态');
    expect(screen.getByRole('button', { name: '置顶快速动作面板' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('原生拖动失败时展示错误，便于识别旧进程或命令不可用', async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === 'get_launcher_always_on_top') return false;
      throw new Error('native command unavailable');
    });
    render(<LauncherPage />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('get_launcher_always_on_top'));
    fireEvent.mouseDown(document.querySelector('.launcher-header')!, { button: 0 });
    expect(await screen.findByRole('alert')).toHaveTextContent('无法拖动快捷窗口');
    expect(invokeMock).not.toHaveBeenCalledWith('hide_launcher_window');
  });

  it('一键收集剪贴板内容到 Inbox', async () => {
    const readText = vi.fn().mockResolvedValueOnce('old clipboard').mockResolvedValueOnce('latest clipboard');
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { readText },
    });
    invokeMock.mockImplementation(async (command) => {
      if (command === 'get_launcher_always_on_top') return false;
      if (command === 'capture_inbox_item') return { item: { id: 'item-1' } };
      return undefined;
    });

    render(<LauncherPage />);
    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    const openListener = vi.mocked(listen).mock.calls.find(([event]) => event === 'launcher-opened')?.[1];
    openListener?.({ event: 'launcher-opened', id: 0, payload: undefined } as never);
    await waitFor(() => expect(readText).toHaveBeenCalledTimes(2));
    fireEvent.click(await screen.findByRole('option', { name: '收集剪贴板内容' }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('capture_inbox_item', {
      input: { content: 'latest clipboard', source: 'launcher', context: 'clipboard' },
    }));
  });

  it('Snow Shot 复制截图后可直接通过 Launcher 收集到 Inbox', async () => {
    const imageBlob = new Blob(['png payload'], { type: 'image/png' });
    const read = vi.fn().mockResolvedValue([{
      types: ['image/png'],
      getType: vi.fn().mockResolvedValue(imageBlob),
    }]);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { readText: vi.fn().mockResolvedValue(''), read, writeText: vi.fn() },
    });
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '收集剪贴板内容' }));

    await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('capture_inbox_item', {
      input: {
        imageDataUrl: expect.stringMatching(/^data:image\/png;base64,/),
        source: 'launcher',
        context: 'clipboard-image',
      },
    }));
    expect(await screen.findByRole('status')).toHaveTextContent(/图片已保存到收集箱|已保存到收集箱/);
  });

  it('剪贴板截图超过限制时阻止归档并显示可恢复错误', async () => {
    const oversized = new Blob(['payload'], { type: 'image/png' });
    Object.defineProperty(oversized, 'size', { value: 32 * 1024 * 1024 + 1 });
    const read = vi.fn().mockResolvedValue([{
      types: ['image/png'],
      getType: vi.fn().mockResolvedValue(oversized),
    }]);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { readText: vi.fn().mockResolvedValue('fallback text'), read, writeText: vi.fn() },
    });
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '收集剪贴板内容' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('超过 32 MB');
    expect(invokeMock.mock.calls.some(([command]) => command === 'capture_inbox_item')).toBe(false);
  });

  it('通过 Relay 翻译剪贴板，复制译文并将原文和译文保存到收集箱', async () => {
    vi.mocked(navigator.clipboard.readText).mockResolvedValue('你好，世界');
    vi.mocked(translateText).mockResolvedValue({ text: 'Hello, world', targetLanguage: 'en', model: 'my-relay-model' });
    render(<LauncherPage />);
    await waitFor(() => expect(navigator.clipboard.readText).toHaveBeenCalled());
    fireEvent.click(await screen.findByRole('option', { name: '翻译文本' }));
    expect(await screen.findByText('Hello, world')).toBeInTheDocument();
    expect(translateText).toHaveBeenCalledWith({
      text: '你好，世界', targetLanguage: 'en', model: 'my-relay-model', signal: expect.any(AbortSignal),
    });
    fireEvent.click(screen.getByRole('button', { name: '复制结果' }));
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Hello, world'));
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('capture_inbox_item', {
      input: { title: '翻译结果', content: '原文\n你好，世界\n\n译文（en）\nHello, world', source: 'launcher', context: 'translation' },
    }));
    await waitFor(() => expect(screen.getByRole('button', { name: '保存到收集箱' })).toBeDisabled());
  });

  it('空剪贴板无需发送翻译请求', async () => {
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '翻译文本' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('请先复制文本');
    expect(translateText).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalledWith('get_settings');
  });

  it('未配置模型给出设置指引，Provider 失败不保存半成品', async () => {
    vi.mocked(navigator.clipboard.readText).mockResolvedValue('hello');
    invokeMock.mockImplementation(async (command) => {
      if (command === 'get_settings') return { ok: true, settings: DEFAULT_SETTINGS };
      return false;
    });
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '翻译文本' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('设置 → 翻译');
    expect(translateText).not.toHaveBeenCalled();
    expect(invokeMock.mock.calls.some(([command]) => command === 'capture_inbox_item')).toBe(false);
  });

  it('翻译失败显示错误，可重试且不会自动写入收集箱', async () => {
    vi.mocked(navigator.clipboard.readText).mockResolvedValue('hello');
    vi.mocked(translateText).mockRejectedValueOnce(new Error('请求过于频繁，请稍后再试'));
    render(<LauncherPage />);
    const translate = await screen.findByRole('option', { name: '翻译文本' });
    fireEvent.click(translate);
    expect(await screen.findByRole('alert')).toHaveTextContent('请求过于频繁');
    expect(invokeMock.mock.calls.some(([command]) => command === 'capture_inbox_item')).toBe(false);
    vi.mocked(translateText).mockResolvedValueOnce({ text: '你好', targetLanguage: 'en', model: 'my-relay-model' });
    fireEvent.click(translate);
    expect(await screen.findByText('你好')).toBeInTheDocument();
  });

  it('关闭或重新唤起会取消翻译，忽略迟到结果并刷新剪贴板', async () => {
    vi.mocked(navigator.clipboard.readText).mockResolvedValue('hello');
    let finish!: (value: Awaited<ReturnType<typeof translateText>>) => void;
    vi.mocked(translateText).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '翻译文本' }));
    await waitFor(() => expect(translateText).toHaveBeenCalledTimes(1));
    const signal = vi.mocked(translateText).mock.calls[0][0].signal!;
    fireEvent.click(screen.getByRole('button', { name: '关闭快速动作面板' }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('hide_launcher_window'));
    expect(signal.aborted).toBe(true);
    vi.mocked(navigator.clipboard.readText).mockResolvedValue('new clipboard');
    const openListener = vi.mocked(listen).mock.calls.find(([event]) => event === 'launcher-opened')?.[1];
    openListener?.({ event: 'launcher-opened', id: 0, payload: undefined } as never);
    finish({ text: 'old translation', targetLanguage: 'en', model: 'my-relay-model' });
    await waitFor(() => expect(navigator.clipboard.readText).toHaveBeenCalledTimes(2));
    expect(screen.queryByText('old translation')).not.toBeInTheDocument();
    fireEvent.click(await screen.findByRole('option', { name: '收集剪贴板内容' }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('capture_inbox_item', {
      input: { content: 'new clipboard', source: 'launcher', context: 'clipboard' },
    }));
  });

  it('原生隐藏事件也取消请求并清除上下文和结果', async () => {
    vi.mocked(navigator.clipboard.readText).mockResolvedValue('hello');
    let finish!: (value: Awaited<ReturnType<typeof translateText>>) => void;
    vi.mocked(translateText).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '翻译文本' }));
    await waitFor(() => expect(translateText).toHaveBeenCalledTimes(1));
    const signal = vi.mocked(translateText).mock.calls[0][0].signal!;
    const hiddenListener = vi.mocked(listen).mock.calls.find(([event]) => event === 'launcher-hidden')?.[1];
    expect(hiddenListener).toBeDefined();
    hiddenListener?.({ event: 'launcher-hidden', id: 0, payload: null } as never);
    finish({ text: 'stale hidden result', targetLanguage: 'en', model: 'my-relay-model' });
    await waitFor(() => expect(screen.queryByRole('button', { name: '取消执行' })).not.toBeInTheDocument());
    expect(signal.aborted).toBe(true);
    expect(screen.queryByText('stale hidden result')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('剪贴板上下文')).not.toBeInTheDocument();
  });

  it('脚本文本输出复用同一结果和 Inbox 出口', async () => {
    const script = { id: 'text-script', name: '文本脚本', path: 'C:/test/script.ps1', script_type: 'powershell' };
    vi.mocked(listScripts).mockResolvedValue([script]);
    vi.mocked(describeScript).mockResolvedValue({ ...script, parameters: [] });
    vi.mocked(runScript).mockResolvedValue({
      script_id: script.id, script_name: script.name, exit_code: 0, status: 'completed', succeeded: true, output: 'script output',
    });
    render(<LauncherPage />);
    fireEvent.click(await screen.findByRole('option', { name: '文本脚本' }));
    expect(await screen.findByText('script output')).toBeInTheDocument();
    expect(runScript).toHaveBeenCalledWith('text-script', { args: [] });
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('capture_inbox_item', {
      input: { title: '文本脚本', content: 'script output', source: 'launcher', context: 'action-result' },
    }));
  });
});

