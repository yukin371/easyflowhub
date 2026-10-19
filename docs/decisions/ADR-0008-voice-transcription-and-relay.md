# ADR-0008：录音后转写的 Owner、Provider 适配与隐私确认

## 状态

已接受（2026-10-19，Phase 5 转写切片设计基线）

## 背景

Phase 5 首个切片已交付 Windows 本地录音与 Inbox 音频回流（ADR-0006）。PRD-v0.2.0 §5.3 与交付计划把"录音后语音转文字"定义为单独的 Provider / 本地模型 slice，要求：

- 录音完成后按 Provider / 本地模型适配器转写，保留音频原件、文本和可选时间戳；
- 转写失败不影响音频保存；结果可写入笔记并可再次处理；
- 发送前显示目标 Provider 和隐私确认。

ADR-0006 §4 明确：翻译已复用 Relay 的决定不自动延伸到转写；未来转写必须通过 `TranscriptionProvider` adapter，提交前显示文件、目标 Provider 和隐私确认。本 ADR 落实该预留边界。

## 决策

### 1. Owner 与边界

- 转写的 native owner 是新增的 `easyflowhub-app/src-tauri/src/transcription.rs`；不放 ScriptMgr Go sidecar。录音采集仍归 `voice.rs`，本决策不改变其 owner。
- 转写请求在 native 侧执行，音频字节不进入 WebView：录音文件可能达到数十 MB，经 WebView 读取再 Base64 上传会放大内存与失败面；`file_search` / `download` / `voice` 均已是 native owner，转写遵循同一模式。
- native 只依赖 reqwest（下载队列已启用默认 Rustls crypto provider）与 Relay 的 loopback 地址，不引入新依赖。
- 前端通过 `easyflowhub-app/src/lib/tauri/transcription.ts` typed bridge 调用；确认 UI、结果展示与设置格式化归共享控件 `easyflowhub-app/src/components/manager/shared/TranscribeControl.tsx`。页面（Voice 面板、Inbox 面板）不各自实现请求逻辑，只挂载该控件并衔接各自的“整理为笔记”入口。
- Launcher 不新增转写动作：转写需要可见的确认界面，沿用 ADR-0006 "开始录音必须在可见界面中发生"的同一原则。从 Launcher 到达转写的路径仍是"打开录音面板 / 收集箱"。
- Inbox 表与其附件生命周期仍归 `inbox.rs`；transcription.rs 只调用其受控内部 helper（查询音频附件、写入 / 读取 transcript 附件），不直接写库。

### 2. Provider 适配

- 首个 Provider 复用本机 ScriptMgr Relay，调用 OpenAI 兼容 `POST /v1/audio/transcriptions`（multipart：`file`、`model`、可选 `language`）。Relay 已按路径前缀透传 `/v1/` 请求，密钥、路由与 failover 继续由 `scriptmgr-go/internal/relay` 拥有；EasyFlowHub 不存储、不读取 Relay 密钥。
- Relay 地址使用与翻译一致的本机默认值 `http://127.0.0.1:8787`；不新增 Provider 密钥配置界面。
- 请求通过 `TranscriptionTransport` trait 抽象，首个实现是 Relay HTTP transport；fake transport 用于单元测试，未来本地模型 / CLI Provider 以实现同一 trait 的方式接入，不在调用方复制分支逻辑。
- 模型与语言来自设置的 `transcription` 段：`model` 必填（为空时返回设置指引错误），`language` 为可选提示（留空表示自动识别）。设置只保存偏好，不保存凭据。
- 上限：音频文件 25 MB（约 4.5 分钟 48 kHz / mono / 16-bit WAV），超出时给出可理解的截断 / 分段建议错误；请求截止 180 秒；响应只接受 OpenAI 兼容的 `text` 字段，为空视为失败。
- 失败分类为固定中文文案：连接失败、鉴权失败、限流、接口不可用（404）、上游不可用（5xx）、超时、空结果、文件过大、未配置模型、无音频附件。原始 Relay / Provider 响应体不回显。

### 3. 隐私与确认

- 任何音频离开本机前，界面必须显示：文件名、大小、估算时长、目标 Provider（本机 Relay）、模型，以及"音频将通过 Relay 发送到其配置的上游 Provider"的明确说明；用户显式确认后才调用 native 命令。
- 未确认时不发送；确认状态不跨会话持久化，每次转写都要重新确认。
- 转写失败不删除、不修改音频原件与已有 transcript；重新转写在同一次事务内替换旧 transcript 附件，不产生重复条目。
- 录音流程本身（ADR-0006）不调用网络；本 ADR 只覆盖用户显式发起的转写。

### 4. 数据模型

- `inbox_attachments.kind` 增加 `transcript`（additive schema，沿用 audio 的迁移方式：检测 CHECK 约束缺失后重建表）。
- transcript 以文本附件落盘：随机存储名、元数据行、与音频同属一个 Inbox item，遵循附件即事实源的约定；UI 不拼接 app-data 路径。
- `convert_inbox_item_to_note` 对 `transcript` 附件把文本内容嵌入笔记正文（区别于 `附件：名称` 的单行引用），满足"结果可写入笔记并可再次处理"。
- transcript 内容通过 native 命令按 item 读取（有界读取，上限 1 MB），供面板在重新打开时恢复已转写状态；Inbox 列表不内联 transcript 全文。

### 5. 状态与入口契约

- 命令集合：`transcribe_audio(app, item_id)` 与 `get_transcript(app, item_id)`；均以 Inbox item 为键，不绑定易失的录音会话，因此 Voice 面板（保存后）与 Inbox 面板（历史音频）都能使用。
- `transcribe_audio` 返回 `{ text, model, attachmentId, audioSizeBytes, durationMs }`；重复转写替换旧 transcript 并返回新附件 id。
- `get_transcript` 返回 `Option<{ text, model, attachmentId, createdAt }>`；无 transcript 时为空，不报错。
- Voice 面板在 `saved` 状态展示转写卡片：未转写时显示确认入口；已转写时展示文本、复制、整理为笔记与重新转写。Inbox 面板对含音频附件的项目提供同一确认控件。

## 测试与验收

### 自动化

- Rust：fake transport 覆盖成功、空文本、各错误分类；大小守卫、WAV 时长解析、响应解析为纯函数测试；inbox transcript 写入 / 替换 / 读取与 convert 嵌入覆盖；设置 `transcription` 段默认值、部分值与往返。
- TypeScript：bridge 命令名与 camelCase 参数；设置默认值与合并；Voice / Inbox 面板的确认流（未确认不调用、确认后调用、失败可重试）组件测试。
- Go：无变更。

### Windows smoke（ Gate F5 延伸）

1. 录音保存后，确认界面展示文件 / 大小 / 时长 / Provider / 模型，取消不发送任何请求。
2. 确认后转写成功，文本可见、可复制、可整理为笔记；笔记正文包含转写文本。
3. Relay 未启动、模型未配置、Provider 不支持转写接口时均有可理解失败；音频原件与 Inbox 项目不受影响。
4. 重新打开 Manager 后，已转写项目的 transcript 仍可读取。
5. 真实 Provider（可达的多模态 / whisper 兼容上游）至少完成一次成功转写。

## 不在本决策内

- 实时转写、说话人识别、时间戳对齐、VAD 与音频编辑。
- 本地模型 / CLI Provider 适配器（接口已预留，实现另行评审）。
- 自动转写：保存录音后不会自动发送音频，必须用户显式确认。
- 超过 25 MB 的长音频分段转写与拼接。
