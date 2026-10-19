import '@testing-library/jest-dom/vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LauncherPanel, type LauncherAction } from './LauncherPanel';
import type { ActionHandlerResult, ActionTextOutput } from '../actions';

const actions: LauncherAction[] = [
  { id: 'capture-note', title: '保存剪贴板到笔记', description: '把当前选中的文本放进 Inbox', category: 'Capture', icon: '✎', keywords: ['笔记', 'inbox'], recent: true },
  { id: 'format-text', title: '格式化选中文本', description: '整理空格、换行和标点', category: 'Transform', icon: '✦', favorite: true },
  { id: 'open-todos', title: '打开今日待办', category: 'Plan', icon: '☑' },
];

function renderPanel(overrides: Partial<React.ComponentProps<typeof LauncherPanel>> = {}) {
  return render(<LauncherPanel actions={actions} onExecute={vi.fn()} onClose={vi.fn()} {...overrides} />);
}
const getSearch = () => screen.getByRole('combobox', { name: '搜索动作、脚本或工具' });
const textOutput: ActionTextOutput = { kind: 'text', title: '翻译结果', text: '今天是美好的一天。\n适合专注工作。', sourceText: 'A good day.', targetLanguage: '简体中文' };
const textResult: ActionHandlerResult = { ok: true, status: 'success', output: textOutput };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

