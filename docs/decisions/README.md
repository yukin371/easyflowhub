# Decisions

本目录只存放长期有效的架构 / 边界 / owner 决策。

## 何时新增 ADR

当改动满足任一条件时，应该新增 `ADR-*.md`：

- 改变 canonical owner
- 改变依赖方向
- 引入新的共享能力或跨层协议
- 暂时允许一个明确的架构例外
- 把多份临时 plan 收敛成长期标准

## 何时不要新增 ADR

- 普通 bug 修复
- 小范围 UI 调整
- 只影响当前迭代的实施步骤
- 代码已能直接说明、且不会形成长期规则的内容

## 状态建议

- `proposed`
- `accepted`
- `superseded`
- `archived`

## 与其他文档的关系

- 短期设计进入 `docs/plans`
- 当前优先级进入 `docs/roadmap.md`
- 模块级约束进入 `MODULE.md`
- 长期决策进入本目录

## 当前已接受的决策

- [ADR-0003 产品方向与 Action-first](./ADR-0003-product-direction-and-action-first.md)
- [ADR-0004 毛玻璃主题与效率工作空间](./ADR-0004-glass-theme-and-efficiency-workspace.md)
- [ADR-0005 下载队列归属与文件安全边界](./ADR-0005-download-owner-and-safety.md)
- [ADR-0006 Windows 录音归属与 Inbox 回流](./ADR-0006-voice-recording-and-inbox.md)
- [ADR-0007 Relay 本地 API 暴露与密钥回显边界](./ADR-0007-relay-local-api-security.md)
- [ADR-0008 录音后转写的 Owner、Provider 适配与隐私确认](./ADR-0008-voice-transcription-and-relay.md)
