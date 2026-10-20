/**
 * 开机自启动域：读取状态与开关提交。
 * 复用 @tauri-apps/plugin-autostart，不在此重复原生命令。
 */
import { useCallback, useEffect, useState } from 'react';
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart';

export function useAutostartSettings() {
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    isAutostartEnabled().then(setEnabled).catch(() => setEnabled(false));
  }, []);

  const toggle = useCallback(async () => {
    try {
      if (enabled) {
        await disableAutostart();
        setEnabled(false);
      } else {
        await enableAutostart();
        setEnabled(true);
      }
    } catch (err) {
      console.error('Failed to toggle autostart:', err);
    }
  }, [enabled]);

  return { enabled, toggle };
}