describe('LauncherPanel', () => {
  it('默认为图标工具栏，聚焦折叠输入框而不展示大标题、说明行或常驻搜索', () => {
    renderPanel();
    expect(getSearch()).toHaveFocus();
    expect(getSearch()).toHaveAttribute('aria-expanded', 'false');
    expect(getSearch().parentElement).toHaveClass('is-collapsed');
    expect(screen.queryByRole('heading')).not.toBeInTheDocument();
    expect(screen.queryByText('把当前选中的文本放进 Inbox')).not.toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '所有动作' })).toHaveAttribute('title', '所有动作（Alt+1）');
    expect(screen.getByRole('option', { name: actions[0].title })).toHaveAttribute('title', expect.stringContaining('把当前选中的文本放进 Inbox'));
  });

  it('直接输入首字符即可展开并过滤，仍使用同一个输入节点', () => {
    renderPanel();
    const input = getSearch();
    fireEvent.change(input, { target: { value: '格' } });
    expect(input).toHaveAttribute('aria-expanded', 'true');
    expect(input.parentElement).toHaveClass('is-open');
    expect(getSearch()).toBe(input);
    expect(input).toHaveValue('格');
    expect(input).toHaveFocus();
    expect(screen.getByRole('option', { name: actions[1].title })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: actions[0].title })).not.toBeInTheDocument();
  });

  it('中文输入法组合期间展开但不执行 Enter，提交首个中文字符后可正常搜索', async () => {
    const onExecute = vi.fn().mockResolvedValue(undefined);
    renderPanel({ onExecute });
    const input = getSearch();
    fireEvent.compositionStart(input);
    expect(input).toHaveAttribute('aria-expanded', 'true');
    fireEvent.change(input, { target: { value: '笔' } });
    fireEvent.keyDown(input, { key: 'Enter', isComposing: true, keyCode: 229 });
    expect(onExecute).not.toHaveBeenCalled();
    expect(input).toHaveValue('笔');
    expect(input).toHaveFocus();
    fireEvent.compositionEnd(input, { data: '笔' });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(onExecute).toHaveBeenCalledWith(actions[0], undefined));
  });

  it.each([{ key: '/', ctrlKey: false }, { key: 'f', ctrlKey: true }])('可以用快捷键 $key 展开搜索', (keys) => {
    renderPanel();
    fireEvent.keyDown(getSearch(), keys);
    expect(getSearch()).toHaveAttribute('aria-expanded', 'true');
    expect(getSearch()).toHaveValue('');
  });

  it('搜索图标能展开，收起图标清空并恢复接收输入', () => {
    renderPanel();
    fireEvent.click(screen.getByRole('button', { name: '搜索动作' }));
    expect(getSearch()).toHaveAttribute('aria-expanded', 'true');
    expect(getSearch()).toHaveFocus();
    fireEvent.change(getSearch(), { target: { value: '格式化' } });
    fireEvent.click(screen.getByRole('button', { name: '收起搜索' }));
    expect(getSearch()).toHaveValue('');
    expect(getSearch()).toHaveAttribute('aria-expanded', 'false');
    expect(getSearch()).toHaveFocus();
    expect(screen.getAllByRole('option')).toHaveLength(3);
  });

  it('卡片聚焦后直接打字也能搜索，首字符不丢失', () => {
    renderPanel();
    fireEvent.keyDown(screen.getByRole('option', { name: actions[0].title }), { key: '格' });
    expect(getSearch()).toHaveValue('格');
    expect(getSearch()).toHaveFocus();
    expect(getSearch()).toHaveAttribute('aria-expanded', 'true');
  });

  it('软件与脚本分组使用网格，分组顺序不受原始数据交错影响', () => {
    renderPanel({ actions: [{ ...actions[0], group: 'actions' }, actions[1], { ...actions[2], group: 'actions' }] });
    const tools = screen.getByRole('group', { name: '软件 / 工具' });
    const scripts = screen.getByRole('group', { name: '脚本 / 动作' });
    expect(within(tools).getByRole('option', { name: actions[1].title })).toBeInTheDocument();
    expect(within(scripts).getAllByRole('option')).toHaveLength(2);
    expect(tools.querySelector('.launcher-grid')).toBeTruthy();
    expect(scripts.querySelector('.launcher-grid')).toBeTruthy();
    expect(screen.getAllByRole('option').map((option) => option.getAttribute('aria-label'))).toEqual([actions[1].title, actions[0].title, actions[2].title]);
  });

  it('网格上下键跨行与分组，左右键选择相邻卡片，Enter 传递上下文', async () => {
    const gridActions: LauncherAction[] = [...Array.from({ length: 6 }, (_, i) => ({ id: `tool-${i}`, title: `工具 ${i}`, group: 'tools' as const })), { id: 'script', title: '脚本', group: 'actions' }];
    const onExecute = vi.fn().mockResolvedValue(undefined);
    renderPanel({ actions: gridActions, onExecute, context: { selection: 'hello' } });
    const input = getSearch();
    fireEvent.keyDown(input, { key: 'ArrowRight' });
    expect(screen.getByRole('option', { name: '工具 1' })).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: '工具 5' })).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: '脚本' })).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(screen.getByRole('option', { name: '工具 4' })).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(input, { key: 'ArrowLeft' });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(onExecute).toHaveBeenCalledWith(gridActions[3], { selection: 'hello' }));
  });

  it('搜索文本中的左右键移动光标，不改变卡片选择', () => {
    renderPanel();
    fireEvent.change(getSearch(), { target: { value: '文' } });
    const selected = screen.getAllByRole('option').find((option) => option.getAttribute('aria-selected') === 'true');
    fireEvent.keyDown(getSearch(), { key: 'ArrowRight' });
    expect(selected).toHaveAttribute('aria-selected', 'true');
  });

  it('最近与收藏以图标筛选，并支持鼠标执行', async () => {
    const onExecute = vi.fn().mockResolvedValue(undefined);
    renderPanel({ onExecute });
    fireEvent.click(screen.getByRole('tab', { name: '最近使用' }));
    expect(screen.getAllByRole('option')).toHaveLength(1);
    expect(screen.getByRole('option', { name: actions[0].title })).toBeInTheDocument();
    fireEvent.keyDown(getSearch(), { key: '3', altKey: true });
    expect(screen.getByRole('tab', { name: '收藏' })).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(screen.getByRole('option', { name: actions[1].title }));
    await waitFor(() => expect(onExecute).toHaveBeenCalledWith(actions[1], undefined));
  });

  it('每次原生窗口唤起重置过滤与搜索并重新聚焦', () => {
    const props = { actions, onExecute: vi.fn(), onClose: vi.fn() };
    const { rerender } = render(<LauncherPanel {...props} activationToken={0} />);
    fireEvent.click(screen.getByRole('tab', { name: '收藏' }));
    fireEvent.change(getSearch(), { target: { value: '格式化' } });
    screen.getByRole('button', { name: '搜索动作' }).focus();
    rerender(<LauncherPanel {...props} activationToken={1} />);
    expect(getSearch()).toHaveFocus();
    expect(getSearch()).toHaveValue('');
    expect(getSearch()).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByRole('tab', { name: '所有动作' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getAllByRole('option')).toHaveLength(3);
  });

  it('原生窗口重新获得焦点时只恢复搜索焦点，不重置查询', () => {
    const props = { actions, onExecute: vi.fn(), onClose: vi.fn() };
    const { rerender } = render(<LauncherPanel {...props} focusToken={0} />);
    fireEvent.change(getSearch(), { target: { value: '格式化' } });
    screen.getByRole('button', { name: '搜索动作' }).focus();
    rerender(<LauncherPanel {...props} focusToken={1} />);
    expect(getSearch()).toHaveFocus();
    expect(getSearch()).toHaveValue('格式化');
    expect(getSearch()).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('option', { name: actions[1].title })).toBeInTheDocument();
  });

  it('空态、执行错误和 Escape 关闭仍然可用', async () => {
    const onClose = vi.fn();
    const onExecute = vi.fn().mockRejectedValue(new Error('脚本不可用'));
    renderPanel({ onClose, onExecute });
    fireEvent.change(getSearch(), { target: { value: '不存在' } });
    expect(screen.getByRole('status')).toHaveTextContent('没有找到匹配动作');
    fireEvent.change(getSearch(), { target: { value: '格式化' } });
    fireEvent.click(screen.getByRole('option', { name: actions[1].title }));
    expect(await screen.findByRole('alert')).toHaveTextContent('脚本不可用');
    fireEvent.keyDown(getSearch(), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('结构化文本结果替换动作网格，并把上下文传给执行适配层', async () => {
    const context = { clipboardText: 'A good day.', selectedText: 'A selection', files: ['D:/notes.txt'] };
    const onExecute = vi.fn().mockResolvedValue(textResult);
    renderPanel({ onExecute, context });
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    const result = await screen.findByRole('region', { name: '翻译结果' });
    expect(onExecute).toHaveBeenCalledWith(actions[0], context);
    expect(result.textContent).toBe(textOutput.text);
    // Focus is applied in an effect/rAF after the result renders; assert it
    // asynchronously so the test does not depend on scheduler timing.
    await waitFor(() => expect(result).toHaveFocus());
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '翻译结果' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '返回动作' }));
    await waitFor(() => expect(getSearch()).toHaveFocus());
    expect(screen.getAllByRole('option')).toHaveLength(3);
  });

  it('复制可重复，保存成功后禁用，操作回调收到同一个文本结果', async () => {
    const onCopyResult = vi.fn().mockResolvedValue(undefined);
    const onSaveResult = vi.fn().mockResolvedValue(undefined);
    renderPanel({ onExecute: vi.fn().mockResolvedValue(textResult), onCopyResult, onSaveResult });
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    await screen.findByRole('region');
    fireEvent.click(screen.getByRole('button', { name: '复制结果' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('已复制'));
    fireEvent.click(screen.getByRole('button', { name: '复制结果' }));
    await waitFor(() => expect(onCopyResult).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByRole('button', { name: '复制结果' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('已保存到收集箱'));
    const saved = screen.getByRole('button', { name: '保存到收集箱' });
    expect(saved).toBeDisabled();
    fireEvent.click(saved);
    expect(onCopyResult).toHaveBeenNthCalledWith(1, textOutput);
    expect(onSaveResult).toHaveBeenCalledExactlyOnceWith(textOutput);
  });

  it('结果操作执行时禁重复，失败保留正文并允许重试', async () => {
    const pending = deferred<void>();
    const onSaveResult = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(undefined);
    const onCopyResult = vi.fn().mockResolvedValue(undefined);
    renderPanel({ onExecute: vi.fn().mockResolvedValue(textResult), onCopyResult, onSaveResult });
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    await screen.findByRole('region');
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    fireEvent.click(screen.getByRole('button', { name: '复制结果' }));
    expect(onSaveResult).toHaveBeenCalledTimes(1);
    expect(onCopyResult).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '复制结果' })).toBeDisabled();
    await act(async () => { pending.reject(new Error('Inbox 暂不可用')); });
    expect(screen.getByRole('alert')).toHaveTextContent('Inbox 暂不可用');
    expect(screen.getByRole('region').textContent).toBe(textOutput.text);
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '保存到收集箱' })).toBeDisabled());
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it.each([
    [{ ok: false, status: 'error', error: 'Relay 尚未启动', message: '忽略这段信息' }, 'Relay 尚未启动'],
    [{ ok: false, status: 'unavailable', message: '没有可用模型' }, '没有可用模型'],
  ] as const)('执行器返回失败时展示原因，而不误报执行成功', async (result, message) => {
    renderPanel({ onExecute: vi.fn().mockResolvedValue(result) });
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    expect(await screen.findByRole('alert')).toHaveTextContent(message);
    expect(screen.queryByText(/已执行/)).not.toBeInTheDocument();
  });

  it('执行器返回 cancelled 时给出简洁反馈', async () => {
    renderPanel({ onExecute: vi.fn().mockResolvedValue({ ok: false, status: 'cancelled', error: 'AbortError' }) });
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('已取消'));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('取消后忽略迟到的旧结果，旧 finally 不会中断新执行', async () => {
    const translateAction: LauncherAction = { id: 'translate-text', title: '翻译剪贴板', cancellable: true };
    const oldExecution = deferred<ActionHandlerResult>();
    const newExecution = deferred<ActionHandlerResult>();
    const onExecute = vi.fn().mockReturnValueOnce(oldExecution.promise).mockReturnValueOnce(newExecution.promise);
    const onCancelExecution = vi.fn();
    renderPanel({ actions: [translateAction, actions[1]], onExecute, onCancelExecution });
    fireEvent.click(screen.getByRole('option', { name: translateAction.title }));
    fireEvent.click(screen.getByRole('button', { name: '取消执行' }));
    expect(onCancelExecution).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('status')).toHaveTextContent('已取消');
    fireEvent.click(screen.getByRole('option', { name: actions[1].title }));
    await act(async () => { oldExecution.resolve(textResult); });
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('执行中');
    await act(async () => { newExecution.resolve({ ...textResult, output: { ...textOutput, text: '新会话的结果' } }); });
    expect(screen.getByRole('region')).toHaveTextContent('新会话的结果');
  });

  it.each([undefined, false])('不可取消动作执行中不展示取消入口（cancellable=%s）', async (cancellable) => {
    const pending = deferred<void>();
    const onCancelExecution = vi.fn();
    const action = { ...actions[0], cancellable };
    renderPanel({ actions: [action], onExecute: vi.fn().mockReturnValue(pending.promise), onCancelExecution });
    fireEvent.click(screen.getByRole('option', { name: action.title }));
    expect(screen.getByRole('status')).toHaveTextContent('执行中');
    expect(screen.queryByRole('button', { name: '取消执行' })).not.toBeInTheDocument();
    expect(onCancelExecution).not.toHaveBeenCalled();
    await act(async () => { pending.resolve(undefined); });
    expect(screen.getByRole('status')).toHaveTextContent(`已执行：${action.title}`);
  });

  it('结果页重新获得焦点保留正文选区，快捷键不会搜索或触发后台动作', async () => {
    const onClose = vi.fn();
    const onExecute = vi.fn().mockResolvedValue(textResult);
    const props = { actions, onClose, onExecute };
    const { rerender } = render(<LauncherPanel {...props} focusToken={0} />);
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    const result = await screen.findByRole('region');
    const range = document.createRange();
    range.selectNodeContents(result);
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);
    rerender(<LauncherPanel {...props} focusToken={1} />);
    // Focus is applied after the token change (effect/rAF); assert it async.
    await waitFor(() => expect(result).toHaveFocus());
    expect(window.getSelection()?.toString()).toBe(textOutput.text);
    for (const event of [{ key: 'ArrowDown' }, { key: 'Enter' }, { key: '/' }, { key: 'f', ctrlKey: true }, { key: 'c', ctrlKey: true }]) {
      expect(fireEvent.keyDown(result, event)).toBe(true);
    }
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    expect(onExecute).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(result, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
    window.getSelection()?.removeAllRanges();
  });

  it('选区结果提供显式替换出口并显示失败反馈', async () => {
    const onReplaceResult = vi.fn().mockRejectedValue(new Error('原始窗口已关闭'));
    const onExecute = vi.fn().mockResolvedValue(textResult);
    renderPanel({ onExecute, onReplaceResult });
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    await screen.findByRole('region');
    fireEvent.click(screen.getByRole('button', { name: '替换原文选区' }));
    await waitFor(() => expect(onReplaceResult).toHaveBeenCalledWith(textOutput));
    expect(await screen.findByRole('alert')).toHaveTextContent('原始窗口已关闭');
  });

  it('重新唤起清除结果、操作状态与反馈，忽略上一会话迟到的操作错误', async () => {
    const pendingCopy = deferred<void>();
    const props = { actions, onExecute: vi.fn().mockResolvedValue(textResult), onClose: vi.fn(), onCopyResult: vi.fn().mockReturnValue(pendingCopy.promise) };
    const { rerender } = render(<LauncherPanel {...props} activationToken={0} />);
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    await screen.findByRole('region');
    fireEvent.click(screen.getByRole('button', { name: '复制结果' }));
    rerender(<LauncherPanel {...props} activationToken={1} />);
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    await waitFor(() => expect(getSearch()).toHaveFocus());
    expect(screen.getAllByRole('option')).toHaveLength(3);
    await act(async () => { pendingCopy.reject(new Error('上一次复制失败')); });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('重新唤起忽略上一会话尚未结束的执行', async () => {
    const pending = deferred<ActionHandlerResult>();
    const props = { actions, onExecute: vi.fn().mockReturnValue(pending.promise), onClose: vi.fn() };
    const { rerender } = render(<LauncherPanel {...props} activationToken={0} />);
    fireEvent.click(screen.getByRole('option', { name: actions[0].title }));
    rerender(<LauncherPanel {...props} activationToken={1} />);
    await act(async () => { pending.resolve(textResult); });
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    await waitFor(() => expect(getSearch()).toHaveFocus());
  });

  it('加载错误先以警告图标提示，点击可展开详情，当前上下文也以图标显示', () => {
    renderPanel({ error: '动作列表加载失败', context: { selection: '选中文字' }, contextLabel: '选中文本' });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    const warning = screen.getByRole('button', { name: '查看动作加载错误' });
    expect(warning).toHaveAttribute('title', '动作列表加载失败');
    fireEvent.click(warning);
    expect(screen.getByRole('alert')).toHaveTextContent('动作列表加载失败');
    fireEvent.click(warning);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByLabelText('选中文本')).toHaveAttribute('title', '选中文本');
    expect(screen.queryByText('选中文本')).not.toBeInTheDocument();
  });

  it('原生交互错误直接显示为即时警告', () => {
    renderPanel({ interactionError: '无法拖动快捷窗口' });
    expect(screen.getByRole('alert')).toHaveTextContent('无法拖动快捷窗口');
  });

  it('收藏按钮的鼠标、Enter 与 Space 不冒泡执行卡片', () => {
    const onExecute = vi.fn();
    const onToggleFavorite = vi.fn();
    renderPanel({ onExecute, onToggleFavorite });
    const favorite = screen.getByRole('button', { name: `收藏 ${actions[0].title}` });
    for (const key of ['Enter', ' ']) {
      fireEvent.keyDown(favorite, { key });
      fireEvent.click(favorite);
    }
    expect(onToggleFavorite).toHaveBeenCalledTimes(2);
    expect(onExecute).not.toHaveBeenCalled();
  });

  it.each(['Enter', ' '])('卡片使用 $0 只执行一次，不被根键盘事件重复触发', async (key) => {
    const onExecute = vi.fn().mockResolvedValue(undefined);
    renderPanel({ onExecute });
    fireEvent.keyDown(screen.getByRole('option', { name: actions[1].title }), { key });
    await waitFor(() => expect(onExecute).toHaveBeenCalledTimes(1));
    expect(onExecute).toHaveBeenCalledWith(actions[1], undefined);
  });

  it('顶部留白启动单次原生拖动，不关闭且阻止 Tauri 文档监听器重复请求', () => {
    const onStartDragging = vi.fn();
    const onClose = vi.fn();
    const bubbled = vi.fn();
    renderPanel({ onStartDragging, onClose });
    document.addEventListener('mousedown', bubbled);
    const handle = document.querySelector('.launcher-drag-handle')!;
    fireEvent.pointerDown(handle, { button: 0 });
    fireEvent.mouseDown(handle, { button: 0 });
    expect(onStartDragging).toHaveBeenCalledTimes(1);
    expect(bubbled).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    document.removeEventListener('mousedown', bubbled);
  });

  it('原生 mousedown 单独派发也能启动拖动', () => {
    const onStartDragging = vi.fn();
    renderPanel({ onStartDragging });
    fireEvent.mouseDown(document.querySelector('.launcher-header')!, { button: 0 });
    expect(onStartDragging).toHaveBeenCalledTimes(1);
  });

  it('点击透明窗口空白区域不会误关，关闭由按钮、Esc 或重复触发完成', () => {
    const onClose = vi.fn();
    renderPanel({ onClose });
    fireEvent.pointerDown(screen.getByRole('dialog'), { button: 0 });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(getSearch(), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('默认不置顶，置顶、搜索、筛选、关闭按钮及其 SVG 都不启动拖动', () => {
    const onClose = vi.fn();
    const onToggleAlwaysOnTop = vi.fn();
    const onStartDragging = vi.fn();
    renderPanel({ onClose, onToggleAlwaysOnTop, onStartDragging });
    const pin = screen.getByRole('button', { name: '置顶快速动作面板' });
    expect(pin).toHaveAttribute('aria-pressed', 'false');
    for (const button of [pin, screen.getByRole('button', { name: '搜索动作' }), screen.getByRole('tab', { name: '收藏' }), screen.getByRole('button', { name: '关闭快速动作面板' })]) {
      fireEvent.mouseDown(button.querySelector('path, rect')!, { button: 0 });
      fireEvent.click(button);
    }
    expect(onToggleAlwaysOnTop).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onStartDragging).not.toHaveBeenCalled();
  });

  it('右键或中键按下标题栏不会拖动或关闭面板', () => {
    const onClose = vi.fn();
    const onStartDragging = vi.fn();
    renderPanel({ onClose, onStartDragging });
    for (const button of [1, 2]) {
      fireEvent.mouseDown(document.querySelector('.launcher-header')!, { button });
      fireEvent.pointerDown(screen.getByRole('dialog'), { button });
    }
    expect(onStartDragging).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
