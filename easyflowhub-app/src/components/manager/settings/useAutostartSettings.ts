/**
 * 开机自启动域：系统状态 + 持久化意图。
 *
 * 不直接调用 `@tauri-apps/plugin-autostart`：插件只认系统启动项，读不到
 * 「用户上次想开还是想关」。启动项被外部删掉时插件返回 false，UI 就会静默
 * 显示成关闭（plan §2.4 的实测现象）。原生 `autostart` 模块会保存意图并在
 * 启动时自愈，这里只负责展示真实状态并把写入失败暴露出来。
 */
import { useCallback, useEffect, useState } from 'react';
import {
  getAutostartState,
  setAutostartState,
  type AutostartState,
} from '../../../lib/tauri/autostart';

function describeError(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return '无法修改开机自启动设置';
}

export function useAutostartSettings() {
  const [state, setState] = useState<AutostartState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      setState(await getAutostartState());
      setError(undefined);
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Another window may have changed the entry; re-read when this window regains
  // focus so the toggle never shows a stale value.
  useEffect(() => {
    const handleFocus = () => {
      void refresh();
    };
    window.addEventListener('focus', handleFocus);
    return () => window.removeEventListener('focus', handleFocus);
  }, [refresh]);

  const toggle = useCallback(async () => {
    if (busy || !state) return;
    const next = !state.system_enabled;
    setBusy(true);
    setError(undefined);
    try {
      setState(await setAutostartState(next));
    } catch (cause) {
      setError(describeError(cause));
      // Re-read so the switch reflects what the registry actually holds.
      await refresh();
    } finally {
      setBusy(false);
    }
  }, [busy, refresh, state]);

  return {
    enabled: state?.system_enabled ?? false,
    /** `false` means the startup entry is not readable/writable on this machine. */
    available: state?.available ?? true,
    /** True when the last read repaired an entry removed outside the app. */
    repaired: state?.repaired ?? false,
    busy,
    error,
    toggle,
    refresh,
  };
}
