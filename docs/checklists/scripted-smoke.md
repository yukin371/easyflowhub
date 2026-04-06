# EasyFlowHub Scripted Smoke

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-04-07

## Purpose

这条 smoke 路径用于把仓库默认验证链收敛成一个可重复执行的入口，优先覆盖：

- 前端测试
- TypeScript 类型检查
- Tauri / Rust 编译检查
- Go 后端测试

它不是 Windows 打包态行为验证的替代品；打包相关仍然使用 `windows-package-smoke.md`。

## Command

```powershell
pwsh -File .\scripts\run-smoke.ps1
```

如果已经完成本地打包，也可以连同产物预检一起执行：

```powershell
pwsh -File .\scripts\run-smoke.ps1 -CheckReleaseArtifacts
```

## Current Coverage

- `cd easyflowhub-app && bun run test`
- `cd easyflowhub-app && bunx tsc --noEmit`
- `cd easyflowhub-app && cargo check --manifest-path src-tauri/Cargo.toml`
- `cd scriptmgr-go && go test ./...`
- 可选：`scripts/check-release-artifacts.ps1`

## Not Covered

- Windows 安装器真实安装流程
- autostart 打包态行为
- quick note / todo card / tray 的人工桌面交互
- 真正的浏览器或桌面 E2E

这些仍需要：

1. `docs/checklists/windows-package-smoke.md`
2. 后续的 Playwright / 桌面验收流
