# ADR-0005：v0.2 下载队列归属与文件安全边界

## 状态

已接受（2026-10-17）

## 背景

v0.2 需要把 URL 下载纳入个人效率 Hub，至少覆盖队列、进度、暂停/继续、取消、失败重试和文件冲突反馈。现有 ScriptMgr 负责脚本发现与执行，不拥有普通文件下载，也没有下载目标目录和文件冲突的桌面语义。把下载放进 Go sidecar 会让网络任务、桌面文件路径和 Manager 状态跨越额外边界。

## 决策

- 下载器由 `easyflowhub-app/src-tauri/src/download.rs` 负责，沿用现有 Tauri native 能力和 Windows 文件系统边界。
- 前端通过 `src/lib/tauri/download.ts` 使用 typed bridge；Manager 面板不散落命令名。
- 队列记录在 v0.2 MVP 中保存在 native 进程内，窗口重开仍可查看当前进程任务；目标目录和冲突策略写入应用配置。应用完全退出时，不承诺恢复未完成任务，`.part` 文件保留并在下次添加同 URL 时可按 HTTP Range 尝试续传。
- 下载先写入同目录的随机临时 `.part` 文件，成功后原子重命名为最终文件名；默认 `rename` 冲突策略避免普通下载因同名文件直接失败，用户可明确选择 `fail` 或 `overwrite`，覆盖策略在提交下载前要求再次确认。
- Reqwest 使用带默认 Rustls crypto provider 的 `rustls` feature；禁止使用未安装 provider 的 `rustls-no-provider` 配置，避免 native 下载 worker 在创建 HTTP client 时运行时 panic。
- URL 只接受 `http` / `https`；响应状态、重定向失败、无效文件名、权限不足、网络中断和空间不足都要映射为可读错误。
- 续传只有在响应为 `206 Partial Content` 且 `Content-Range` 起点等于现有 `.part` 长度时才追加；否则从零开始下载，避免把不同内容拼接到一起。
- 自定义文件名会拒绝 Windows 保留设备名和无效值；下载状态写盘失败时，内存状态回滚到上一次成功保存的值。
- 暂停、继续、取消、重试通过 native 控制标志实现；UI 通过状态查询读取进度，避免把网络流直接暴露到 React。
- 完成后的打开、定位和复制路径沿用现有 opener / clipboard 入口；native 只返回经过目标目录校验的路径，Manager 通过 typed native clipboard bridge 写入系统剪贴板；加入 Inbox 通过现有 `capture_inbox_item` 形成文件附件，不新增第二套附件存储。
- 完成后的路径动作会重新校验 canonical 路径位于该任务目标目录内且仍为普通文件；`DownloadSettings` 对外字段统一为 `targetDir`，同时兼容读取早期 `directory` 配置。

## 不在本切片内

- 后台常驻服务、系统级下载接管、BT/P2P、多线程分片和全局浏览器扩展。
- 应用退出后的自动恢复队列和云端同步。
- 把下载任务注册成 ScriptMgr 脚本任务；两者仍由各自 owner 管理。

## 验收

- 自动化覆盖 URL 校验、URL / RFC 5987 文件名解析、冲突策略基础逻辑、暂停/继续/取消控制标志、成功写入 `.part` 以及取消时保留 `.part` 的流式下载边界；取消、失败重试、冲突确认和权限路径仍需 Windows 打包 smoke。
- Manager 覆盖添加 URL、目标目录、状态轮询、错误反馈和完成后的打开/定位/复制路径、加入 Inbox；覆盖策略必须经过明确确认。
- Windows 打包 smoke 覆盖公开小文件成功下载、取消、网络失败重试、同名冲突和权限失败；应用退出恢复列为后续验证债务。
