# Ctrl+滚轮调整透明度功能设计

## 概述

为快速笔记窗口添加 Ctrl+滚轮调整透明度功能，支持分层透明度（背景可完全透明，文字保持最低可见度）。

## 功能规格

### 交互方式
- **触发**：按住 `Ctrl` + 滚轮上下滚动
- **步进**：每次滚动调整 10%
- **范围**：0% ~ 100%

### 透明度分层
| 层级 | 透明度范围 | 说明 |
|------|-----------|------|
| 背景 | 0% ~ 100% | 可完全透明 |
| 文字/图标 | minThreshold ~ 100% | 默认最低 20%，保证可读性 |

### 默认值
- 背景色：米黄色 `#faf8f0`
- 默认透明度：80%
- 透明度不持久化（每次启动重置）
- 文字最低透明度：20%（后期可从设置调整）

### 自动文字颜色
- 透明度 ≥ 50% → 黑字 `#333333`
- 透明度 < 50% → 白字 `#ffffff`

## 技术实现

### 状态管理
```typescript
const [opacity, setOpacity] = useState(80);
```

### 事件监听
```typescript
useEffect(() => {
  const handleWheel = (e: WheelEvent) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const delta = e.deltaY < 0 ? 10 : -10;
    setOpacity(prev => Math.max(0, Math.min(100, prev + delta)));
  };
  window.addEventListener('wheel', handleWheel, { passive: false });
  return () => window.removeEventListener('wheel', handleWheel);
}, []);
```

### 样式结构
```tsx
<div style={{ backgroundColor: '#faf8f0', opacity: opacity / 100 }}>
  <div style={{ opacity: Math.max(opacity, minTextOpacity) / 100 }}>
    {children}
  </div>
</div>
```

## 文件修改

| 文件 | 修改 |
|------|------|
| `src/App.tsx` | 添加 opacity 状态、滚轮监听、动态样式 |

## 后续优化（v2）

1. 从设置页读取 minTextOpacity 阈值
2. 真正检测背景亮度自动切换文字颜色
3. 可选：透明度持久化选项
