# relay manager MODULE

> status: active
> owner: EasyFlowHub frontend maintainers
> last_verified: 2026-04-07

## 职责

`src/components/manager/relay` 是管理中心中的 relay / extension 运营面板，负责展示 `scriptmgr` 暴露的 relay 配置、provider health 和 manifest 扩展清单。

## 数据流

manager panel -> `src/lib/api/scriptmgr` -> `scriptmgr serve /api/relay/*` + `/api/extensions` -> 渲染 JSON 编辑器、provider 状态和扩展列表
extension card -> `importRelayContributions` -> 更新本地 JSON 编辑器 -> 用户确认后调用 save API

## 约定 & 陷阱

- 此模块只负责管理界面，不拥有 relay 选路或 failover 逻辑，真正 owner 在 `scriptmgr-go/internal/relay`。
- 当前配置编辑是 raw JSON 模式，保存成功与否以服务端校验为准。
- `scriptmgr serve` 与 `scriptmgr relay serve` 是两个进程职责：前者提供管理接口，后者提供真实转发入口。
- 扩展 relay preset 导入只写入编辑器，不自动保存 live config；导入完成后仍需用户显式点击“保存配置”。
- 由扩展导入的 provider / route 必须保留 `source=extension:<id>` 来源标记，且新增 provider 默认禁用，避免占位上游被立刻加入选路。
