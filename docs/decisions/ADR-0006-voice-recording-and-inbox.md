# ADR-0006：Windows 录音归属、可见状态与 Inbox 回流

## 状态

已接受（2026-10-18，Phase 5 采集实现基线）

## 背景

EasyFlowHub 需要把学习、会议和临时想法中的声音纳入个人效率 Hub。现有 Inbox 可以保存文本、链接、图片和普通文件，但没有音频附件或麦克风会话模型。录音还涉及 Windows 麦克风权限、设备占用、应用退出时的数据完整性，以及将音频发送到外部转写服务的隐私边界。

Phase 5 先形成可验证的“录音 -> 本地文件 -> Inbox”闭环。实时转写、说话人识别和音频编辑会扩大设备、网络和资源占用，不能与首个录音切片绑定交付。

## 决策

### 1. Owner 与边界

- 录音的 native owner 是 `easyflowhub-app/src-tauri/src/voice.rs`；不放入 ScriptMgr Go sidecar，也不让 React 直接操作麦克风流。
- 前端通过 `easyflowhub-app/src/lib/tauri/voice.ts` 使用 typed bridge。Manager 的 Voice 面板负责会话控制和历史状态，Launcher 只提供“打开录音面板”入口；开始录音必须在可见界面中发生。
- Inbox 仍是附件的唯一归档入口。录音完成后由 native 侧调用现有 Inbox 写入流程的受控扩展，创建 `audio` 附件并保留 `source=voice`、`context=launcher|manager`、标题和时间；不建立第二套音频数据库。
- 转写属于后续 adapter，不属于本切片的成功条件。没有用户明确选择 Provider 和确认发送前，音频只保存在本地。

### 2. Windows-first 采集实现

- 首个实现使用默认 Windows WASAPI 输入设备。`voice.rs` 在专用采集线程中使用 `cpal` 负责设备 / 输入流，使用 `hound` 负责 WAV 封装；Tauri 全局状态只保存命令通道和可序列化会话快照，不保存也不跨线程传递 `cpal::Stream`。设备选择和更复杂格式作为后续设置扩展。
- 默认格式固定为 `48 kHz / mono / 16-bit PCM WAV`，文件扩展名和 MIME 为 `.wav` / `audio/wav`。停止时先 flush 并关闭 writer，再将随机 `.part` 文件原子提升为完成文件。
- 录音状态是 `idle -> recording -> paused|stopping -> saved|failed`。暂停只停止向文件写入，不销毁会话；继续沿用同一文件。`stopping` 期间禁止重复停止，失败必须给出可读错误。
- 活跃会话保存在 native 进程状态中，录音线程不能阻塞 Tauri 命令或 WebView。关闭 Manager 窗口只隐藏窗口，录音可继续；应用进程退出时不承诺续录，但已完成的 WAV 不得丢失，未完成 `.part` 必须保留。启动时扫描并标记中断文件仍是后续 slice，不作为当前实现的已交付能力。

### 3. Inbox 扩展方式

- `inbox_items.kind` 增加 `audio`，`inbox_attachments.kind` 增加 `audio`；这是 additive schema 变更，不改现有 notes 表和 `asset:<uuid>` 引用。
- `CaptureInboxItemInput` 增加受控的音频文件输入（或由 `voice.rs` 调用 inbox 内部函数），沿用已有随机存储名、临时文件、事务和附件打开路径。音频写入失败时回滚数据库并清理临时文件；数据库提交成功后才将会话标记为 `saved`。
- 音频附件通过附件 id 打开，UI 不拼接 app-data 路径。删除/归档沿用 Inbox 现有语义；MVP 不自动清理用户音频。

### 4. 权限、设备和隐私

