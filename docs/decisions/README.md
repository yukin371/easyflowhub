# Decisions

本目录只存放长期有效的架构 / 边界 / owner 决策。文件名 `ADR-NNNN-<slug>.md`，编号一旦分配不复用、不回收。

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

## 生命周期

ADR 不是“写完即永久”。一份决策要么仍在被代码执行，要么已经由新的决策接替或修正——**状态必须能读出来，而不是靠读者去猜哪份还算数**。

| 状态 | 含义 | 能否作为现行约束 |
|---|---|---|
| `proposed` | 已提出，未定论 | 否 |
| `accepted` | 现行决策，代码应照此执行 | 是 |
| `amended` | 整体仍现行，但点名的小节已被新 ADR 改掉 | 是（被改的小节除外） |
| `superseded` | 整体不再现行，由新 ADR 接替 | 否 |
| `withdrawn` | 提出后撤回 / 评估后不采纳 | 否 |

`amended` 是这套词表里最容易被省掉、也最容易被误用的一格。没有它，一份“方向还对、手段已换”的 ADR 只有两个坏选择：标成 `superseded`（会让仍然生效的部分一起失效），或者什么都不做（让已经死掉的那条继续被引用）。**部分取代不是小事，必须写清楚改的是哪一节。**

### 头字段

H1 之后、第一个 `## ` 之前，一行一个字段：

```markdown
- status: accepted
- date: 2026-11-27
- scope: v1.2+
```

- `status`：上面五个 token 之一，**只写 token**，解释放正文
- `date`：决策定稿日期，`YYYY-MM-DD`
- `scope`：管到哪——版本范围（`v0.2.0`、`v1.2+`）或 `长期`。这一项是失效判断的前提，没有它就无法回答“什么时候该回头看”
- 关系字段：`supersedes` / `superseded-by` / `amends` / `amended-by`

关系必须**双向**：A 写 `supersedes: B`，B 就必须写 `superseded-by: A`。单向指针等于只有一半人说得出真相，而读者往往先打开的是被取代的那一份。`amends` / `amended-by` 同理，且 `amends` **必须带小节号**（`amends: ADR-0004 §2`）——不带小节号的“部分修正”与整篇取代无法区分。

## 复查节奏

ADR 会静默过期：代码搬了家、owner 换了 crate、某条前提被新版本吃掉，而文件里的字一个都没错。所以按周期批量复查，而不是等有人想起来。

- **周期 90 天**，台账在 [`scripts/adr-review-log.json`](../../scripts/adr-review-log.json)，由 `scripts/check-adr-lifecycle.mjs` 检查
- 每次复查记一条：`date`（本次日期）、`kind`（`sweep` 或 `deep`）、`next`（下次预计日期）、`summary`（这轮查到了什么）
- **超期即红**：`next` 过了还没记账，门禁失败。这一条是刻意的——没有它会回到“有规则、没人执行”
- **长期未查必须深查**：实际复查日期比上一轮 `next` 晚了 30 天以上时，这一次的 `kind` 必须是 `deep`。拖得越久，需要重新验证的前提越多，例行扫一遍不足以支撑结论

两种深度做的事不一样：

| kind | 做什么 |
|---|---|
| `sweep` | 逐条核对 `status` / `scope` / 双向指针 / 路径是否还能解析；对每条 `accepted` 确认“决定还在被执行”——找到文件或函数级别的证据，不是读一遍正文 |
| `deep` | 例行全部内容，加上：① 代码里出现了新 owner / 新边界却没有 ADR 的空白；② 每条 `accepted` 的前提是否被版本或迁移吃掉；③ `## 失效条件` 是否已经触发。产出通常是新增 ADR，或把某条降级为 `amended` / `superseded` |

## 当前决策

| ADR | 决策 | 状态 | 日期 | scope |
|---|---|---|---|---|
| [0001](./ADR-0001-ai-doc-authority-and-bootstrap.md) | 文档权威顺序、分层与退役机制 | `accepted` | 2026-04-07 | 长期 |
| [0002](./ADR-0002-scriptmgr-automation-boundary.md) | scriptmgr 不接管仓库级截图 / 文档同步 / 通用自动化 | `accepted` | 2026-04-07 | 长期（v1 起） |
| [0003](./ADR-0003-product-direction-and-action-first.md) | 产品方向：Hub + Launcher + Manager，Capture / Act / Flow | `accepted` | 2026-10-16 | v0.2.0+ |
| [0004](./ADR-0004-glass-theme-and-efficiency-workspace.md) | 毛玻璃工作空间与个人效率 Hub 的视觉方向 | `amended` | 2026-10-16 | v0.2.0 and later |
| [0005](./ADR-0005-download-owner-and-safety.md) | 下载队列归 Tauri native，文件安全边界 | `accepted` | 2026-10-17 | v0.2.0 |
| [0006](./ADR-0006-voice-recording-and-inbox.md) | Windows 录音归 native，音频回流 Inbox | `accepted` | 2026-10-18 | v0.2.0（Phase 5 采集基线） |
| [0007](./ADR-0007-relay-local-api-security.md) | 两类本地 HTTP 只绑 loopback，密钥回显与恢复 | `accepted` | 2026-10-18 | v0.2.0（发布前安全收口） |
| [0008](./ADR-0008-voice-transcription-and-relay.md) | 转写归 native，Relay transport 与隐私确认 | `accepted` | 2026-10-19 | v0.2.0（Phase 5 转写基线） |
| [0009](./ADR-0009-windows-native-runtime.md) | 单进程原生 Rust 运行时与 egui / RichEdit 栈 | `accepted` | 2026-11-24 | v1.2+（Native Runtime track） |

最近一次复查的日期与下次预期日期**只写在台账里**，不在本文件重复——重复的那个数字没人有义务去改，
很快就会和台账对不上。门禁每次运行都会打印当前状态：`last <date> (<kind>), next <date>, N day(s) left`。

## 与其他文档的关系

- 短期设计进入 `docs/plans`（按生命周期分目录，见 [plans/README.md](../plans/README.md)）
- 当前优先级进入 `docs/roadmap.md`
- 模块级约束进入 `MODULE.md`
- 长期决策进入本目录

---

门禁：`node scripts/check-adr-lifecycle.mjs`（真实树）、`node --test scripts/check-adr-lifecycle.selfcheck.mjs`（规则自检）。判九件事：文件与命名、头字段、编号连续、关系双向、状态自洽、索引一致、路径可解析、复查台账未超期、失效 ADR 不被当前硬约束引用。
