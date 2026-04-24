# EasyFlowHub Windows Package Smoke Record

> date: 2026-04-24
> status: in_progress
> verifier: Codex
> source: local build artifacts
> windows_version: not_verified
> related_checklist: `docs/checklists/windows-package-smoke.md`

## 1. Preflight Result

- Artifact preflight: `passed`
- Command actually used: `& .\scripts\check-release-artifacts.ps1`
- Note: current shell environment did not provide `pwsh`, so the PowerShell script was executed directly from Windows PowerShell.

## 2. Detected Artifacts

### NSIS

- `easyflowhub-app/src-tauri/target/release/bundle/nsis/DeskFlow_1.0.0_x64-setup.exe`
- `easyflowhub-app/src-tauri/target/release/bundle/nsis/EasyFlowHub_1.0.0_x64-setup.exe`

### MSI

- `easyflowhub-app/src-tauri/target/release/bundle/msi/DeskFlow_1.0.0_x64_en-US.msi`
- `easyflowhub-app/src-tauri/target/release/bundle/msi/EasyFlowHub_1.0.0_x64_en-US.msi`

### Portable

- `easyflowhub-app/src-tauri/target/release/EasyFlowHub.exe`

## 3. Current Assessment

- 可确认：当前存在可用于 Windows 手工 smoke 的安装器和 portable 产物。
- 未确认：安装流程、启动行为、autostart、tray、多窗口、quick note / todo card 打包态交互。
- 风险提示：当前产物目录中同时存在历史 `DeskFlow_*` 命名和当前 `EasyFlowHub_*` 命名，发版前需要明确哪些产物是正式交付物，避免手工验收或发布时误用旧命名包。

## 4. Manual Smoke Pending

以下项目尚未在本轮记录中完成：

- NSIS / MSI 安装流程
- portable EXE 启动行为
- 启动时是否弹额外终端
- manager / quick note / todo card / settings / relay 主路径
- quick note 图片、legacy asset、关闭当前窗口行为
- autostart 隐藏启动与关闭行为

## 5. Manual Smoke Findings

- `2026-04-24` 用户已反馈一轮人工验收，发现以下问题：
  - 图片当前不支持双击放大或独立预览。
  - 编辑器撤销失效。
  - quick note 含图片笔记在管理中心打开后，图片以原始 markdown 形式暴露：`![tmp268](asset:...)`。
  - 其他问题暂未发现。
- 当前记录缺口：
  - 用户本轮使用的是 `EXE / NSIS / MSI` 哪一种产物，尚未回填。
  - 修复后尚未完成第二轮人工复验。

## 6. Remediation Applied

- `2026-04-24` 已在前端补齐图片双击放大预览层，覆盖 quick note 图片缩略图与 manager markdown 预览图。
- `2026-04-24` 已修复 `useHistory` 历史快照未正确入栈导致的撤销失效问题，并补充自动化回归测试。
- `2026-04-24` 已将 manager 编辑态的图片处理改为“正文 + 附件图片”分离展示，避免打开含图笔记时显示原始 `asset:` markdown，同时保持保存内容回写兼容。
- `2026-04-24` 已修正 manager 编辑器预览模式中的图片排版，取消默认整块居中，改为与正文绕排，便于混排阅读。
- `2026-04-24` 已为 manager 编辑器补齐显式撤销/重做历史，`Ctrl+Z / Ctrl+Shift+Z` 不再依赖浏览器原生 undo，覆盖删除或移动图片 markdown 等受控编辑场景。
- `2026-04-24` 已通过增量自动化验证：
  - `bun run vitest run src/hooks/useHistory.test.tsx src/pages/QuickNotePage.test.tsx src/components/manager/NotesPanel.test.tsx`
  - `bunx tsc --noEmit`
- `2026-04-24` 已重新完成一轮 Windows 产物构建并再次通过预检：
  - `easyflowhub-app/src-tauri/target/release/EasyFlowHub.exe`
  - `easyflowhub-app/src-tauri/target/release/bundle/nsis/EasyFlowHub_1.0.0_x64-setup.exe`
  - `easyflowhub-app/src-tauri/target/release/bundle/msi/EasyFlowHub_1.0.0_x64_en-US.msi`
  - `& .\scripts\check-release-artifacts.ps1`

## 7. Release Decision

- Current verdict: `not_ready_for_signoff`
- Blocker type: `manual_windows_smoke_pending_after_fix`
- Additional risk: `artifact_naming_mixed_deskflow_easyflowhub`
