/**
 * useHistory - 撤销/重做历史管理 Hook
 *
 * 特性：
 * - 支持动态配置的历史步数限制
 * - 防抖合并连续输入（debounce）
 * - 暴露 canUndo/canRedo 状态
 */

import { useCallback, useRef, useState, useEffect } from 'react';

interface HistoryState<T> {
  past: T[];
  present: T;
  future: T[];
}

interface UseHistoryOptions {
  maxSteps: number;
  debounceMs?: number; // 防抖时间，默认 300ms
}

interface UseHistoryReturn<T> {
  value: T;
  setValue: (value: T, skipHistory?: boolean) => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  clear: () => void;
  flush: () => void; // 强制刷新待处理的更新
}

export function useHistory<T>(
  initialValue: T,
  options: UseHistoryOptions = { maxSteps: 100, debounceMs: 300 }
): UseHistoryReturn<T> {
  const { debounceMs = 300 } = options;

  // 使用 ref 存储 maxSteps，支持动态更新
  const maxStepsRef = useRef(options.maxSteps);

  // 更新 maxSteps ref
  useEffect(() => {
    maxStepsRef.current = options.maxSteps;
  }, [options.maxSteps]);

  const [state, setState] = useState<HistoryState<T>>({
    past: [],
    present: initialValue,
    future: [],
  });

  // 防抖相关
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingValueRef = useRef<T | null>(null);
  const lastRecordedValueRef = useRef<T>(initialValue);

  // 强制将待处理的值写入历史
  const flushPendingUpdate = useCallback(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }

    if (pendingValueRef.current !== null) {
      const value = pendingValueRef.current;
      pendingValueRef.current = null;

      setState((prev) => {
        const previousRecordedValue = lastRecordedValueRef.current;

        // 检查值是否真的改变了
        if (previousRecordedValue === value) {
          return prev;
        }

        // 限制历史步数（使用 ref 获取最新值）
        const maxSteps = maxStepsRef.current;
        const newPast = [...prev.past, previousRecordedValue].slice(-maxSteps);
        lastRecordedValueRef.current = value;

        return {
          past: newPast,
          present: value,
          future: [], // 新操作清空重做栈
        };
      });
    }
  }, []);

  // 组件卸载时清理
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, []);

  const setValue = useCallback(
    (value: T, skipHistory = false) => {
      // 立即更新 present（保证 UI 响应）
      setState((prev) => ({ ...prev, present: value }));

      if (skipHistory) {
        // 跳过历史记录，直接更新
        lastRecordedValueRef.current = value;
        pendingValueRef.current = null;
        if (debounceTimerRef.current) {
          clearTimeout(debounceTimerRef.current);
          debounceTimerRef.current = null;
        }
        return;
      }

      // 检查值是否改变
      if (value === lastRecordedValueRef.current) {
        return;
      }

      // 设置待处理的值
      pendingValueRef.current = value;

      // 清除之前的定时器
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }

      // 设置新的防抖定时器
      debounceTimerRef.current = setTimeout(() => {
        flushPendingUpdate();
      }, debounceMs);
    },
    [debounceMs, flushPendingUpdate]
  );

  const undo = useCallback(() => {
    // 先刷新待处理的更新
    flushPendingUpdate();

    setState((prev) => {
      if (prev.past.length === 0) {
        return prev;
      }

      const previous = prev.past[prev.past.length - 1];
      const newPast = prev.past.slice(0, -1);
      lastRecordedValueRef.current = previous;

      return {
        past: newPast,
        present: previous,
        future: [prev.present, ...prev.future],
      };
    });
  }, [flushPendingUpdate]);

  const redo = useCallback(() => {
    // 先刷新待处理的更新
    flushPendingUpdate();

    setState((prev) => {
      if (prev.future.length === 0) {
        return prev;
      }

      const next = prev.future[0];
      const newFuture = prev.future.slice(1);
      lastRecordedValueRef.current = next;

      return {
        past: [...prev.past, prev.present],
        present: next,
        future: newFuture,
      };
    });
  }, [flushPendingUpdate]);

  const clear = useCallback(() => {
    // 清除待处理的更新
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
    pendingValueRef.current = null;

    setState((prev) => {
      lastRecordedValueRef.current = prev.present;
      return {
        past: [],
        present: prev.present,
        future: [],
      };
    });
  }, []);

  const flush = useCallback(() => {
    flushPendingUpdate();
  }, [flushPendingUpdate]);

  return {
    value: state.present,
    setValue,
    undo,
    redo,
    canUndo: state.past.length > 0,
    canRedo: state.future.length > 0,
    clear,
    flush,
  };
}
