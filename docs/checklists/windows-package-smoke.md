# EasyFlowHub Windows Package Smoke

> status: active
> owner: EasyFlowHub maintainers
> last_updated: 2026-10-18

## Scope

用于 Windows 打包产物的最小发布前验收，目标是先确认产物齐全，再覆盖最容易在打包态回归的路径：

- 安装器与 portable 产物是否生成
- 应用是否能正常启动，不弹多余终端
- manager / quick note / todo card / settings / relay 面板的主路径是否可用
- Launcher 的 Snow Shot 动作能否从 D 盘安装进入截图会话
- Voice 面板能否在 Windows 默认麦克风上完成录音、暂停、继续、停止并回流 Inbox
- autostart 是否仍保持隐藏启动语义

## Preflight

### Automated portable launch preflight (2026-10-18)

- [x] 本轮生成的 `src-tauri/target/release/easyflowhub.exe` 已启动并保持进程存活 5 秒，随后仅关闭本次测试进程；未观察到启动即退出。
- [x] `src-tauri/target/release/scriptmgr.exe` 已随 portable / NSIS / MSI 输出，MSI CAB 中 sidecar 哈希与新构建二进制一致。
- [ ] 该检查只证明便携版进程可启动，不替代下方安装器、窗口控件、权限和外部应用人工 smoke。

1. 构建产物

```powershell
Set-Location easyflowhub-app
bun install
bunx tauri build
```

2. 校验产物是否齐全

```powershell
pwsh -File .\scripts\check-release-artifacts.ps1
```

## Expected Artifacts

- `easyflowhub-app/src-tauri/target/release/bundle/nsis/*.exe`
- `easyflowhub-app/src-tauri/target/release/bundle/msi/*.msi`
- `easyflowhub-app/src-tauri/target/release/easyflowhub.exe` 或 `EasyFlowHub.exe`
- `easyflowhub-app/src-tauri/target/release/scriptmgr.exe`

说明：

- 当前仓库可能同时出现历史 `DeskFlow_*` 与当前 `EasyFlowHub_*` 命名的安装包。
- 发版验收时应明确以当前正式产品命名为准，并记录旧命名产物是否仍需要保留。

## Manual Smoke Checklist

### A. Installer / Launch

- [ ] NSIS 安装器可启动并完成安装
- [ ] MSI 安装器可启动并完成安装
- [ ] portable EXE 可直接启动
- [ ] 安装后的 EasyFlowHub 可从应用目录启动 `scriptmgr.exe`，Scripts / Relay 面板主路径可用
- [ ] 启动时不弹额外终端窗口
- [ ] manager 窗口可正常显示

### B. Manager Core Path

- [ ] manager 可切换 `settings / scripts / notes / relay` 面板
- [ ] notes 面板可打开已有笔记并编辑保存
- [ ] settings 面板可正常读取当前设置
- [ ] relay 面板在 sidecar 未启动或 relay 未启动时能展示可理解状态，而不是静默失败
- [ ] 已保存的 API key / 认证 header 显示为脱敏占位值；不改占位值保存后原密钥仍可用，清空或删除密钥字段后不再发送旧密钥
- [ ] 含 userinfo / query 凭据的 provider `base_url` 显示为占位值；保持不变可保存，替换占位值可修改地址
- [ ] Downloads 面板可加入公开小文件，显示进度并完成落盘
- [ ] 下载可暂停 / 继续 / 取消；网络失败后可重试
- [ ] 同名文件按 rename / fail / overwrite 策略反馈，overwrite 前有明确确认
- [ ] 完成下载可打开、定位、复制路径并加入 Inbox
- [ ] 下载“复制路径”能在其他应用粘贴；剪贴板被占用时显示明确错误且不影响队列
- [ ] 无权限目录和无效 URL 显示可理解错误，不阻塞其他面板
- [ ] Files 面板可选择受控目录，点击刷新后显示索引数量和结果
- [ ] Files 面板可按文件名 / 路径片段搜索，并对文本和小图显示预览
- [ ] Files 面板的打开、资源管理器定位和复制路径动作可用；复制路径后系统剪贴板内容正确
- [ ] Files “复制路径”在打包态通过 native clipboard 生效；剪贴板被其他进程占用时显示可理解错误
- [ ] Launcher 读取剪贴板上下文、翻译结果复制在打包态通过 native clipboard 生效；Tauri bridge 不可用的浏览器回退不作为 Windows 验收依据
- [ ] Files 面板的重命名、移动、复制和回收站操作在目标冲突、越界目录和无权限场景显示可理解错误，不静默覆盖
- [ ] Files 面板暂停 / 恢复 / 清除索引后，结果和 generation 状态一致；重开应用后不会自动扫描目录

