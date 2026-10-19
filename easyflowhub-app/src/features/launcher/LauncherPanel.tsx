import { useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from 'react';
import { isActionTextOutput, type ActionExecutionContext, type ActionHandlerResult, type ActionTextOutput } from '../actions';
import './LauncherPanel.css';

export interface LauncherContext extends ActionExecutionContext {
  selection?: string;
  [key: string]: unknown;
}

export interface LauncherAction {
  id: string;
  title: string;
  description?: string;
  category?: string;
  icon?: string;
  keywords?: string[];
  shortcut?: string;
  favorite?: boolean;
  recent?: boolean;
  disabled?: boolean;
  /** Only executors that can abort their underlying work expose cancellation. */
  cancellable?: boolean;
  /** Tools are external/built-in apps; actions are scripts. */
  group?: 'tools' | 'actions';
}

export interface LauncherPanelProps {
  actions: LauncherAction[];
  onExecute: (action: LauncherAction, context?: LauncherContext) => void | ActionHandlerResult | Promise<void | ActionHandlerResult>;
  onCopyResult?: (output: ActionTextOutput) => Promise<void>;
  onSaveResult?: (output: ActionTextOutput) => Promise<void>;
  onReplaceResult?: (output: ActionTextOutput) => Promise<void>;
  onCancelExecution?: () => void;
  onClose: () => void;
  context?: LauncherContext;
  recentActionIds?: string[];
  favoriteActionIds?: string[];
  /** A loading error remains available without obscuring usable tools. */
  error?: string;
  /** Immediate failures from native window operations remain visible as alerts. */
  interactionError?: string;
  contextLabel?: string;
  className?: string;
  onToggleFavorite?: (action: LauncherAction) => void;
  alwaysOnTop?: boolean;
  onToggleAlwaysOnTop?: () => void | Promise<void>;
  onStartDragging?: () => void | Promise<void>;
  /** Changes when the native window is shown while this component stays mounted. */
  activationToken?: number;
  /** Changes when the native window becomes focused without resetting search state. */
  focusToken?: number;
}

type Filter = 'all' | 'recent' | 'favorites';
type Glyph = 'grid' | 'clock' | 'star' | 'search' | 'pin' | 'close' | 'warning' | 'context' | 'back' | 'copy' | 'inbox' | 'check' | 'stop' | 'replace';

const filterLabels: Record<Filter, string> = { all: '所有动作', recent: '最近使用', favorites: '收藏' };
const filterGlyphs: Record<Filter, Glyph> = { all: 'grid', recent: 'clock', favorites: 'star' };
const filters: Filter[] = ['all', 'recent', 'favorites'];

function LauncherGlyph({ name }: { name: Glyph }) {
  const paths: Record<Glyph, ReactNode> = {
    grid: <><rect x="3" y="3" width="6" height="6" rx="1.3" /><rect x="15" y="3" width="6" height="6" rx="1.3" /><rect x="3" y="15" width="6" height="6" rx="1.3" /><rect x="15" y="15" width="6" height="6" rx="1.3" /></>,
    clock: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>,
    star: <path d="m12 3 2.8 5.7 6.3.9-4.6 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9L12 3Z" />,
    search: <><circle cx="10.5" cy="10.5" r="6.8" /><path d="m15.5 15.5 5 5" /></>,
    pin: <><path d="M8 3h8l-1 6 4 4v2H5v-2l4-4-1-6Z" /><path d="M12 15v7" /></>,
    close: <path d="m6 6 12 12M18 6 6 18" />,
    warning: <><path d="m10.3 4.3-8 13.8A2 2 0 0 0 4 21h16a2 2 0 0 0 1.7-2.9l-8-13.8a2 2 0 0 0-3.4 0Z" /><path d="M12 9v4m0 4h.01" /></>,
    context: <><path d="M4 9V4h5m6 0h5v5M4 15v5h5m6 0h5v-5" /><circle cx="12" cy="12" r="2" /></>,
    back: <path d="m10 5-7 7 7 7M3 12h18" />,
    copy: <><rect x="8" y="8" width="12" height="13" rx="2" /><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3" /></>,
    inbox: <><path d="m3 14 3-10h12l3 10v6H3v-6Z" /><path d="M3 14h5l2 3h4l2-3h5" /></>,
    check: <path d="m5 12 4 4L19 6" />,
    stop: <rect x="6" y="6" width="12" height="12" rx="2" />,
    replace: <><path d="M4 7h11l-3-3" /><path d="m15 7-3 3" /><path d="M20 17H9l3 3" /><path d="m9 17 3-3" /></>,
  };
  return <svg className="launcher-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">{paths[name]}</svg>;
}

export function LauncherPanel({
  actions, onExecute, onClose, context, recentActionIds = [], favoriteActionIds = [], error, interactionError,
  contextLabel = '当前上下文', className = '', onToggleFavorite, alwaysOnTop = false,
  onToggleAlwaysOnTop, onStartDragging, onCopyResult, onSaveResult, onReplaceResult, onCancelExecution, activationToken = 0, focusToken = 0,
}: LauncherPanelProps) {
  const searchId = useId();
  const searchRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);
  const composingRef = useRef(false);
  const executionGeneration = useRef(0);
  const operationGeneration = useRef(0);
  const executingRef = useRef(false);
  const operatingRef = useRef(false);
  const [query, setQuery] = useState('');
  const [searchOpen, setSearchOpen] = useState(false);
  const [filter, setFilter] = useState<Filter>('all');
  const [activeIndex, setActiveIndex] = useState(0);
  const [isExecuting, setIsExecuting] = useState(false);
  const [executionCancellable, setExecutionCancellable] = useState(false);
  const [executionError, setExecutionError] = useState<string>();
  const [executionMessage, setExecutionMessage] = useState<string>();
  const [textOutput, setTextOutput] = useState<ActionTextOutput>();
  const [resultTitle, setResultTitle] = useState('');
  const [resultOperation, setResultOperation] = useState<'copy' | 'save' | 'replace'>();
  const [resultSaved, setResultSaved] = useState(false);
  const [errorDetailsOpen, setErrorDetailsOpen] = useState(false);
  const recentSet = useMemo(() => new Set(recentActionIds), [recentActionIds]);
  const favoriteSet = useMemo(() => new Set(favoriteActionIds), [favoriteActionIds]);

  const sections = useMemo(() => {
    const search = query.trim().toLocaleLowerCase();
    const matching = actions.filter((action) => {
      if (action.disabled) return false;
      if (filter === 'recent' && !action.recent && !recentSet.has(action.id)) return false;
      if (filter === 'favorites' && !action.favorite && !favoriteSet.has(action.id)) return false;
      if (!search) return true;
      return [action.title, action.description, action.category, ...(action.keywords ?? [])]
        .filter(Boolean).join(' ').toLocaleLowerCase().includes(search);
    });
    return (['tools', 'actions'] as const).map((group) => ({
      group, actions: matching.filter((action) => (action.group ?? 'tools') === group),
    })).filter((section) => section.actions.length);
  }, [actions, favoriteSet, filter, query, recentSet]);
  const visibleActions = useMemo(() => sections.flatMap((section) => section.actions), [sections]);
  const focusSearch = useCallback(() => searchRef.current?.focus({ preventScroll: true }), []);
  const openSearch = useCallback(() => { setSearchOpen(true); focusSearch(); }, [focusSearch]);

  useEffect(() => {
    executionGeneration.current += 1;
    operationGeneration.current += 1;
    executingRef.current = false;
    operatingRef.current = false;
    setIsExecuting(false);
    setExecutionCancellable(false);
    setTextOutput(undefined);
    setResultOperation(undefined);
    setResultSaved(false);
    setQuery('');
    setSearchOpen(false);
    setFilter('all');
    setActiveIndex(0);
    setExecutionError(undefined);
    setExecutionMessage(undefined);
    setErrorDetailsOpen(false);
    composingRef.current = false;
    focusSearch();
  }, [activationToken, focusSearch]);

  useEffect(() => {
    if (!textOutput) focusSearch();
  }, [focusToken, focusSearch, textOutput]);

  useEffect(() => {
    if (textOutput) outputRef.current?.focus({ preventScroll: true });
  }, [textOutput]);

  useEffect(() => () => {
    executionGeneration.current += 1;
    operationGeneration.current += 1;
  }, []);

  useEffect(() => {
    setActiveIndex((index) => Math.min(index, Math.max(visibleActions.length - 1, 0)));
  }, [visibleActions.length]);

  const execute = useCallback(async (action: LauncherAction | undefined) => {
    if (!action || executingRef.current) return;
    const generation = ++executionGeneration.current;
    executingRef.current = true;
    setExecutionError(undefined);
    setExecutionMessage(undefined);
    setExecutionCancellable(action.cancellable === true);
    setIsExecuting(true);
    try {
      const result = await onExecute(action, context);
      if (generation !== executionGeneration.current) return;
      if (result?.status === 'cancelled') {
        setExecutionMessage('已取消');
      } else if (result && !result.ok) {
        setExecutionError(result.error || result.message || '动作执行失败，请稍后重试');
      } else if (result && isActionTextOutput(result.output)) {
        setTextOutput(result.output);
        setResultTitle(result.output.title || action.title);
        setResultSaved(false);
      } else {
        setExecutionMessage(result?.message || `已执行：${action.title}`);
      }
    } catch (caught) {
      if (generation !== executionGeneration.current) return;
      if (caught instanceof Error && caught.name === 'AbortError') setExecutionMessage('已取消');
      else setExecutionError(caught instanceof Error && caught.message ? caught.message : '动作执行失败，请稍后重试');
    } finally {
      if (generation === executionGeneration.current) {
        executingRef.current = false;
        setIsExecuting(false);
        setExecutionCancellable(false);
      }
    }
  }, [context, onExecute]);

  const cancelExecution = () => {
    if (!executingRef.current || !executionCancellable || !onCancelExecution) return;
    executionGeneration.current += 1;
    executingRef.current = false;
    setIsExecuting(false);
    setExecutionCancellable(false);
    setExecutionError(undefined);
    setExecutionMessage('已取消');
    onCancelExecution();
  };

  const closePanel = () => {
    executionGeneration.current += 1;
    operationGeneration.current += 1;
    executingRef.current = false;
    operatingRef.current = false;
    setIsExecuting(false);
    setExecutionCancellable(false);
    setResultOperation(undefined);
    onClose();
  };

  const operateOnResult = async (operation: 'copy' | 'save') => {
    const callback = operation === 'copy' ? onCopyResult : onSaveResult;
    if (!textOutput || !callback || operatingRef.current || (operation === 'save' && resultSaved)) return;
    const generation = ++operationGeneration.current;
    operatingRef.current = true;
    setResultOperation(operation);
    setExecutionError(undefined);
    setExecutionMessage(undefined);
    try {
      await callback(textOutput);
      if (generation !== operationGeneration.current) return;
      if (operation === 'save') setResultSaved(true);
      setExecutionMessage(operation === 'copy' ? '已复制' : '已保存到收集箱');
    } catch (caught) {
      if (generation !== operationGeneration.current) return;
      setExecutionError(caught instanceof Error && caught.message ? caught.message : operation === 'copy' ? '复制失败，请重试' : '保存失败，请重试');
    } finally {
      if (generation === operationGeneration.current) {
        operatingRef.current = false;
        setResultOperation(undefined);
      }
    }
  };

  const returnToActions = () => {
    operationGeneration.current += 1;
    operatingRef.current = false;
    setTextOutput(undefined);
    setResultOperation(undefined);
    setExecutionError(undefined);
    setExecutionMessage(undefined);
  };

  const selectFilter = (next: Filter) => { setFilter(next); setActiveIndex(0); focusSearch(); };
  const moveSelection = (key: string) => {
    const count = visibleActions.length;
    if (!count) return;
    if (key === 'ArrowLeft' || key === 'ArrowRight') {
      const direction = key === 'ArrowRight' ? 1 : -1;
      setActiveIndex((index) => (index + direction + count) % count);
      return;
    }
    // Every section has its own grid; vertical movement crosses the divider
    // and clamps to the nearest column when the next row is shorter.
    const grids = resultsRef.current?.querySelectorAll<HTMLElement>('.launcher-grid');
    const rows: number[][] = [];
    let start = 0;
    sections.forEach((section, sectionIndex) => {
      const template = grids?.[sectionIndex] ? getComputedStyle(grids[sectionIndex]).gridTemplateColumns : '';
      const columns = template && !template.includes('repeat(') ? template.split(' ').filter(Boolean).length : 4;
      for (let offset = 0; offset < section.actions.length; offset += columns) {
        rows.push(section.actions.slice(offset, offset + columns).map((_, index) => start + offset + index));
      }
      start += section.actions.length;
    });
    const rowIndex = rows.findIndex((row) => row.includes(activeIndex));
    const columnIndex = rows[rowIndex]?.indexOf(activeIndex) ?? 0;
    const direction = key === 'ArrowDown' ? 1 : -1;
    const nextRow = rows[(rowIndex + direction + rows.length) % rows.length];
    setActiveIndex(nextRow[Math.min(columnIndex, nextRow.length - 1)]);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') { event.preventDefault(); closePanel(); return; }
    // Text results retain native selection, scrolling, typing and copy shortcuts.
    if (textOutput || isExecuting) return;
    if (composingRef.current || event.nativeEvent.isComposing || event.keyCode === 229) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
      event.preventDefault(); openSearch(); return;
    }
    if (event.altKey && ['1', '2', '3'].includes(event.key)) {
      event.preventDefault(); selectFilter(filters[Number(event.key) - 1]); return;
    }
    const target = event.target;
    const isInput = target === searchRef.current;
    if (event.key === '/' && !searchOpen && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault(); openSearch(); return;
    }
    // Header and favorite buttons retain their native keyboard behavior.
    if (target instanceof HTMLElement && target.closest('button') && (event.key === 'Enter' || event.key === ' ')) return;
    if (['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight'].includes(event.key)) {
      if (isInput && query && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) return;
      event.preventDefault(); moveSelection(event.key); return;
    }
    if (event.key === 'Enter' && visibleActions[activeIndex]) {
      event.preventDefault(); void execute(visibleActions[activeIndex]); return;
    }
    if (!isInput && event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey && event.key !== ' ') {
      event.preventDefault(); setQuery((value) => value + event.key); setActiveIndex(0); openSearch();
    }
  };

  const handleHeaderMouseDown = (event: MouseEvent<HTMLElement>) => {
    const target = event.target;
    if (event.button !== 0 || !onStartDragging || !(target instanceof Element)
      || target.closest('button, input, textarea, select, a, [data-no-drag]')) return;
    // Tauri's document drag listener must not start a second native drag loop.
    event.preventDefault();
    event.stopPropagation();
    void onStartDragging();
  };

  const activeId = visibleActions[activeIndex] ? `launcher-action-${visibleActions[activeIndex].id}` : undefined;
  const displayError = interactionError || executionError || (errorDetailsOpen ? error : undefined);
  const hasContext = Boolean(context && Object.values(context).some(Boolean));
  let actionOffset = 0;

  return (
    <div className={`launcher-shell ${className}`.trim()} role="dialog" aria-modal="false" aria-label="快速动作面板"
      onKeyDown={handleKeyDown}>
      <div className="launcher-panel">
        <header className="launcher-header" data-tauri-drag-region onMouseDown={handleHeaderMouseDown} title="按住此处拖动窗口">
          {textOutput ? <div className="launcher-output-heading" data-no-drag>
            <button type="button" className="launcher-icon-button" aria-label="返回动作" title="返回动作" onClick={returnToActions}><LauncherGlyph name="back" /></button>
            <h2 title={resultTitle}>{resultTitle}</h2>
          </div> : <div className="launcher-filters" role="tablist" aria-label="动作筛选" data-no-drag>
            {filters.map((option, index) => <button key={option}
              className={`launcher-icon-button launcher-filter ${filter === option ? 'is-active' : ''}`}
              type="button" role="tab" aria-label={filterLabels[option]} aria-selected={filter === option}
              aria-keyshortcuts={`Alt+${index + 1}`} title={`${filterLabels[option]}（Alt+${index + 1}）`}
              onClick={() => selectFilter(option)}><LauncherGlyph name={filterGlyphs[option]} /></button>)}
          </div>}
          <div className="launcher-drag-handle" data-tauri-drag-region aria-hidden="true"><span /><span /></div>
          <div className="launcher-header-actions" data-no-drag>
            {!textOutput && hasContext ? <span className="launcher-context" aria-label={contextLabel} title={contextLabel}><LauncherGlyph name="context" /></span> : null}
            {error ? <button type="button" className="launcher-icon-button launcher-warning" aria-label="查看动作加载错误" aria-expanded={errorDetailsOpen} title={error}
              onClick={() => { setErrorDetailsOpen((value) => !value); if (!textOutput) focusSearch(); }}><LauncherGlyph name="warning" /></button> : null}
            {textOutput ? <>
              {onCopyResult ? <button type="button" className="launcher-icon-button" aria-label="复制结果" title="复制结果" disabled={Boolean(resultOperation)}
                onClick={() => void operateOnResult('copy')}><LauncherGlyph name="copy" /></button> : null}
              {onReplaceResult && textOutput.sourceText ? <button type="button" className="launcher-icon-button" aria-label="替换原文选区" title="替换原文选区"
                disabled={Boolean(resultOperation)} onClick={() => void (async () => {
                  const generation = ++operationGeneration.current;
                  operatingRef.current = true;
                  setResultOperation('replace');
                  setExecutionError(undefined);
                  setExecutionMessage(undefined);
                  try {
                    await onReplaceResult(textOutput);
                    if (generation === operationGeneration.current) setExecutionMessage('已替换原文选区');
                  } catch (caught) {
                    if (generation === operationGeneration.current) setExecutionError(caught instanceof Error && caught.message ? caught.message : '替换失败，请重试');
                  } finally {
                    if (generation === operationGeneration.current) { operatingRef.current = false; setResultOperation(undefined); }
                  }
                })()}><LauncherGlyph name="replace" /></button> : null}
              {onSaveResult ? <button type="button" className={`launcher-icon-button ${resultSaved ? 'is-active' : ''}`} aria-label="保存到收集箱"
                title={resultSaved ? '已保存到收集箱' : '保存到收集箱'} disabled={Boolean(resultOperation) || resultSaved}
                onClick={() => void operateOnResult('save')}><LauncherGlyph name={resultSaved ? 'check' : 'inbox'} /></button> : null}
            </> : <button type="button" className={`launcher-icon-button ${searchOpen ? 'is-active' : ''}`} aria-label="搜索动作" aria-expanded={searchOpen}
              aria-controls={`${searchId}-search`} aria-keyshortcuts="Control+f /" title="搜索（直接输入 / Ctrl+F）" onClick={openSearch}><LauncherGlyph name="search" /></button>}
            {onToggleAlwaysOnTop ? <button className={`launcher-icon-button launcher-pin ${alwaysOnTop ? 'is-active' : ''}`} type="button"
              onClick={() => { void onToggleAlwaysOnTop(); if (!textOutput) focusSearch(); }} aria-pressed={alwaysOnTop}
              aria-label={alwaysOnTop ? '取消置顶快速动作面板' : '置顶快速动作面板'} title={alwaysOnTop ? '取消置顶' : '置顶'}><LauncherGlyph name="pin" /></button> : null}
            <button className="launcher-icon-button launcher-close" type="button" onClick={closePanel} aria-label="关闭快速动作面板" aria-keyshortcuts="Escape" title="关闭（Esc）"><LauncherGlyph name="close" /></button>
          </div>
        </header>

        {!textOutput ? <div id={`${searchId}-search`} className={`launcher-search-wrap ${searchOpen ? 'is-open' : 'is-collapsed'}`}>
          <LauncherGlyph name="search" />
          <input ref={searchRef} id={searchId} className="launcher-search" type="search" value={query}
            onCompositionStart={() => { composingRef.current = true; setSearchOpen(true); }}
            onCompositionEnd={() => { composingRef.current = false; }}
            onChange={(event) => { setQuery(event.target.value); setSearchOpen(true); setActiveIndex(0); setExecutionError(undefined); }}
            placeholder="搜索动作、脚本或工具…" aria-label="搜索动作、脚本或工具" autoComplete="off" spellCheck={false}
            role="combobox" aria-controls={`${searchId}-list`} aria-expanded={searchOpen} aria-activedescendant={activeId} />
          <button className="launcher-icon-button launcher-search-clear" type="button" tabIndex={searchOpen ? 0 : -1} aria-hidden={!searchOpen} aria-label="收起搜索" title="清空并收起搜索"
            onClick={() => { setQuery(''); setSearchOpen(false); setActiveIndex(0); focusSearch(); }}><LauncherGlyph name="close" /></button>
        </div> : null}

        {displayError ? <div className="launcher-feedback launcher-feedback--error" role="alert"><LauncherGlyph name="warning" />{displayError}</div> : null}
        {executionMessage && !displayError ? <div className="launcher-feedback launcher-feedback--success" role="status">✓ {executionMessage}</div> : null}

        {textOutput ? <div className="launcher-output" ref={outputRef} tabIndex={0} role="region" aria-label={resultTitle}>
          {textOutput.text}
        </div> : <div className="launcher-results" ref={resultsRef} id={`${searchId}-list`} role="listbox" aria-label="可执行动作" aria-busy={isExecuting}>
          {visibleActions.length ? sections.map((section) => {
            const start = actionOffset;
            actionOffset += section.actions.length;
            const label = section.group === 'actions' ? '脚本 / 动作' : '软件 / 工具';
            return <div key={section.group} className="launcher-action-group" role="group" aria-label={label}>
              <div className="launcher-group-heading" aria-hidden="true"><span>{label}</span><span className="launcher-group-rule" /></div>
              <div className="launcher-grid">
                {section.actions.map((action, localIndex) => {
                  const index = start + localIndex;
                  const selected = index === activeIndex;
                  const isFavorite = action.favorite || favoriteSet.has(action.id);
                  return <div key={action.id} id={`launcher-action-${action.id}`} className={`launcher-action ${selected ? 'is-selected' : ''}`}
                    role="option" tabIndex={selected ? 0 : -1} aria-selected={selected} aria-label={action.title}
                    title={[action.title, action.description, action.shortcut].filter(Boolean).join('\n')}
                    onFocus={() => setActiveIndex(index)} onMouseEnter={() => setActiveIndex(index)} onClick={() => void execute(action)}
                    onKeyDown={(event) => {
                      if (event.target !== event.currentTarget) return;
                      if ((event.key === 'Enter' || event.key === ' ') && !event.nativeEvent.isComposing) {
                        event.preventDefault(); event.stopPropagation(); void execute(action);
                      }
                    }}>
                    <span className="launcher-action-icon" aria-hidden="true">{action.icon || '↗'}</span>
                    <span className="launcher-action-title">{action.title}</span>
                    {action.shortcut ? <kbd className="launcher-shortcut">{action.shortcut}</kbd> : null}
                    {onToggleFavorite ? <button type="button" className={`launcher-star-button ${isFavorite ? 'is-favorite' : ''}`}
                      aria-label={isFavorite ? `取消收藏 ${action.title}` : `收藏 ${action.title}`}
                      onKeyDown={(event) => { if (event.key === 'Enter' || event.key === ' ') event.stopPropagation(); }}
                      onClick={(event) => { event.stopPropagation(); onToggleFavorite(action); }}><LauncherGlyph name="star" /></button>
                      : isFavorite ? <span className="launcher-star-mark" aria-label="已收藏"><LauncherGlyph name="star" /></span> : null}
                  </div>;
                })}
              </div>
            </div>;
          }) : <div className="launcher-empty" role="status"><LauncherGlyph name="search" /><strong>{query ? '没有找到匹配动作' : filter === 'all' ? '还没有可用动作' : `暂无${filterLabels[filter]}动作`}</strong></div>}
        </div>}

        <footer className="launcher-footer">
          {textOutput ? <span title="选中文本后复制" aria-label="选中文本后复制"><kbd>Ctrl+C</kbd></span>
            : <span title="方向键选择 · Enter 执行" aria-label="方向键选择，Enter 执行"><kbd>↑ ↓ ← →</kbd><kbd>↵</kbd></span>}
          {isExecuting ? <span className="launcher-execution-controls"><span className="launcher-status" role="status"><span className="launcher-spinner" aria-hidden="true" />执行中</span>
            {executionCancellable && onCancelExecution ? <button type="button" className="launcher-icon-button launcher-cancel" aria-label="取消执行" title="取消执行" onClick={cancelExecution}><LauncherGlyph name="stop" /></button> : null}</span>
            : resultOperation ? <span className="launcher-status" role="status"><span className="launcher-spinner" aria-hidden="true" />{resultOperation === 'copy' ? '复制中' : resultOperation === 'save' ? '保存中' : '替换中'}</span>
            : textOutput ? <span title="Esc 关闭" aria-label="Esc 关闭"><kbd>Esc</kbd><LauncherGlyph name="close" /></span>
            : <span className="launcher-footer-shortcuts"><span title="直接输入即可搜索" aria-label="直接输入或按斜杠搜索"><kbd>/</kbd><LauncherGlyph name="search" /></span><span title="Esc 关闭" aria-label="Esc 关闭"><kbd>Esc</kbd><LauncherGlyph name="close" /></span></span>}
        </footer>
      </div>
    </div>
  );
}
