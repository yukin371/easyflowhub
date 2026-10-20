/**
 * 开发者日志查看域：读取 / 刷新 / 清空 native 调试日志。
 * 继续复用 typed bridge lib/tauri/logging，不在组件里散落 invoke。
 */
import { useCallback, useEffect, useState } from 'react';
import { clearDebugLogs, getDebugLogs } from '../../../lib/tauri/logging';

export function useDebugLogs() {
  const [lines, setLines] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setLines(await getDebugLogs(200));
      setError(undefined);
    } catch (err) {
      // Browser preview / non-Tauri shells cannot read native logs; keep this
      // out of the settings error banner.
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const clear = useCallback(async () => {
    await clearDebugLogs().catch(() => undefined);
    await refresh();
  }, [refresh]);

  return { lines, loading, error, refresh, clear };
}
