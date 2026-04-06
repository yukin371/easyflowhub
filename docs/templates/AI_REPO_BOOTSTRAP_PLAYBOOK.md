# AI Repo Bootstrap Playbook

> 目标: 让 AI 在一个全新或存量仓库中，自动建立完整工作流程、标准和最小治理骨架
> 适用范围: Web / 桌面 / 后端 / CLI / 库 / 脚本仓库
> 使用方式: 将本文档复制到目标仓库，命名为 `docs/AI_REPO_BOOTSTRAP.md` 或直接作为一次性工作指令提供给 AI

## 1. 使命

你的任务不是“理解一点代码然后开始改”，而是先为这个仓库建立一套可持续的工作系统，使后续 AI 和人工协作都遵循同一套边界、验证方式和文档生命周期。

你要完成的不是单个功能，而是仓库级初始化：

1. 建立项目画像
2. 建立 AI 工作规则
3. 建立当前工作面
4. 建立架构边界和唯一 owner
5. 建立关键模块上下文
6. 建立设计文档与决策文档的生命周期

---

## 2. 强制原则

### 2.1 不编造

- 不确定的事实一律写 `TBD`
- 每个 `TBD` 后写明确认路径
- 不要把猜测写成标准

### 2.2 先复用后新增

在新增任何 shared helper / utility / service / adapter 前，必须先搜索现有实现。  
如果已有实现存在，优先扩展；不能复用时必须说明原因。

### 2.3 边界先于实现

在开始大改前，必须先明确：

- 目标模块
- 共享能力 owner
- 影响面
- 验证方案
- 要同步的文档

### 2.4 文档只写高价值信息

文档只记录工具不容易推导的信息：

- 业务意图
- 模块职责
- 依赖边界
- 禁止事项
- 不变量
- 权衡和例外

不要抄源码目录、函数列表、显而易见的结构。

### 2.5 完成必须闭环

任务不算完成，直到：

- 代码或文档已落地
- 验证已执行或明确阻塞
- 文档已同步
- 没有引入新的重复实现或架构漂移

---

## 3. 首次接手仓库时必须交付的文件

最少交付物：

1. `docs/PROJECT_PROFILE.md`
2. 根目录 `AGENTS.md`
3. `docs/roadmap.md`
4. `docs/ARCHITECTURE_GUARDRAILS.md`
5. 1 到 3 个关键模块的 `MODULE.md`

按需补充：

6. `docs/plans/YYYY-MM-DD-*.md`
7. `docs/decisions/ADR-*.md`

---

## 4. 执行顺序

严格按下面顺序执行，不要一开始就批量生成所有文档。

### Phase 1. 建立项目画像

产物：`docs/PROJECT_PROFILE.md`

从以下来源收集事实：

1. `README`
2. 依赖声明文件
   - `package.json`
   - `go.mod`
   - `Cargo.toml`
   - `pyproject.toml`
   - 其他语言等价文件
3. CI / workflow 文件
4. 入口代码
5. 构建、测试、运行脚本

必须确认的内容：

- 项目类型
- 技术栈
- 运行入口
- 验证命令
- 仓库拓扑
- 共享能力可能的 owner
- 已知高风险区域

输出要求：

- 只写高置信度事实
- 不确定项标 `TBD`
- 不得凭经验补全未知命令

### Phase 2. 建立仓库级工作规则

产物：根目录 `AGENTS.md`

必须写入：

- 修改前必读文件顺序
- “先复用后新增”规则
- 大改前必须先输出边界摘要
- 完成后必须输出验证结果和文档同步结果
- 哪些情况下必须停下并询问

### Phase 3. 建立当前工作面

产物：`docs/roadmap.md`

只保留：

- 当前版本目标
- 当前 active tracks
- 最近进展
- 待验证项
- 下一步

不要把 roadmap 写成历史流水账。

### Phase 4. 建立架构边界

产物：`docs/ARCHITECTURE_GUARDRAILS.md`

必须明确：

- 模块或层次划分
- 允许的依赖方向
- forbidden imports / forbidden ownership
- 每个跨切关注点的 canonical owner

典型跨切关注点包括：

- logging
- config
- auth
- persistence
- HTTP / API client
- shared utilities
- UI primitives
- error mapping
- file / path helpers
- feature flags

### Phase 5. 建立关键模块上下文

产物：关键目录下的 `MODULE.md`

只为关键模块生成，不要铺满全仓库。

优先级：

1. 入口模块
2. 共享能力模块
3. 容易重复造轮子的模块
4. 多人频繁改动的模块

每个 `MODULE.md` 至少包含：

- 模块职责
- owns
- must not own
- 关键依赖
- 不变量
- 常见坑
- 文档同步触发条件

### Phase 6. 建立任务与决策生命周期

产物：

- `docs/plans/YYYY-MM-DD-*.md`
- `docs/decisions/ADR-*.md`

规则：

- 临时方案和实施设计进入 `plans`
- 长期边界决策进入 `ADR`
- 一个 plan 完成后必须归档、收敛为 ADR，或同步到 roadmap / MODULE 后结束

---

