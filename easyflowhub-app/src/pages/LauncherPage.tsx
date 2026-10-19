import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { LauncherPanel, type LauncherAction, type LauncherContext } from '../features/launcher/LauncherPanel';
import { actionRegistry, scriptSummaryToAction, CAPTURE_CLIPBOARD_ACTION, SNOW_SHOT_ACTION, TRANSLATE_TEXT_ACTION, type Action, type ActionExecutor, type ActionTextOutput } from '../features/actions';
import { translateText } from '../features/translation';
import { getSettings } from '../lib/tauri/settings';
import { captureInboxItem } from '../lib/tauri/inbox';
import { copyTextWithFallback, readTextWithFallback } from '../lib/tauri/clipboard';
import { describeScript, listScripts, runScript } from '../lib/tauri/scriptmgr';
import {
  getConfiguredSnowShotPath,
  launchSnowShot,
  openSnowShotInstallPage,
  hideLauncherWindow,
  getLauncherAlwaysOnTop,
  onLauncherFocused,
  onLauncherHidden,
  onLauncherOpened,
  setLauncherAlwaysOnTop,
  startLauncherDragging,
  replaceSelectedText,
} from '../lib/tauri/launcher';

const MAX_CLIPBOARD_IMAGE_BYTES = 32 * 1024 * 1024;

/**
 * Read a supported image from the system clipboard at the moment the user
 * invokes the capture action. Clipboard images are intentionally read lazily:
 * this keeps launcher activation lightweight and preserves the user gesture
 * required by Chromium/Tauri for clipboard permissions.
 */
async function readClipboardImageDataUrl(): Promise<string | undefined> {
  const clipboard = navigator.clipboard as Clipboard & {
    read?: () => Promise<ClipboardItems>;
  };
  if (typeof clipboard?.read !== 'function') return undefined;
  const items = await clipboard.read();
  for (const item of items) {
    const mimeType = item.types.find((type) => ['image/png', 'image/jpeg', 'image/webp', 'image/gif'].includes(type));
    if (!mimeType) continue;
    const blob = await item.getType(mimeType);
    if (blob.size === 0) throw new Error('剪贴板图片为空。');
    if (blob.size > MAX_CLIPBOARD_IMAGE_BYTES) throw new Error('剪贴板图片超过 32 MB 限制。');
    const bytes = new Uint8Array(await blob.arrayBuffer());
    let binary = '';
    const chunkSize = 0x8000;
    for (let offset = 0; offset < bytes.length; offset += chunkSize) {
      binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length)));
    }
    return `data:${mimeType};base64,${btoa(binary)}`;
  }
  return undefined;
}

function toLauncherAction(action: Action): LauncherAction {
  return {
    id: action.id,
    title: action.name,
    description: action.description,
    category: action.category,
    icon: action.icon ?? (action.kind === 'script' ? '⚙' : '↗'),
    keywords: action.keywords,
    favorite: actionRegistry.isFavorite(action.id),
    recent: actionRegistry.getRecent().some((recent) => recent.id === action.id),
    disabled: action.enabled === false,
    cancellable: action.metadata?.cancellable === true,
    group: action.kind === 'script' || action.source === 'script' ? 'actions' : 'tools',
  };
}

