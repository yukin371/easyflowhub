/**
 * jsdom 环境下的 Web Storage 兜底。
 *
 * Node 24+ 在 `globalThis` 上定义了 `localStorage` / `sessionStorage` 访问器，
 * 只有启动时传入 `--localstorage-file` 才返回值，否则是 `undefined`：
 *
 *   (node:2904) ExperimentalWarning: localStorage is not available because
 *   --localstorage-file was not provided.
 *
 * vitest 的 jsdom 环境在把 jsdom 的全局补进来时会**跳过已存在的键**（否则会覆盖
 * `process`、`console` 这些 Node 自己的全局），于是 `localStorage` 就留成了 Node 那个
 * 返回 `undefined` 的访问器。而这一步发生在 `window` 已经指向同一个全局对象之后，
 * 所以 `window.localStorage` 同样是 `undefined`——"指回 jsdom"没有东西可指。
 *
 * 结果：Node 22（没有这个全局）本机 418 项全绿，CI 的 `lts/*` 8 个文件 59 项全红。
 *
 * 这里只在 storage 确实缺失时补一个等价实现；能拿到 jsdom 的就不动它，所以本机行为
 * 不变，CI 补上的是同一套 Storage 契约（getItem / setItem / removeItem / clear / key / length）。
 */
function createMemoryStorage(): Storage {
  const entries = new Map<string, string>();

  return {
    get length() {
      return entries.size;
    },
    clear: () => entries.clear(),
    getItem: (key: string) => entries.get(String(key)) ?? null,
    key: (index: number) => [...entries.keys()][index] ?? null,
    removeItem: (key: string) => {
      entries.delete(String(key));
    },
    setItem: (key: string, value: string) => {
      entries.set(String(key), String(value));
    },
  };
}

for (const key of ['localStorage', 'sessionStorage'] as const) {
  if ((globalThis as Record<string, unknown>)[key]) continue;

  Object.defineProperty(globalThis, key, {
    configurable: true,
    enumerable: true,
    value: createMemoryStorage(),
  });
}
