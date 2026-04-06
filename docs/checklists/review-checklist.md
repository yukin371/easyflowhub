# EasyFlowHub Review Checklist

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-04-07

## Purpose

这份 checklist 用于在实现前和 review 时快速确认三件事：

1. 改动是否落在正确 owner 上
2. import 和依赖方向是否没有越层
3. 验证与文档同步是否足够支撑合入

## When To Use

- 新增共享能力、wrapper、service、module、panel
- 改动跨 `TS / Rust / Go` 任意两层
- 改动 `manager`、`relay`、`extensions`、`notes`、`mcp` 这类 canonical owner
- code review 里怀疑出现重复实现、错误 owner 或越层 import

## 1. Pre-Change Intake

- [ ] 已读 `AGENTS.md`、`docs/README.md`、`docs/roadmap.md`、`docs/ARCHITECTURE_GUARDRAILS.md`
- [ ] 已确认本次改动影响哪些模块和哪一层：`TS`、`Rust`、`Go`
- [ ] 已确认现有 canonical owner，而不是从文件名猜位置
- [ ] 已写明验证方式：类型检查、测试、人工 smoke、或其组合
- [ ] 已写明需要同步的文档：`roadmap`、`MODULE.md`、`ADR`、`plan`

## 2. Import Review

- [ ] React 组件或页面没有直接散写新的 Tauri `invoke('...')` 命令名；前端原生命令走 `easyflowhub-app/src/lib/tauri`
- [ ] 前端共享解析、格式化、变换逻辑优先进入 `easyflowhub-app/src/lib`，不塞进随机 `utils`
- [ ] manager 新面板通过 `easyflowhub-app/src/modules` 注册进入，而不是只改侧边栏局部 wiring
- [ ] `src-tauri/src` 没有复制 `scriptmgr-go` 已拥有的脚本发现、执行、relay 或 MCP 业务逻辑
- [ ] `scriptmgr-go/internal/http`、`internal/mcpcli` 等非 owner 模块没有吸收 relay / extension 的核心职责

## 3. Ownership Review

- [ ] 新共享能力已先搜索现有实现，并说明为什么不能复用
- [ ] 每个新能力只有一个 canonical owner，没有在 `TS / Rust / Go` 两侧平行实现
- [ ] 如果只是临时适配层，已写明 cleanup 条件和退出时机
- [ ] 涉及 relay 的 provider 选择、failover、health 逻辑仍收敛在 `scriptmgr-go/internal/relay`
- [ ] 涉及 extension 的 manifest 扫描与贡献发现仍收敛在 `scriptmgr-go/internal/extensions`
- [ ] 涉及 manager 壳层、模块注册、原生命令 wrapper 的改动仍分别收敛在既有 owner

## 4. Review Questions

- [ ] 这次改动是否引入了第二个 owner？
- [ ] 是否把 domain 规则塞进了名字模糊的 helper / utils？
- [ ] 是否新增了跨层 import，但没有在 guardrails 或模块文档中说明？
- [ ] 是否存在“为了赶进度先复制一份”的实现？
- [ ] 是否有任何未验证的高风险行为被当作已完成？

## 5. Completion Gate

- [ ] 已运行与改动匹配的验证命令，或明确记录无法运行的原因
- [ ] 已检查是否需要补充人工 smoke，特别是 Windows 多窗口、autostart、tray、relay UI 等路径
- [ ] 已同步至少一项文档：`docs/roadmap.md`、相关 `MODULE.md`、`docs/decisions`、`docs/plans`
- [ ] roadmap 中的任务状态、验证债和 next queue 已按结果更新
- [ ] 最终说明包含：改了什么、怎么验证、剩余风险是什么
