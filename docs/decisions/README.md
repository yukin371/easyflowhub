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
