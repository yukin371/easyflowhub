# scripts manager MODULE

> status: active
> owner: EasyFlowHub frontend maintainers
> last_verified: 2026-10-19

## 职责

`src/components/manager/scripts` 是管理中心里的脚本工作台：分类浏览、搜索、脚本详情、源码查看与编辑、手动执行与结果查看、新建 / 导入脚本，以及运行记录视图。脚本文件的读写与回收站由 `scriptmgr-go` 拥有，这里只做界面与调用编排。

## 数据流

`ScriptsPanel` -> `ScriptStoreProvider`（列表 / 分类 / 选中详情）
脚本文件操作 -> `lib/api/scriptmgr`（`scriptsApi` / `rootsApi` / `scriptTrashApi`）-> `scriptmgr serve` HTTP API -> 文件系统
执行 -> `tasksApi.run`（同步、捕获输出）-> 工作台展示退出码与输出
运行记录 -> `TasksPanel`（同一页面内的 tab），按可见性与是否有运行中任务调整轮询

## 约定 & 陷阱

- 所有文件写操作必须经 Go 侧接口：目标路径必须在已配置 root 内、扩展名限定 `.ps1/.py/.bat/.cmd`、源码上限 2 MB。前端不得自行拼接或写入路径。
- 存储位置分两类：`custom`（roots.json 固定，重启后仍有效）与自动发现（依赖进程工作目录）。UI 必须允许把自动发现目录「固定」，否则换启动方式后脚本会消失。
- 删除脚本是「移入回收站」，不是直接删文件；恢复回到原路径，同名冲突时报错而不是覆盖。
- 源码保存使用 native 侧的临时文件 + rename 原子替换，UI 需要区分「未保存 / 已保存」状态。
- 执行结果来自同步 `POST /api/run`（`CaptureOutput`），只展示 `exit_code` / `status` / `output`；不要在组件里推断成功与否以外的执行状态。
- 运行记录轮询：窗口隐藏时不轮询；有 running/pending 任务时 3 秒，空闲时 15 秒。新增轮询逻辑必须保持这两条约束。
- 执行记录不是独立侧栏模块，它是脚本页的一个视图；不要重新注册 `tasks` 模块。
