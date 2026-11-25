/// <reference types="vitest" />
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'jsdom',
    // Node 24+ 自带一个返回 undefined 的全局 localStorage，会挡住 jsdom 的实现；
    // 这个 setup 把它指回 jsdom，否则同一份测试在 Node 22 / CI 的 lts/* 上结论不同。
    setupFiles: ['./src/test/setupDomStorage.ts'],
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
    },
  },
});
