# EasyFlowHub Windows Package Smoke

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-04-07

## Scope

用于 Windows 打包产物的最小发布前验收，目标是先确认产物齐全，再覆盖最容易在打包态回归的路径：

- 安装器与 portable 产物是否生成
- 应用是否能正常启动，不弹多余终端
- manager / quick note / todo card / settings / relay 面板的主路径是否可用
- autostart 是否仍保持隐藏启动语义

## Preflight

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

说明：

- 当前仓库可能同时出现历史 `DeskFlow_*` 与当前 `EasyFlowHub_*` 命名的安装包。
- 发版验收时应明确以当前正式产品命名为准，并记录旧命名产物是否仍需要保留。

## Manual Smoke Checklist

### A. Installer / Launch

- [ ] NSIS 安装器可启动并完成安装
- [ ] MSI 安装器可启动并完成安装
- [ ] portable EXE 可直接启动
- [ ] 启动时不弹额外终端窗口
- [ ] manager 窗口可正常显示

### B. Manager Core Path

- [ ] manager 可切换 `settings / scripts / notes / relay` 面板
- [ ] notes 面板可打开已有笔记并编辑保存
- [ ] settings 面板可正常读取当前设置
- [ ] relay 面板在 sidecar 未启动或 relay 未启动时能展示可理解状态，而不是静默失败

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

只有在产物校验通过，且上面四组 smoke 没有阻塞发布的问题时，才能把打包态验证标记为完成。
