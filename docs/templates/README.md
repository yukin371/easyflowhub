# AI Project Governance Templates

> 目标: 提供一套可跨项目复用、可由 AI 自行微调的治理模板
> 适用范围: Web / 桌面 / 后端 / CLI / 脚本仓库

## 1. 设计目标

这套模板不是为了“多写文档”，而是为了让 AI 在进入新项目时快速建立正确边界：

1. 快速理解项目结构、技术栈和验证方式
2. 在修改前先检查是否已有现成实现
3. 避免跨包重复实现同类能力
4. 让进度、设计和标准文档保持短、准、可清理

## 2. 模板清单

| 文件 | 作用 | 建议优先级 |
|---|---|---|
| `AI_REPO_BOOTSTRAP_PLAYBOOK.md` | 单文档版仓库初始化手册，可直接复制到其他仓库或交给 AI | 必须 |
| `PROJECT_PROFILE.template.md` | 项目画像，供 AI 建立基础上下文 | 必须 |
| `AGENTS.template.md` | 仓库级 AI 行为规则 | 必须 |
| `roadmap.template.md` | 当前阶段目标、进度和验证债务 | 必须 |
| `ARCHITECTURE_GUARDRAILS.template.md` | 架构边界与唯一归属规则 | 必须 |
| `MODULE.template.md` | 模块职责、归属、依赖和不变量 | 强烈建议 |
| `plan.template.md` | 单个设计 / 实施任务文档 | 强烈建议 |
| `ADR.template.md` | 长期架构决策记录 | 强烈建议 |

## 3. 推荐用法

优先顺序：

1. 先使用 `AI_REPO_BOOTSTRAP_PLAYBOOK.md` 作为总入口
2. 再按需实例化各个 template 文件
3. 不要跳过项目画像和架构边界定义，直接生成大批业务文档

## 4. AI 实例化顺序

AI 接手一个新项目时，建议按下面顺序实例化，而不是一次性生成全部文档。

### Step 1. 建立项目画像

先生成 `PROJECT_PROFILE.md`，来源按优先级：

1. 仓库现有 `README`
2. `package.json` / `go.mod` / `Cargo.toml` / `pyproject.toml`
3. CI 文件
4. 入口代码
5. 测试和脚本命令

要求：

- 只写高置信度事实
- 不确定的内容写 `TBD`
- 每个 `TBD` 后写明后续确认路径

### Step 2. 建立仓库规则

基于 `PROJECT_PROFILE.md` 生成根目录 `AGENTS.md`，明确：

- 先读哪些文件
- 哪些目录有唯一归属
- 哪些能力禁止重复实现
- 哪些场景必须先验证再改

### Step 3. 建立当前工作面

基于当前迭代目标生成 `docs/roadmap.md`：

- 当前版本目标
- 当前 active tracks
- 已完成
- 待验证
- 下一步

### Step 4. 建立架构硬边界

生成 `docs/ARCHITECTURE_GUARDRAILS.md`，明确：

- 模块层次
- 依赖方向
- 共享能力 owner
- forbidden duplication

### Step 5. 为核心模块生成 `MODULE.md`

只给核心目录生成，不需要全仓库铺开。

优先给这些目录生成：

1. 入口模块
2. 公共能力模块
3. 容易重复造轮子的模块
4. 多人频繁改动的模块

### Step 6. 随任务增量生成 `plan` / `ADR`

- 临时任务进入 `docs/plans`
- 长期决策进入 `docs/decisions`

不要把所有讨论都变成长期文档。

## 5. AI 自适配规则

AI 在不同项目中微调模板时，应遵守以下规则：

1. **先识别项目类型**
   Web、桌面、后端服务、库、CLI、脚本仓库的模板重点不同。
2. **先识别验证出口**
   类型检查、单元测试、集成测试、E2E、构建命令必须优先填充。
3. **先识别共享能力 owner**
   如日志、HTTP 客户端、配置、状态管理、数据库访问、认证、文件存储。
4. **文档只写工具不易推导的信息**
   业务意图、边界、禁令、例外、权衡，优先于代码结构抄录。
5. **未知不要编造**
   一律写 `TBD`，并记录如何确认。

## 6. 推荐目录结构

```text
<repo>/
  AGENTS.md
  docs/
    roadmap.md
    PROJECT_PROFILE.md
    ARCHITECTURE_GUARDRAILS.md
    decisions/
      ADR-*.md
    plans/
      YYYY-MM-DD-*.md
  <core-module>/
    MODULE.md
```

## 7. 防腐化规则

### 7.1 项目画像

`PROJECT_PROFILE.md` 只在这些变化时更新：

- 技术栈切换
- 构建 / 测试命令变化
- 部署形态变化
- 模块边界变化

### 7.2 roadmap

`roadmap.md` 只保留 active 内容，不保留长历史。

### 7.3 plan

每个 plan 完成后必须三选一：

1. 更新 roadmap 后归档
2. 更新 `MODULE.md` 后归档
3. 提炼成 ADR 后标记 `superseded`

### 7.4 ADR

只记录长期有效的决策，不记录普通实现步骤。

### 7.5 MODULE

`MODULE.md` 只写：

- 模块职责
- owner 能力
- 依赖规则
- 不变量
- 常见坑

不抄源码结构。

## 8. 最小执行清单

AI 在首次接手一个项目时，最少应完成：

1. `PROJECT_PROFILE.md`
2. `AGENTS.md`
3. `docs/roadmap.md`
4. `docs/ARCHITECTURE_GUARDRAILS.md`
5. 1 到 3 个关键模块的 `MODULE.md`

只完成这五项，项目可维护性就会明显改善。