- 首次点击“开始录音”才请求/触发 Windows 麦克风访问；界面明确显示设备名称、录音中/暂停状态、时长和麦克风占用提示。不得在启动、托盘驻留或 Launcher 唤起时静默录音。
- 无输入设备、权限拒绝、设备被占用、写入失败和空间不足都映射为用户可理解的错误，并提供打开 Windows 麦克风隐私设置（`ms-settings:privacy-microphone`）的入口；这些错误不能让 Manager 或 Launcher 崩溃。
- Phase 5 首个 slice 不调用网络、不上传音频、不要求 Relay 密钥。未来转写必须通过 `TranscriptionProvider` adapter，提交前显示文件、目标 Provider 和隐私确认；翻译已复用 Relay 的决定不自动延伸到转写。

### 5. 状态与入口契约

建议 native 返回以下最小模型：

```text
VoiceSession {
  id, title, status, startedAt, stoppedAt,
  durationMs, deviceName, sampleRate, channels,
  audioPath?, inboxItemId?, error?
}
```

`voice_get_state`、`voice_start`、`voice_pause`、`voice_resume`、`voice_stop`、`voice_open` 是首个 typed bridge 命令集合。`voice_start` 返回可轮询的会话，不把音频字节传入 WebView；进度只传状态和时长。

Launcher 的 `builtin.voice.open` 动作只打开带明确“开始录音”按钮的 Voice 面板。native 支持暂停、继续和停止命令；面板已提供这些控制、时长 / 设备状态轮询以及停止后的“打开附件 / 在收集箱查看”入口。录音失败必须保留可读错误和 `.part` 路径，允许用户重新开始；不得把失败状态显示为已保存。后续可以根据用户试用再增加按键直接开始，但必须保留可见录音指示。

### 6. 当前实现边界

- 已实现：Windows 默认输入设备采集、专用线程、`cpal` 输入流、`hound` WAV 写入、暂停 / 继续 / 停止控制、`.wav.part` 到 `.wav` 的同目录提升、Inbox 音频归档和 `voice_open`。
- 已实现的失败保护：初始化、权限 / 设备、回调、写入、WAV finalize、文件提升和 Inbox 归档失败均进入 `failed`；失败路径会保留临时或最终音频路径，只有 Inbox 提交成功才进入 `saved`。
- 尚待收口：Windows 打包态实机验证、麦克风隐私设置快捷入口、设备选择、启动时中断 `.part` 扫描和录音后转写 Provider。当前 Voice 面板生命周期控制与状态轮询已完成。

## 测试与验收

### 自动化

- Rust：当前先用 fake backend 固定状态转移（recording -> pause -> resume -> stop 及失败不进入 saved），不在 CI 依赖物理麦克风。后续应将文件归档和错误注入抽成可测试 backend，覆盖重复 pause/resume/stop、空音频、设备 / 权限 / 写入失败、`.part` 原子提升和中断文件保留。
- Rust Inbox：覆盖 audio kind schema、WAV 文件归档、事务失败清理、附件打开和重复回流不产生副本。
- TypeScript：覆盖 voice bridge 命令参数、Launcher action 注册、Manager 状态显示、不可用错误和停止后 Inbox 刷新。

### Windows smoke（Gate F5）

1. 默认输入设备开始、暂停、继续、停止各完成一次，WAV 可播放且进入 Inbox。
2. 录音中隐藏/重新打开 Manager，状态与时长保持；关闭窗口不丢失已保存音频。
3. 无麦克风、拒绝权限和设备占用时，提示原因并能打开隐私设置；不能产生“已保存”假状态。
4. 写入或停止失败时 `.part` 未被静默删除，重试后不会重复生成 Inbox 项。
5. 录音流程不发起网络请求；未配置转写 Provider 时界面不显示“自动转写已完成”。

## 不在本决策内

- 实时转写、说话人识别、VAD、降噪、音频编辑和多轨录音。
- 应用完全退出后的自动续录或队列恢复。
- 将转写 Provider 默认绑定到 Relay；Provider、权限提示和用户确认需要单独 ADR。