### C. Quick Note / Todo

- [ ] 全局或应用内入口可打开 quick note
- [ ] quick note 可输入 Markdown，并在关闭后重开时保留最新内容
- [ ] quick note 图片插入后不报错，重开笔记后仍能显示缩略图或预览
- [ ] 如存在旧笔记图片资源，legacy asset 路径在打包产物下仍可显示
- [ ] quick note 中移除图片后，当前窗口预览与重开后的内容一致
- [ ] `Esc`、`Alt+F4`、关闭按钮都只关闭当前 quick note 窗口，不误伤其他 quick note / manager 窗口
- [ ] todo card 可从 manager 或笔记路径打开
- [ ] todo card 回跳到来源笔记路径正常

### D. Autostart

- [ ] 在设置中开启 autostart 后，重启系统或使用等效验证方式确认应用能自动启动
- [ ] autostart 启动时 manager 默认保持隐藏
- [ ] autostart 启动时不出现卡死、空白窗口或额外终端
- [ ] 关闭 autostart 后配置状态与实际行为一致

### E. Launcher / Snow Shot

- [ ] 先在 Snow Shot「设置 → 系统」开启 MCP integration，并确认主程序与 `snow-shot-mcp.exe` 来自同一安装目录
- [ ] 从 Launcher 执行「截屏」后，Snow Shot 进入可见截图界面，而不只显示主窗口
- [ ] 对 D 盘注册安装验证自动发现；另用自定义路径验证其优先级
- [ ] 关闭 MCP integration 后，Launcher 给出设置指引且不跳安装页
- [ ] 移除或改名 MCP bridge 后，Launcher 提示修复 / 更新，不误判为未安装
- [ ] 在没有 Snow Shot 的环境中，Launcher 才打开官方安装页

### F. Capture / Inbox (P0)

- [ ] Launcher 可将剪贴板纯文本和 URL 分别捕获为 Inbox 项，并保留来源与捕获时间
- [ ] Manager 可粘贴图片并通过文件选择器、拖放添加文件；捕获后图片和附件可正常打开
- [ ] Snow Shot 复制截图后，Launcher 收集动作将图片保存到 Inbox；无图片时正确回退到文本，不重复或误报
- [ ] Inbox 内容在关闭并重开应用后仍存在；可在活动与归档视图间切换
- [ ] Inbox 项可归档、恢复，并可整理为笔记；重复触发整理不会创建重复笔记
- [ ] 图片超过 32 MB、无效 URL、缺失附件或无权限路径显示可恢复错误，不留下半成品 Inbox 项

### G. Voice / Microphone (P2)

- [ ] 首次点击“开始录音”时才触发 Windows 麦克风访问；启动应用、托盘驻留和唤起 Launcher 不应静默录音
- [ ] 默认输入设备可开始录音，Voice 面板显示设备名称、录音状态和持续时长
- [ ] 录音可暂停、继续并停止；停止后生成可播放的 `48 kHz / mono / 16-bit PCM WAV`
- [ ] 录音先写入同目录 `.wav.part`，正常停止后提升为 `.wav`，并在 Inbox 中出现一个 `audio` 项 / 附件
- [ ] 录音中隐藏并重新打开 Manager 后，会话状态和时长仍可读取；关闭窗口不应丢失已保存音频
- [ ] 无麦克风、拒绝权限、设备被占用或录音目录无权限时，显示可理解错误，不能进入“已保存”；错误应提供打开 `ms-settings:privacy-microphone` 的入口（若该入口已在当前 UI 提供）
- [ ] 回调、写入、停止 finalize、`.part` 提升或 Inbox 归档失败时，保留 `.part` 或最终音频路径，不静默删除；重试或重新开始不得重复创建 Inbox 项
- [ ] 录音过程不发起网络请求；未配置转写 Provider 时，不显示“自动转写已完成”

## Record Template

记录至少包含：

- 构建来源：本地构建或 CI artifact
- 验证日期
- Windows 版本
- 验证人
- 通过项
- 失败项
- 是否阻塞发布

## Exit Rule

v0.2.0 P0/P1 核心 RC 必须通过 A-F 与 [current Windows theme smoke](./windows-theme-smoke-2026-10-18.md) 的适用项；每项需通过，或记录明确的非阻塞例外和接受理由。Voice 属于 P2：不纳入核心 RC 时不阻塞发布，Phase 5 保持 `in_progress`；若 RC 包含 Voice，则 G 组必须通过后才能发布该能力。