/** Windows-first global action surface backed by the shared ActionRegistry. */
export function LauncherPage() {
  const [actions, setActions] = useState<Action[]>([]);
  const [error, setError] = useState<string>();
  const [context, setContext] = useState<LauncherContext>();
  // The launcher is a transient action surface. It only becomes a pinned
  // desktop surface after the user explicitly enables the pin control.
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [interactionError, setInteractionError] = useState<string>();
  const [activationToken, setActivationToken] = useState(0);
  const [focusToken, setFocusToken] = useState(0);
  const executionRef = useRef<AbortController | undefined>(undefined);
  const clipboardReadRef = useRef(0);

  const cancelExecution = useCallback(() => {
    executionRef.current?.abort();
    executionRef.current = undefined;
  }, []);

  useEffect(() => () => {
    cancelExecution();
    clipboardReadRef.current += 1;
  }, [cancelExecution]);

  const refreshClipboardContext = useCallback(async (selectedText?: string) => {
    const readId = ++clipboardReadRef.current;
    setContext(undefined);
    try {
      const clipboard = await readTextWithFallback();
      if (readId === clipboardReadRef.current) {
        const selection = selectedText?.trim();
        setContext(selection || clipboard ? {
          ...(selection ? { selectedText: selection.slice(0, 1_000_000), selection: selection.slice(0, 1_000_000) } : {}),
          ...(clipboard ? { clipboardText: clipboard.slice(0, 1_000_000) } : {}),
        } : undefined);
      }
    } catch {
      if (readId === clipboardReadRef.current) setContext(undefined);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onLauncherHidden(() => {
      cancelExecution();
      clipboardReadRef.current += 1;
      setContext(undefined);
      setActivationToken((token) => token + 1);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch(() => { /* Browser previews have no native lifecycle events. */ });
    return () => { disposed = true; unlisten?.(); };
  }, [cancelExecution]);

  useEffect(() => {
    const root = document.documentElement;
    const previousWindow = root.dataset.window;
    root.dataset.window = 'launcher';
    return () => {
      if (root.dataset.window !== 'launcher') return;
      if (previousWindow === undefined) delete root.dataset.window;
      else root.dataset.window = previousWindow;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onLauncherOpened((event) => {
      const payload = event && typeof event === 'object' && 'payload' in event
        ? (event as { payload?: { selectedText?: string | null } }).payload
        : event;
      cancelExecution();
      setActivationToken((token) => token + 1);
      setInteractionError(undefined);
      void refreshClipboardContext(payload?.selectedText ?? undefined);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch(() => {
      // Browser preview has no native activation events.
    });
    return () => { disposed = true; unlisten?.(); };
  }, [cancelExecution, refreshClipboardContext]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onLauncherFocused(() => setFocusToken((token) => token + 1)).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch(() => {
      // Browser preview has no native window focus events.
    });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  useEffect(() => {
    let disposed = false;
    void getLauncherAlwaysOnTop().then((pinned) => {
      if (!disposed) setAlwaysOnTop(pinned);
    }).catch(() => {
      // The browser preview has no native window and starts unpinned.
    });
    return () => { disposed = true; };
  }, []);

  const executeAction = useCallback<ActionExecutor>(async (invocationContext, action) => {
    const cancelled = () => ({ ok: false, status: 'cancelled' as const, message: '已取消' });
    if (invocationContext.signal?.aborted) return cancelled();
    if (action.id === SNOW_SHOT_ACTION.id) {
      const result = await launchSnowShot(getConfiguredSnowShotPath());
      if (result.status === 'launched') return { ok: true, status: 'success', message: '已打开截屏工具' };
      if (result.status === 'not_installed') {
        await openSnowShotInstallPage();
        throw new Error('未检测到 Snow Shot，已打开官方安装页。安装后可再次执行此动作。');
      }
      throw new Error(result.message || 'Snow Shot 启动失败，请检查安装路径或权限。');
    }
    if (action.id === CAPTURE_CLIPBOARD_ACTION.id) {
      let imageDataUrl: string | undefined;
      try {
        imageDataUrl = await readClipboardImageDataUrl();
      } catch (caught) {
        // Permission errors are allowed to fall back to the text snapshot that
        // was captured when the Launcher opened. Size/empty-image validation
        // errors are actionable and should remain visible to the user.
        const message = caught instanceof Error ? caught.message : String(caught);
        if (message === '剪贴板图片为空。' || message === '剪贴板图片超过 32 MB 限制。') throw new Error(message);
      }
      if (imageDataUrl) {
        await captureInboxItem({ imageDataUrl, source: 'launcher', context: 'clipboard-image' });
        return { ok: true, status: 'success', message: '图片已保存到收集箱' };
      }
      const clipboardText = invocationContext.clipboardText?.trim();
      if (!clipboardText) throw new Error('剪贴板中没有可收集的文字或图片。');
      await captureInboxItem({ content: clipboardText, source: 'launcher', context: 'clipboard' });
      return { ok: true, status: 'success', message: '已保存到收集箱' };
    }
    if (action.id === TRANSLATE_TEXT_ACTION.id) {
      const text = invocationContext.selectedText?.trim() || invocationContext.clipboardText?.trim();
      if (!text) throw new Error('剪贴板中没有可翻译的文字，请先复制文本。');
      let settings;
      try {
        settings = await getSettings();
      } catch {
        throw new Error('无法读取翻译设置，请在桌面应用的设置中配置目标语言和模型。');
      }
      if (invocationContext.signal?.aborted) return cancelled();
      const config = settings.translation;
      if (!config?.model?.trim()) throw new Error('请先在设置 → 翻译中填写 Relay 可用的模型名称。');
      const result = await translateText({
        text, targetLanguage: config.target_language, model: config.model.trim(), signal: invocationContext.signal,
      });
      if (invocationContext.signal?.aborted) return cancelled();
      const output: ActionTextOutput = {
        kind: 'text', text: result.text, title: '翻译结果', sourceText: text, targetLanguage: result.targetLanguage,
      };
      return { ok: true, status: 'success', output };
    }
    if (action.kind !== 'script') throw new Error(`暂不支持执行 ${action.kind} 动作`);
    const scriptId = String(action.metadata?.scriptId ?? action.id.replace(/^script:/, ''));
    const detail = await describeScript(scriptId);
    if (invocationContext.signal?.aborted) return cancelled();
    const args = [];
    for (const parameter of detail.parameters ?? []) {
      const value = window.prompt(`${parameter.label || parameter.name}${parameter.required ? '（必填）' : ''}`, parameter.default ?? '');
      if (value === null) return cancelled();
      if (value !== '') args.push(value);
    }
    const result = await runScript(scriptId, { args });
    if (!result.succeeded) throw new Error(result.output || `脚本执行失败（${result.status}）`);
    return {
      ok: true, status: 'success',
      output: result.output?.trim() ? { kind: 'text', text: result.output, title: action.name } satisfies ActionTextOutput : undefined,
      message: '已执行',
    };
  }, []);

  const reload = useCallback(async () => {
    setError(undefined);
    try {
      // Built-in actions are registered alongside ScriptMgr actions so they
      // participate in search, favorites, and the recent action index.
      actionRegistry.register(SNOW_SHOT_ACTION, executeAction);
      actionRegistry.register(CAPTURE_CLIPBOARD_ACTION, executeAction);
      actionRegistry.register(TRANSLATE_TEXT_ACTION, executeAction);
      const scripts = await listScripts();
      scripts.forEach((script) => {
        const action = scriptSummaryToAction(script);
        actionRegistry.register(action, executeAction);
      });
      setActions(actionRegistry.getAll());
    } catch (caught) {
      const rawMessage = caught instanceof Error ? caught.message : '';
      // The browser preview does not expose Tauri's IPC bridge. Keep the
      // built-in tools usable while explaining why ScriptMgr actions are
      // unavailable, instead of surfacing an implementation-level error.
      const unavailable = /invoke|tauri|undefined/i.test(rawMessage);
      setError(
        unavailable
          ? '脚本服务暂不可用，内置工具仍可使用。请在桌面应用中启动 ScriptMgr。'
          : rawMessage || '无法加载脚本动作',
      );
      setActions(actionRegistry.getAll());
    }
  }, [executeAction]);

  useEffect(() => {
    void reload();
    void refreshClipboardContext();
  }, [refreshClipboardContext, reload]);

  const launcherActions = useMemo(() => actions.map(toLauncherAction), [actions]);

  const execute = useCallback(async (launcherAction: LauncherAction, invocationContext?: LauncherContext) => {
    cancelExecution();
    const controller = new AbortController();
    executionRef.current = controller;
    try {
      const result = await actionRegistry.execute(launcherAction.id, {
        ...invocationContext,
        selectedText: invocationContext?.selectedText ?? invocationContext?.selection,
        signal: controller.signal,
      });
      if (controller.signal.aborted) return { ...result, ok: false, status: 'cancelled' as const, output: undefined, error: undefined, message: '已取消' };
      setActions(actionRegistry.getAll());
      return result;
    } finally {
      if (executionRef.current === controller) executionRef.current = undefined;
    }
  }, [cancelExecution]);

  const copyResult = useCallback(async (output: ActionTextOutput) => {
    try {
      await copyTextWithFallback(output.text);
    } catch {
      throw new Error('复制失败，请重试或手动选择译文复制。');
    }
  }, []);

  const saveResult = useCallback(async (output: ActionTextOutput) => {
    const content = output.sourceText
      ? `原文\n${output.sourceText}\n\n译文${output.targetLanguage ? `（${output.targetLanguage}）` : ''}\n${output.text}`
      : output.text;
    await captureInboxItem({ title: output.title, content, source: 'launcher', context: output.sourceText ? 'translation' : 'action-result' });
  }, []);

  const replaceResult = useCallback(async (output: ActionTextOutput) => {
    if (!output.sourceText) throw new Error('当前结果没有关联的原文选区。');
    await replaceSelectedText(output.text);
  }, []);

  const toggleFavorite = useCallback((launcherAction: LauncherAction) => {
    actionRegistry.toggleFavorite(launcherAction.id);
    setActions(actionRegistry.getAll());
  }, []);

  const toggleAlwaysOnTop = useCallback(async () => {
    const next = !alwaysOnTop;
    try {
      const applied = await setLauncherAlwaysOnTop(next);
      setAlwaysOnTop(typeof applied === 'boolean' ? applied : next);
      setInteractionError(undefined);
    } catch {
      setInteractionError('无法切换置顶状态，请确认使用的是最新桌面应用。');
    }
  }, [alwaysOnTop]);

  const close = useCallback(async () => {
    cancelExecution();
    clipboardReadRef.current += 1;
    setContext(undefined);
    try {
      await hideLauncherWindow();
    } catch {
      setInteractionError('无法关闭快捷窗口，请确认使用的是最新桌面应用。');
    }
  }, [cancelExecution]);

  const startDragging = useCallback(async () => {
    try {
      await startLauncherDragging();
      setInteractionError(undefined);
    } catch {
      setInteractionError('无法拖动快捷窗口，请确认使用的是最新桌面应用。');
    }
  }, []);

  return (
    <LauncherPanel
      activationToken={activationToken}
      focusToken={focusToken}
      actions={launcherActions}
      onExecute={execute}
      onCopyResult={copyResult}
      onSaveResult={saveResult}
      onReplaceResult={replaceResult}
      onCancelExecution={cancelExecution}
      onClose={close}
      context={context}
      contextLabel="剪贴板上下文"
      recentActionIds={actionRegistry.getRecent().map((action) => action.id)}
      favoriteActionIds={actionRegistry.getFavorites().map((action) => action.id)}
      error={error}
      interactionError={interactionError}
      onToggleFavorite={toggleFavorite}
      alwaysOnTop={alwaysOnTop}
      onToggleAlwaysOnTop={toggleAlwaysOnTop}
      onStartDragging={startDragging}
    />
  );
}

