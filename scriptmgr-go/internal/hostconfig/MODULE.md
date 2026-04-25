# internal/hostconfig MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-25

## 职责

`internal/hostconfig` 负责 Claude / Codex 宿主配置的读取、状态检测、备份和原子写入。

## 数据流

CLI `scriptmgr relay hosts *` -> `internal/hostconfig` -> `~/.claude/settings.json` / `~/.codex/config.toml`

## 约定 & 陷阱

- 该模块只管理宿主配置文件，不拥有 relay provider 选路、failover 或任何代理转发逻辑。
- Claude 与 Codex 的上游协议不同：Claude 侧只写 Anthropic 风格环境配置，Codex 侧只写 OpenAI 风格 provider `base_url`。
- 对宿主配置的写入必须先备份，再原子替换，避免把本地 AI 工作流配置写坏。
- 若后续要支持更多宿主，继续在本模块扩展，不要把配置文件编辑逻辑散到 `internal/cli`、`internal/relay` 或前端。