## 5. AI 自适配规则

AI 必须根据项目类型微调文档重点。

### 5.1 Web / 前端项目

重点补：

- 页面入口
- 状态管理 owner
- API client owner
- UI primitives owner
- E2E / 浏览器验证流程

### 5.2 桌面项目

重点补：

- 前后端边界
- IPC / command owner
- 本地数据路径
- 多窗口 / 多进程入口
- 打包与平台特有验证方式

### 5.3 后端服务

重点补：

- API 入口
- service / repository 边界
- 数据库访问 owner
- 配置与鉴权 owner
- 集成测试和部署验证

### 5.4 CLI / 脚本仓库

重点补：

- 命令入口
- 参数解析 owner
- 输出格式约束
- 环境依赖
- 幂等性与安全边界

### 5.5 库 / SDK

重点补：

- public API surface
- compatibility promises
- examples / smoke tests
- versioning strategy
- forbidden internal leakage

---

## 6. 防止 AI 破坏架构的强制问题

在新增任何共享能力之前，必须回答：

1. 我搜索了哪些现有实现？
2. 为什么现有实现不能复用？
3. 新能力的 canonical owner 是谁？
4. 这会不会让两个模块同时拥有相同职责？
5. 哪些文档必须同步？
6. 是否需要 ADR 才能安全落地？

任何一个问题答不清，就先不要新增。

---

## 7. 文档防腐化规则

### 7.1 `PROJECT_PROFILE.md`

只在这些变化时更新：

- 技术栈变化
- 构建 / 测试命令变化
- 部署方式变化
- 核心拓扑变化

### 7.2 `roadmap.md`

只保留 active 内容。  
完成历史不要无限累积，转移到 plan / ADR 或直接清理。

### 7.3 `MODULE.md`

只记录：

- 职责
- owner 能力
- 不变量
- 依赖规则
- 常见坑

### 7.4 `plans`

只记录临时设计和实施方案。  
完成后必须收敛，不允许长期堆积为“设计坟场”。

### 7.5 `ADR`

只记录长期有效的决策。  
普通实现步骤、临时 workaround 不应写成 ADR。

---

## 8. 每次非平凡改动前必须输出的摘要

在实施前，先输出以下内容：

```text
目标模块：
现有 owner：
影响面：
计划改动：
验证方式：
需要同步的文档：
```

如果是高风险改动，再补：

```text
架构风险：
重复实现风险：
回滚路径：
```

---

## 9. 每次完成后必须输出的摘要

```text
已完成改动：
验证结果：
未验证区域：
同步文档：
残留风险：
```

---

## 10. 完成标准

仓库初始化完成的标准：

- `PROJECT_PROFILE.md` 已建立
- `AGENTS.md` 已建立
- `roadmap.md` 已建立
- `ARCHITECTURE_GUARDRAILS.md` 已建立
- 至少 1 到 3 个关键模块有 `MODULE.md`
- 设计文档和决策文档的入口已建立

单次任务完成的标准：

- 改动已实施
- 验证已完成或阻塞已说明
- 文档已同步
- 未引入新的重复 owner

---

## 11. 推荐与模板组合使用

若仓库中存在模板目录，优先按以下文件实例化：

- `docs/templates/PROJECT_PROFILE.template.md`
- `docs/templates/AGENTS.template.md`
- `docs/templates/roadmap.template.md`
- `docs/templates/ARCHITECTURE_GUARDRAILS.template.md`
- `docs/templates/MODULE.template.md`
- `docs/templates/plan.template.md`
- `docs/templates/ADR.template.md`

若模板不存在，则按本文档结构自行创建最小版本。

---

## 12. 可直接复制给 AI 的启动指令

下面这段可以直接作为对 AI 的一次性指令：

```text
你正在接手一个新仓库。你的第一目标不是直接改业务，而是为仓库建立可持续的 AI 工作流程和标准。

请严格按以下顺序执行：
1. 建立 docs/PROJECT_PROFILE.md，只写高置信度事实，未知写 TBD。
2. 基于项目画像生成根目录 AGENTS.md，明确读文件顺序、先复用后新增、验证和文档同步规则。
3. 建立 docs/roadmap.md，只保留当前版本目标、active tracks、最近进展、待验证和下一步。
4. 建立 docs/ARCHITECTURE_GUARDRAILS.md，明确层次、依赖方向、canonical owners、禁止重复实现的规则。
5. 为 1 到 3 个关键模块建立 MODULE.md，记录职责、owns、must not own、不变量和常见坑。
6. 如有当前任务，再按需创建 docs/plans/YYYY-MM-DD-*.md；如涉及长期边界决策，再创建 docs/decisions/ADR-*.md。

规则：
- 不编造未知事实
- 新增 shared utility / service 前必须搜索现有实现
- 不能确认 canonical owner 时先停下说明风险
- 完成后输出：改了什么、怎么验证、更新了哪些文档、还剩哪些风险
```

---

## 13. 一句话准则

AI 在任何仓库中的第一职责都不是“尽快写代码”，而是先建立边界、验证路径和知识入口，再开始持续交付。
