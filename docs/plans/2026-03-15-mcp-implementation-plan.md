# ScriptMgr MCP 集成 - 实施计划

> 创建日期: 2026-03-15
> 设计文档: [2026-03-15-mcp-integration-design.md](./2026-03-15-mcp-integration-design.md)
> 预计周期: 5-7 周

## 项目背景

将 ScriptMgr 封装为 MCP Server，让 AI 能高效调用脚本库，同时提供可视化 GUI 管理。

### 现有代码库

```
AllScripts/
├── scriptmgr-go/           # Go CLI (1320行 main.go)
│   ├── main.go             # 所有功能在单文件
│   └── go.mod              # module scriptmgr
├── scriptmgr-shell/        # Tauri GUI
│   ├── src-tauri/          # Rust 后端
│   └── ui/                 # 前端 (index.html, main.js, styles.css)
├── PowerShell/             # PowerShell 脚本
├── Python/                 # Python 脚本
└── docs/plans/             # 设计文档
```

### 现有 CLI 命令

| 命令 | 功能 | 状态 |
|------|------|------|
| `list` | 列出脚本 | ✅ 可用 |
| `describe <id>` | 脚本详情 | ✅ 可用 |
| `run <id>` | 执行脚本 | ✅ 可用 |
| `history` | 执行历史 | ✅ 可用 |
| `favorites` | 收藏管理 | ✅ 可用 |
| `sessions` | 会话管理 | ✅ 可用 |
| `cancel <id>` | 取消会话 | ✅ 可用 |

---

## Phase 1: 基础能力重构 (1-2 周)

**目标**: 将现有代码重构为模块化结构，添加任务状态存储和异步执行能力。

### 1.1 项目结构重构

**任务**: 将 1320 行 main.go 拆分为模块化结构

**目标结构**:
```
scriptmgr-go/
├── cmd/
│   └── scriptmgr/
│       └── main.go              # 入口点 (50行)
├── internal/
│   ├── api/
│   │   └── api.go               # Core API 层
│   ├── discovery/
│   │   ├── discovery.go         # 脚本发现
│   │   └── metadata.go          # 元数据解析
│   ├── executor/
│   │   ├── executor.go          # 执行引擎
│   │   └── async.go             # 异步执行
│   ├── store/
│   │   ├── store.go             # SQLite 存储
│   │   └── migrations.sql       # 表结构
│   ├── registry/
│   │   └── registry.go          # 类别/标签索引
│   └── config/
│       └── config.go            # 配置管理
├── go.mod
└── go.sum
```

**实施步骤**:

1. 创建目录结构
```bash
mkdir -p scriptmgr-go/cmd/scriptmgr
mkdir -p scriptmgr-go/internal/{api,discovery,executor,store,registry,config}
```

2. 提取数据类型到 `internal/model/types.go`:

说明:
- 实际实现已将共享数据结构放入 `internal/model`，而不是 `internal/api/types.go`。
- 原因是 `api / discovery / executor / store / cli` 都会复用这些类型，单独放在 `model` 可避免包循环和职责混淆。
- 后续 Phase 1/2/3 都应以 `internal/model` 为准，不再按 `internal/api/types.go` 判断结构偏离。

```go
package model

type ScriptRecord struct {
    ID          string            `json:"id"`
    Name        string            `json:"name"`
    Path        string            `json:"path"`
    ScriptType  string            `json:"script_type"`
    Description string            `json:"description,omitempty"`
    Category    string            `json:"category,omitempty"`
    Tags        []string          `json:"tags,omitempty"`
    Author      string            `json:"author,omitempty"`
    Version     string            `json:"version,omitempty"`
    Icon        string            `json:"icon,omitempty"`
    Parameters  []ScriptParameter `json:"parameters,omitempty"`
}

// ... 其他类型定义
```

3. 提取脚本发现逻辑到 `internal/discovery/discovery.go`:
```go
package discovery

import (
    "path/filepath"
    "strings"
)

type Discovery struct {
    roots []string
}

func New(roots []string) *Discovery {
    return &Discovery{roots: roots}
}

func (d *Discovery) Scan() ([]ScriptRecord, error) {
    // 从 main.go 迁移 scanScripts 逻辑
}

func (d *Discovery) GetByID(id string) (*ScriptRecord, error) {
    // 从 main.go 迁移相关逻辑
}
```

4. 提取执行逻辑到 `internal/executor/executor.go`:
```go
package executor

type Executor struct {
    store       *store.Store
    config      *config.Config
}

func New(s *store.Store, c *config.Config) *Executor {
    return &Executor{store: s, config: c}
}

func (e *Executor) Run(scriptID string, params map[string]any) (*RunResult, error) {
    // 从 main.go 迁移 runForeground 逻辑
}

func (e *Executor) RunAsync(scriptID string, params map[string]any) (string, error) {
    // 新增异步执行
}
```

5. 重构 main.go 为薄入口:
```go
package main

import (
    "scriptmgr/internal/api"
    "scriptmgr/internal/config"
    "scriptmgr/internal/discovery"
    "scriptmgr/internal/executor"
    "scriptmgr/internal/store"
)

func main() {
    cfg := config.Load()
    s := store.New(cfg.StorePath)
    d := discovery.New(cfg.Roots)
    e := executor.New(s, cfg)
    a := api.New(d, e, s)

    if err := runCLI(os.Args[1:], a); err != nil {
        fmt.Fprintln(os.Stderr, err)
        os.Exit(1)
    }
}
```

**验收标准**:
- [ ] `go build ./cmd/scriptmgr` 编译成功
- [ ] `scriptmgr list` 功能与重构前一致
- [ ] `scriptmgr run <id>` 功能与重构前一致
- [ ] 单元测试覆盖核心模块

**提交**: `refactor: modularize scriptmgr-go structure`

---

### 1.2 SQLite 任务状态存储

**任务**: 实现 SQLite 存储，持久化任务状态

**文件**: `internal/store/store.go`

```go
package store

import (
    "database/sql"
    "time"
    _ "modernc.org/sqlite"
)

type Store struct {
    db *sql.DB
}

type Task struct {
    TaskID      string     `json:"task_id"`
    ScriptID    string     `json:"script_id"`
    Status      string     `json:"status"` // pending, running, success, failed, cancelled
    CreatedAt   time.Time  `json:"created_at"`
    StartedAt   *time.Time `json:"started_at,omitempty"`
    CompletedAt *time.Time `json:"completed_at,omitempty"`
    ExitCode    *int       `json:"exit_code,omitempty"`
    DurationMs  int64      `json:"duration_ms,omitempty"`
    InputJSON   string     `json:"input_json"`
    OutputPath  string     `json:"output_path"`
    OutputSummary string   `json:"output_summary,omitempty"`
    Error       string     `json:"error,omitempty"`
}

func New(path string) (*Store, error) {
    db, err := sql.Open("sqlite", path)
    if err != nil {
        return nil, err
    }
    s := &Store{db: db}
    if err := s.migrate(); err != nil {
        return nil, err
    }
    return s, nil
}

func (s *Store) migrate() error {
    _, err := s.db.Exec(`
        CREATE TABLE IF NOT EXISTS tasks (
            task_id TEXT PRIMARY KEY,
            script_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            started_at DATETIME,
            completed_at DATETIME,
            exit_code INTEGER,
            duration_ms INTEGER,
            input_json TEXT,
            output_path TEXT,
            output_summary TEXT,
            output_full TEXT,
            error_message TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);
    `)
    return err
}

func (s *Store) CreateTask(taskID, scriptID, inputJSON string) error {
    _, err := s.db.Exec(
        `INSERT INTO tasks (task_id, script_id, status, input_json) VALUES (?, ?, 'pending', ?)`,
        taskID, scriptID, inputJSON,
    )
    return err
}

func (s *Store) UpdateStatus(taskID, status string, exitCode *int, durationMs int64) error {
    now := time.Now()
    if status == "running" {
        _, err := s.db.Exec(
            `UPDATE tasks SET status = ?, started_at = ? WHERE task_id = ?`,
            status, now, taskID,
        )
        return err
    }
    _, err := s.db.Exec(
        `UPDATE tasks SET status = ?, completed_at = ?, exit_code = ?, duration_ms = ? WHERE task_id = ?`,
        status, now, exitCode, durationMs, taskID,
    )
    return err
}

func (s *Store) GetTask(taskID string) (*Task, error) {
    row := s.db.QueryRow(
        `SELECT task_id, script_id, status, created_at, started_at, completed_at,
                exit_code, duration_ms, input_json, output_path, output_summary, error_message
         FROM tasks WHERE task_id = ?`,
        taskID,
    )
    var t Task
    err := row.Scan(&t.TaskID, &t.ScriptID, &t.Status, &t.CreatedAt, &t.StartedAt,
        &t.CompletedAt, &t.ExitCode, &t.DurationMs, &t.InputJSON, &t.OutputPath,
        &t.OutputSummary, &t.Error)
    if err != nil {
        return nil, err
    }
    return &t, nil
}

func (s *Store) ListTasks(status string, limit int) ([]Task, error) {
    query := `SELECT task_id, script_id, status, created_at, started_at, completed_at,
                     exit_code, duration_ms, input_json, output_path, output_summary, error_message
              FROM tasks`
    args := []any{}
    if status != "" {
        query += " WHERE status = ?"
        args = append(args, status)
    }
    query += " ORDER BY created_at DESC LIMIT ?"
    args = append(args, limit)

    rows, err := s.db.Query(query, args...)
    if err != nil {
        return nil, err
    }
    defer rows.Close()

    var tasks []Task
    for rows.Next() {
        var t Task
        if err := rows.Scan(&t.TaskID, &t.ScriptID, &t.Status, &t.CreatedAt, &t.StartedAt,
            &t.CompletedAt, &t.ExitCode, &t.DurationMs, &t.InputJSON, &t.OutputPath,
            &t.OutputSummary, &t.Error); err != nil {
            return nil, err
        }
        tasks = append(tasks, t)
    }
    return tasks, nil
}

func (s *Store) SetOutput(taskID, summary, outputPath string) error {
    _, err := s.db.Exec(
        `UPDATE tasks SET output_summary = ?, output_path = ? WHERE task_id = ?`,
        summary, outputPath, taskID,
    )
    return err
}

func (s *Store) Close() error {
    return s.db.Close()
}
```

**验收标准**:
- [ ] `scriptmgr run --async <id>` 创建任务记录
- [ ] `scriptmgr status <task_id>` 返回任务状态
- [ ] 任务数据持久化到 `~/.scriptmgr/data/tasks.db`

**依赖**:
```bash
go get modernc.org/sqlite
```

**提交**: `feat(store): add SQLite task storage`

---

### 1.3 异步执行框架

**任务**: 实现异步执行、状态追踪、取消能力

说明:
- 当前实现已提取到 `internal/executor/async.go`，并由 `executor.Service` 组合使用。
- 任务 ID 继续复用现有 detached session ID，这样可以保持 CLI / GUI 现有取消与会话展示兼容。
- `cancel` 当前同时接受 task/session ID；对于后台任务两者等价。

**文件**: `internal/executor/async.go`

```go
package executor

import (
    "context"
    "fmt"
    "os"
    "os/exec"
    "path/filepath"
    "sync"
    "time"

    "scriptmgr/internal/store"
)

type AsyncTask struct {
    TaskID   string
    ScriptID string
    Cmd      *exec.Cmd
    Cancel   context.CancelFunc
    Done     chan struct{}
}

type AsyncManager struct {
    store    *store.Store
    tasks    map[string]*AsyncTask
    mu       sync.RWMutex
    logDir   string
}

func NewAsyncManager(s *store.Store, logDir string) *AsyncManager {
    return &AsyncManager{
        store:  s,
        tasks:  make(map[string]*AsyncTask),
        logDir: logDir,
    }
}

func (m *AsyncManager) Start(scriptID string, params map[string]any) (string, error) {
    taskID := fmt.Sprintf("task_%d", time.Now().UnixNano())

    // 创建任务记录
    inputJSON, _ := json.Marshal(params)
    if err := m.store.CreateTask(taskID, scriptID, string(inputJSON)); err != nil {
        return "", err
    }

    // 创建日志文件
    logPath := filepath.Join(m.logDir, taskID+".log")
    logFile, err := os.Create(logPath)
    if err != nil {
        return "", err
    }

    ctx, cancel := context.WithCancel(context.Background())
    cmd := exec.CommandContext(ctx, "powershell", "-File", scriptPath) // 简化
    cmd.Stdout = logFile
    cmd.Stderr = logFile

    task := &AsyncTask{
        TaskID:   taskID,
        ScriptID: scriptID,
        Cmd:      cmd,
        Cancel:   cancel,
        Done:     make(chan struct{}),
    }

    m.mu.Lock()
    m.tasks[taskID] = task
    m.mu.Unlock()

    // 更新状态为 running
    m.store.UpdateStatus(taskID, "running", nil, 0)

    // 启动执行
    go m.runTask(task, logFile)

    return taskID, nil
}

func (m *AsyncManager) runTask(task *AsyncTask, logFile *os.File) {
    defer close(task.Done)
    defer logFile.Close()

    start := time.Now()
    err := task.Cmd.Run()
    duration := time.Since(start).Milliseconds()

    var exitCode int
    var status string
    if err != nil {
        if exitErr, ok := err.(*exec.ExitError); ok {
            exitCode = exitErr.ExitCode()
        } else {
            exitCode = -1
        }
        status = "failed"
    } else {
        exitCode = 0
        status = "success"
    }

    // 读取输出摘要
    summary := m.readSummary(logFile.Name(), 500)

    m.store.UpdateStatus(task.TaskID, status, &exitCode, duration)
    m.store.SetOutput(task.TaskID, summary, logFile.Name())

    // 从活跃任务中移除
    m.mu.Lock()
    delete(m.tasks, task.TaskID)
    m.mu.Unlock()
}

func (m *AsyncManager) readSummary(path string, maxLen int) string {
    data, err := os.ReadFile(path)
    if err != nil {
        return ""
    }
    if len(data) > maxLen {
        return string(data[:maxLen]) + "\n... (截断)"
    }
    return string(data)
}

func (m *AsyncManager) Cancel(taskID string) error {
    m.mu.RLock()
    task, ok := m.tasks[taskID]
    m.mu.RUnlock()

    if !ok {
        return fmt.Errorf("task not found or already completed")
    }

    task.Cancel()
    m.store.UpdateStatus(taskID, "cancelled", nil, 0)

    return nil
}

func (m *AsyncManager) GetStatus(taskID string) (*store.Task, error) {
    return m.store.GetTask(taskID)
}

func (m *AsyncManager) ReadLog(taskID string, offset, limit int) (string, error) {
    task, err := m.store.GetTask(taskID)
    if err != nil {
        return "", err
    }
    if task.OutputPath == "" {
        return "", fmt.Errorf("no log file")
    }

    data, err := os.ReadFile(task.OutputPath)
    if err != nil {
        return "", err
    }

    lines := strings.Split(string(data), "\n")
    if offset >= len(lines) {
        return "", nil
    }
    end := offset + limit
    if end > len(lines) {
        end = len(lines)
    }
    return strings.Join(lines[offset:end], "\n"), nil
}
```

**验收标准**:
- [ ] `scriptmgr run --async <id>` 返回 task_id
- [ ] `scriptmgr status <task_id>` 显示任务状态
- [ ] `scriptmgr cancel <task_id>` 取消运行中的任务
- [ ] `scriptmgr log <task_id> --offset 0 --limit 50` 分页读取日志

**提交**: `feat(executor): add async execution framework`

---

### 1.4 Schema 校验前置

**任务**: 在 Core API 层集成参数校验

说明:
- 当前 CLI 仍以位置参数执行脚本，因此实际实现采用“按 `parameters` 顺序校验位置参数”的策略。
- 已支持 `required / default / integer / number / boolean / string` 的前置校验。
- 这一步先保证 Core API 不会在明显错误输入下直接执行脚本；后续若引入结构化参数输入，再扩展为更完整 schema。

**文件**: `internal/validator/validator.go`

```go
package validator

import (
    "fmt"
    "reflect"
    "regexp"
    "strings"
)

type ValidationError struct {
    Field   string `json:"field"`
    Code    string `json:"code"`
    Message string `json:"message"`
    Value   any    `json:"value,omitempty"`
}

type ValidationErrors []ValidationError

func (e ValidationErrors) Error() string {
    var msgs []string
    for _, err := range e {
        msgs = append(msgs, fmt.Sprintf("%s: %s", err.Field, err.Message))
    }
    return strings.Join(msgs, "; ")
}

type ParameterSchema struct {
    Name        string   `json:"name"`
    Type        string   `json:"type"`
    Required    bool     `json:"required"`
    Default     any      `json:"default,omitempty"`
    Enum        []string `json:"enum,omitempty"`
    Min         *float64 `json:"min,omitempty"`
    Max         *float64 `json:"max,omitempty"`
    Pattern     string   `json:"pattern,omitempty"`
    Description string   `json:"description"`
}

func Validate(params []ParameterSchema, input map[string]any) (map[string]any, ValidationErrors) {
    var errors ValidationErrors
    result := make(map[string]any)

    for _, schema := range params {
        value, exists := input[schema.Name]

        // 必填检查
        if !exists || value == nil {
            if schema.Required {
                errors = append(errors, ValidationError{
                    Field:   schema.Name,
                    Code:    "required",
                    Message: fmt.Sprintf("%s 是必填字段", schema.Name),
                })
                continue
            }
            if schema.Default != nil {
                result[schema.Name] = schema.Default
            }
            continue
        }

        // 类型检查
        if err := validateType(schema, value); err != nil {
            errors = append(errors, *err)
            continue
        }

        // 枚举检查
        if len(schema.Enum) > 0 {
            if !contains(schema.Enum, fmt.Sprint(value)) {
                errors = append(errors, ValidationError{
                    Field:   schema.Name,
                    Code:    "enum",
                    Message: fmt.Sprintf("值必须是: %v", schema.Enum),
                    Value:   value,
                })
                continue
            }
        }

        // 范围检查
        if schema.Min != nil || schema.Max != nil {
            num, ok := toFloat64(value)
            if ok {
                if schema.Min != nil && num < *schema.Min {
                    errors = append(errors, ValidationError{
                        Field:   schema.Name,
                        Code:    "minimum",
                        Message: fmt.Sprintf("最小值: %v", *schema.Min),
                        Value:   value,
                    })
                    continue
                }
                if schema.Max != nil && num > *schema.Max {
                    errors = append(errors, ValidationError{
                        Field:   schema.Name,
                        Code:    "maximum",
                        Message: fmt.Sprintf("最大值: %v", *schema.Max),
                        Value:   value,
                    })
                    continue
                }
            }
        }

        // 正则检查
        if schema.Pattern != "" {
            matched, err := regexp.MatchString(schema.Pattern, fmt.Sprint(value))
            if err != nil || !matched {
                errors = append(errors, ValidationError{
                    Field:   schema.Name,
                    Code:    "pattern",
                    Message: fmt.Sprintf("格式不匹配: %s", schema.Pattern),
                    Value:   value,
                })
                continue
            }
        }

        result[schema.Name] = value
    }

    if len(errors) > 0 {
        return nil, errors
    }
    return result, nil
}

func validateType(schema ParameterSchema, value any) *ValidationError {
    expected := schema.Type
    actual := reflect.TypeOf(value).Kind()

    typeMap := map[string]reflect.Kind{
        "string":  reflect.String,
        "integer": reflect.Int,
        "number":  reflect.Float64,
        "boolean": reflect.Bool,
        "array":   reflect.Slice,
        "object":  reflect.Map,
    }

    expectedKind, ok := typeMap[expected]
    if !ok {
        return nil
    }

    // 宽松类型检查
    if expected == "integer" && (actual == reflect.Int || actual == reflect.Float64) {
        return nil
    }
    if expected == "number" && (actual == reflect.Int || actual == reflect.Float64) {
        return nil
    }

    if actual != expectedKind {
        return &ValidationError{
            Field:   schema.Name,
            Code:    "type_mismatch",
            Message: fmt.Sprintf("期望 %s，实际 %s", expected, actual),
            Value:   value,
        }
    }
    return nil
}

func contains(slice []string, item string) bool {
    for _, s := range slice {
        if s == item {
            return true
        }
    }
    return false
}

func toFloat64(v any) (float64, bool) {
    switch n := v.(type) {
    case int:
        return float64(n), true
    case int64:
        return float64(n), true
    case float64:
        return n, true
    case float32:
        return float64(n), true
    }
    return 0, false
}
```

**集成到 API 层**:
```go
func (a *API) Run(scriptID string, params map[string]any) (*RunResult, error) {
    script, err := a.discovery.GetByID(scriptID)
    if err != nil {
        return nil, err
    }

    // 参数校验
    validated, errs := validator.Validate(script.Parameters, params)
    if errs != nil {
        return nil, &ValidationErrorResponse{
            ScriptID: scriptID,
            Errors:   errs,
        }
    }

    return a.executor.Run(scriptID, validated)
}
```

**验收标准**:
- [ ] 漏传 required 字段返回清晰错误
- [ ] 类型错误返回期望值和实际值
- [ ] 校验失败不执行脚本

**提交**: `feat(validator): add JSON Schema validation`

---

### 1.5 输出截断与日志读取

**任务**: 实现输出截断 + read_log 命令

说明:
- 实际实现文件为 `internal/executor/output.go`。
- `ProcessOutput` 已统一用于前台执行结果预览与任务摘要。
- `scriptmgr log <task_id>` 已实现，并支持在任务仍处于 `running` 状态时直接读取当前日志。

**文件**: `internal/executor/output.go`

```go
package executor

const (
    MaxOutputPreview = 1000  // AI 返回的最大字符数
    MaxLogLineLength = 10000 // 单行最大长度
)

type OutputResult struct {
    Truncated   bool   `json:"truncated"`
    Preview     string `json:"preview"`
    TotalLength int    `json:"total_length"`
    LineCount   int    `json:"line_count"`
    LogPath     string `json:"log_path,omitempty"`
}

func ProcessOutput(fullOutput, logPath string) *OutputResult {
    totalLen := len(fullOutput)
    lines := strings.Count(fullOutput, "\n") + 1

    preview := fullOutput
    truncated := false
    if totalLen > MaxOutputPreview {
        preview = fullOutput[:MaxOutputPreview]
        truncated = true
    }

    return &OutputResult{
        Truncated:   truncated,
        Preview:     preview,
        TotalLength: totalLen,
        LineCount:   lines,
        LogPath:     logPath,
    }
}

func ReadLog(logPath string, offset, limit int, tail bool) (string, error) {
    data, err := os.ReadFile(logPath)
    if err != nil {
        return "", err
    }

    lines := strings.Split(string(data), "\n")

    if tail {
        // 读取最后 N 行
        if limit >= len(lines) {
            return string(data), nil
        }
        return strings.Join(lines[len(lines)-limit:], "\n"), nil
    }

    // 分页读取
    if offset >= len(lines) {
        return "", nil
    }
    end := offset + limit
    if end > len(lines) {
        end = len(lines)
    }
    return strings.Join(lines[offset:end], "\n"), nil
}
```

**新增 CLI 命令**:
```go
func runLog(args []string) error {
    var taskID string
    offset := 0
    limit := 100
    tail := false

    for i := 0; i < len(args); i++ {
        switch args[i] {
        case "--offset":
            i++
            offset, _ = strconv.Atoi(args[i])
        case "--limit":
            i++
            limit, _ = strconv.Atoi(args[i])
        case "--tail":
            tail = true
        default:
            taskID = args[i]
        }
    }

    if taskID == "" {
        return errors.New("task_id required")
    }

    task, err := store.GetTask(taskID)
    if err != nil {
        return err
    }

    output, err := executor.ReadLog(task.OutputPath, offset, limit, tail)
    if err != nil {
        return err
    }

    fmt.Println(output)
    return nil
}
```

**验收标准**:
- [ ] 超过 1000 字符的输出被截断
- [ ] `scriptmgr log <task_id>` 读取日志
- [ ] `scriptmgr log <task_id> --tail --limit 20` 读取最后 20 行
- [ ] `scriptmgr log <task_id> --offset 100 --limit 50` 分页读取

**提交**: `feat(output): add truncation and log reading`

---

## Phase 1 检查点

完成标志:
- [x] 代码模块化重构完成
- [x] SQLite 存储可用
- [x] 异步执行可用
- [ ] 参数校验可用
- [ ] 输出截断可用

验收命令:
```bash
# 编译
go build -o scriptmgr.exe ./cmd/scriptmgr

# 同步执行
./scriptmgr run demo_success --json

# 异步执行
./scriptmgr run demo_success --async --json
# 返回: {"task_id": "task_1234567890", "status": "pending"}

# 查询状态
./scriptmgr status task_1234567890 --json

# 读取日志
./scriptmgr log task_1234567890 --limit 50

# 取消任务
./scriptmgr cancel task_1234567890
```

---

## Phase 2: MCP 核心 (2-3 周)

**目标**: 实现 MCP Server 和动态工具加载

### 2.1 MCP Server 基础

**任务**: 实现 `scriptmgr mcp` 命令

**依赖**:
```bash
go get github.com/mark3labs/mcp-go
```

**文件**: `internal/mcp/server.go`

```go
package mcp

import (
    "context"
    "fmt"

    "github.com/mark3labs/mcp-go/mcp"
    "github.com/mark3labs/mcp-go/server"

    "scriptmgr/internal/api"
    "scriptmgr/internal/config"
)

type MCPServer struct {
    server *server.MCPServer
    api    *api.API
    config *config.Config
    router *ToolRouter
}

func NewMCPServer(a *api.API, cfg *config.Config) *MCPServer {
    s := server.NewMCPServer(
        "scriptmgr",
        "1.0.0",
        server.WithToolCapabilities(true),
    )

    ms := &MCPServer{
        server: s,
        api:    a,
        config: cfg,
        router: NewToolRouter(s, a, cfg),
    }

    // 注册初始工具
    ms.registerInitialTools()

    return ms
}

func (s *MCPServer) registerInitialTools() {
    // list_categories
    s.server.AddTool(mcp.Tool{
        Name:        "list_categories",
        Description: "列出所有脚本类别，返回类别名称和脚本数量",
        InputSchema: mcp.ToolInputSchema{
            Type:       "object",
            Properties: map[string]any{},
        },
    }, s.handleListCategories)

    // load_category
    s.server.AddTool(mcp.Tool{
        Name:        "load_category",
        Description: "加载指定类别的脚本工具，之后可调用该类别下的脚本",
        InputSchema: mcp.ToolInputSchema{
            Type: "object",
            Properties: map[string]any{
                "category": map[string]any{
                    "type":        "string",
                    "description": "类别名称",
                },
            },
            Required: []string{"category"},
        },
    }, s.handleLoadCategory)

    // unload_category
    s.server.AddTool(mcp.Tool{
        Name:        "unload_category",
        Description: "卸载指定类别的脚本工具，释放上下文空间",
        InputSchema: mcp.ToolInputSchema{
            Type: "object",
            Properties: map[string]any{
                "category": map[string]any{
                    "type":        "string",
                    "description": "类别名称",
                },
            },
            Required: []string{"category"},
        },
    }, s.handleUnloadCategory)

    // search_scripts
    s.server.AddTool(mcp.Tool{
        Name:        "search_scripts",
        Description: "按关键词搜索脚本，返回匹配的脚本 ID 和简要描述",
        InputSchema: mcp.ToolInputSchema{
            Type: "object",
            Properties: map[string]any{
                "query": map[string]any{
                    "type":        "string",
                    "description": "搜索关键词",
                },
            },
            Required: []string{"query"},
        },
    }, s.handleSearchScripts)

    // get_task_result
    s.server.AddTool(mcp.Tool{
        Name:        "get_task_result",
        Description: "获取异步任务的结果。设置 wait=true 会阻塞等待完成",
        InputSchema: mcp.ToolInputSchema{
            Type: "object",
            Properties: map[string]any{
                "task_id": map[string]any{
                    "type":        "string",
                    "description": "任务 ID",
                },
                "wait": map[string]any{
                    "type":        "boolean",
                    "default":     false,
                    "description": "是否等待完成",
                },
            },
            Required: []string{"task_id"},
        },
    }, s.handleGetTaskResult)

    // read_log
    s.server.AddTool(mcp.Tool{
        Name:        "read_log",
        Description: "读取任务的完整日志，支持分页",
        InputSchema: mcp.ToolInputSchema{
            Type: "object",
            Properties: map[string]any{
                "task_id": map[string]any{
                    "type":        "string",
                    "description": "任务 ID",
                },
                "offset": map[string]any{
                    "type":        "integer",
                    "default":     0,
                    "description": "起始行",
                },
                "limit": map[string]any{
                    "type":        "integer",
                    "default":     100,
                    "description": "行数限制",
                },
                "tail": map[string]any{
                    "type":        "boolean",
                    "default":     false,
                    "description": "读取最后 N 行",
                },
            },
            Required: []string{"task_id"},
        },
    }, s.handleReadLog)
}

func (s *MCPServer) Serve() error {
    return server.ServeStdio(s.server)
}

// 处理函数实现...
func (s *MCPServer) handleListCategories(ctx context.Context, args map[string]any) (*mcp.CallToolResult, error) {
    categories := s.api.ListCategories()

    var lines []string
    for _, cat := range categories {
        lines = append(lines, fmt.Sprintf("- %s (%d scripts)", cat.Name, cat.Count))
    }

    return mcp.NewToolResultText(strings.Join(lines, "\n")), nil
}

func (s *MCPServer) handleLoadCategory(ctx context.Context, args map[string]any) (*mcp.CallToolResult, error) {
    category := args["category"].(string)
    return s.router.LoadCategory(category)
}

func (s *MCPServer) handleUnloadCategory(ctx context.Context, args map[string]any) (*mcp.CallToolResult, error) {
    category := args["category"].(string)
    return s.router.UnloadCategory(category)
}

// ... 其他处理函数
```

**验收标准**:
- [ ] `scriptmgr mcp` 启动 MCP Server
- [ ] Claude Code 可连接并调用 list_categories

**提交**: `feat(mcp): add MCP server with initial tools`

---

### 2.2 动态工具注册

**任务**: 实现动态加载/卸载脚本工具

**文件**: `internal/mcp/router.go`

```go
package mcp

import (
    "fmt"
    "strings"
    "sync"

    "github.com/mark3labs/mcp-go/mcp"
    "github.com/mark3labs/mcp-go/server"

    "scriptmgr/internal/api"
    "scriptmgr/internal/config"
)

type ToolRouter struct {
    server     *server.MCPServer
    api        *api.API
    config     *config.Config
    loaded     map[string][]string // category -> script IDs
    toolCount  int
    mu         sync.RWMutex
}

func NewToolRouter(s *server.MCPServer, a *api.API, cfg *config.Config) *ToolRouter {
    return &ToolRouter{
        server:    s,
        api:       a,
        config:    cfg,
        loaded:    make(map[string][]string),
        toolCount: 6, // 初始工具数量
    }
}

func (r *ToolRouter) LoadCategory(category string) (*mcp.CallToolResult, error) {
    r.mu.Lock()
    defer r.mu.Unlock()

    // 检查是否已加载
    if _, ok := r.loaded[category]; ok {
        return mcp.NewToolResultText(fmt.Sprintf("类别 %s 已加载", category)), nil
    }

    // 获取该类别下的脚本
    scripts := r.api.GetScriptsByCategory(category)
    if len(scripts) == 0 {
        return mcp.NewToolResultError(fmt.Sprintf("类别 %s 不存在或无脚本", category)), nil
    }

    // 检查工具数量限制
    maxTools := r.config.MCP.MaxLoadedTools
    unloaded := []string{}
    unloadedCategories := []string{}

    for r.toolCount+len(scripts) > maxTools {
        // LRU 卸载最旧的类别
        var oldestCat string
        for cat := range r.loaded {
            oldestCat = cat
            break
        }
        if oldestCat == "" {
            break
        }
        for _, scriptID := range r.loaded[oldestCat] {
            r.server.RemoveTool("script_" + sanitizeID(scriptID))
            r.toolCount--
            unloaded = append(unloaded, scriptID)
        }
        delete(r.loaded, oldestCat)
        unloadedCategories = append(unloadedCategories, oldestCat)
    }

    // 注册新工具
    loaded := []string{}
    for _, script := range scripts {
        toolName := "script_" + sanitizeID(script.ID)

        desc := script.Description
        if len(desc) > 200 {
            desc = desc[:200] + "..."
        }
        desc += "\n\n【异步模式】设置 async=true 时立即返回 task_id，请使用 get_task_result(task_id, wait=true) 等待结果。"

        r.server.AddTool(mcp.Tool{
            Name:        toolName,
            Description: desc,
            InputSchema: r.buildInputSchema(script),
        }, r.makeScriptHandler(script))

        loaded = append(loaded, script.ID)
        r.toolCount++
    }

    r.loaded[category] = loaded

    // 构建返回消息
    msg := fmt.Sprintf("已加载 %s (%d 个工具)", category, len(loaded))
    if len(unloaded) > 0 {
        msg += fmt.Sprintf("\n已卸载 %s (%d 个工具，LRU)", strings.Join(unloadedCategories, ", "), len(unloaded))
    }

    return mcp.NewToolResultText(msg), nil
}

func (r *ToolRouter) UnloadCategory(category string) (*mcp.CallToolResult, error) {
    r.mu.Lock()
    defer r.mu.Unlock()

    scriptIDs, ok := r.loaded[category]
    if !ok {
        return mcp.NewToolResultText(fmt.Sprintf("类别 %s 未加载", category)), nil
    }

    for _, scriptID := range scriptIDs {
        r.server.RemoveTool("script_" + sanitizeID(scriptID))
        r.toolCount--
    }
    delete(r.loaded, category)

    return mcp.NewToolResultText(fmt.Sprintf("已卸载 %s (%d 个工具)", category, len(scriptIDs))), nil
}

func (r *ToolRouter) makeScriptHandler(script api.ScriptRecord) func(ctx context.Context, args map[string]any) (*mcp.CallToolResult, error) {
    return func(ctx context.Context, args map[string]any) (*mcp.CallToolResult, error) {
        // 提取 async 参数
        async := false
        if v, ok := args["async"]; ok {
            async = v.(bool)
        }
        delete(args, "async")

        if async {
            taskID, err := r.api.RunAsync(script.ID, args)
            if err != nil {
                return mcp.NewToolResultError(err.Error()), nil
            }
            return mcp.NewToolResultText(fmt.Sprintf(
                "任务已提交: %s\n请使用 get_task_result(\"%s\", wait=true) 获取结果",
                taskID, taskID,
            )), nil
        }

        // 同步执行
        result, err := r.api.Run(script.ID, args)
        if err != nil {
            return mcp.NewToolResultError(err.Error()), nil
        }

        return mcp.NewToolResultText(formatResult(result)), nil
    }
}

func (r *ToolRouter) buildInputSchema(script api.ScriptRecord) mcp.ToolInputSchema {
    props := make(map[string]any)
    required := []string{}

    for _, p := range script.Parameters {
        prop := map[string]any{
            "type":        p.Type,
            "description": p.Description,
        }
        if p.Default != nil {
            prop["default"] = p.Default
        }
        if len(p.Enum) > 0 {
            prop["enum"] = p.Enum
        }
        props[p.Name] = prop

        if p.Required {
            required = append(required, p.Name)
        }
    }

    // 添加 async 参数
    props["async"] = map[string]any{
        "type":        "boolean",
        "default":     false,
        "description": "异步执行，不阻塞等待结果",
    }

    return mcp.ToolInputSchema{
        Type:       "object",
        Properties: props,
        Required:   required,
    }
}

func sanitizeID(id string) string {
    return strings.ReplaceAll(id, "-", "_")
}

func formatResult(r *api.RunResult) string {
    var sb strings.Builder
    sb.WriteString(fmt.Sprintf("状态: %s\n", r.Status))
    sb.WriteString(fmt.Sprintf("耗时: %dms\n", r.DurationMs))
    if r.Output != nil {
        sb.WriteString(fmt.Sprintf("输出:\n%s", r.Output.Preview))
        if r.Output.Truncated {
            sb.WriteString(fmt.Sprintf("\n... (共 %d 字符，使用 read_log 读取完整日志)", r.Output.TotalLength))
        }
    }
    return sb.String()
}
```

**验收标准**:
- [ ] `load_category` 后 AI 可调用脚本工具
- [ ] `unload_category` 移除工具
- [ ] 超过 `max_loaded_tools` 时 LRU 自动卸载

**提交**: `feat(mcp): add dynamic tool loading with LRU unloading`

---

### 2.3 HTTP API 服务

**任务**: 为 Tauri 前端提供 HTTP API

**文件**: `internal/http/server.go`

```go
package http

import (
    "encoding/json"
    "net/http"

    "scriptmgr/internal/api"
)

type HTTPServer struct {
    api    *api.API
    server *http.Server
}

func NewHTTPServer(a *api.API, port int) *HTTPServer {
    s := &HTTPServer{
        api: a,
        server: &http.Server{
            Addr: fmt.Sprintf("127.0.0.1:%d", port),
        },
    }

    mux := http.NewServeMux()
    mux.HandleFunc("/api/scripts", s.handleScripts)
    mux.HandleFunc("/api/scripts/", s.handleScriptDetail)
    mux.HandleFunc("/api/tasks", s.handleTasks)
    mux.HandleFunc("/api/tasks/", s.handleTaskDetail)
    mux.HandleFunc("/api/mcp/config", s.handleMCPConfig)

    s.server.Handler = mux
    return s
}

func (s *HTTPServer) Start() error {
    return s.server.ListenAndServe()
}

func (s *HTTPServer) handleScripts(w http.ResponseWriter, r *http.Request) {
    switch r.Method {
    case "GET":
        scripts := s.api.ListScripts()
        json.NewEncoder(w).Encode(scripts)
    }
}

func (s *HTTPServer) handleScriptDetail(w http.ResponseWriter, r *http.Request) {
    id := strings.TrimPrefix(r.URL.Path, "/api/scripts/")

    switch r.Method {
    case "GET":
        script, err := s.api.GetScript(id)
        if err != nil {
            http.Error(w, err.Error(), 404)
            return
        }
        json.NewEncoder(w).Encode(script)
    case "PUT":
        // 更新元数据
        var updates map[string]any
        json.NewDecoder(r.Body).Decode(&updates)
        err := s.api.UpdateScriptMetadata(id, updates)
        if err != nil {
            http.Error(w, err.Error(), 400)
            return
        }
        w.WriteHeader(200)
    }
}

func (s *HTTPServer) handleTasks(w http.ResponseWriter, r *http.Request) {
    status := r.URL.Query().Get("status")
    tasks := s.api.ListTasks(status, 100)
    json.NewEncoder(w).Encode(tasks)
}

func (s *HTTPServer) handleTaskDetail(w http.ResponseWriter, r *http.Request) {
    taskID := strings.TrimPrefix(r.URL.Path, "/api/tasks/")

    switch r.Method {
    case "GET":
        task, err := s.api.GetTask(taskID)
        if err != nil {
            http.Error(w, err.Error(), 404)
            return
        }
        json.NewEncoder(w).Encode(task)
    case "POST":
        // 取消任务
        err := s.api.CancelTask(taskID)
        if err != nil {
            http.Error(w, err.Error(), 400)
            return
        }
        w.WriteHeader(200)
    }
}

func (s *HTTPServer) handleMCPConfig(w http.ResponseWriter, r *http.Request) {
    switch r.Method {
    case "GET":
        cfg := s.api.GetMCPConfig()
        json.NewEncoder(w).Encode(cfg)
    case "PUT":
        var cfg config.MCPConfig
        json.NewDecoder(r.Body).Decode(&cfg)
        s.api.UpdateMCPConfig(&cfg)
        w.WriteHeader(200)
    }
}
```

**验收标准**:
- [ ] `GET /api/scripts` 返回脚本列表
- [ ] `GET /api/tasks` 返回任务列表
- [ ] `PUT /api/mcp/config` 更新配置

**提交**: `feat(http): add HTTP API for Tauri frontend`

---

### 2.4 进程生命周期管理

**任务**: 实现 PID/Port 文件管理

**文件**: `internal/lifecycle/lifecycle.go`

```go
package lifecycle

import (
    "fmt"
    "os"
    "path/filepath"
    "strconv"
    "syscall"
)

const (
    RunDir = ".scriptmgr/run"
)

type Lifecycle struct {
    runDir string
}

func New() *Lifecycle {
    homeDir, _ := os.UserHomeDir()
    return &Lifecycle{
        runDir: filepath.Join(homeDir, RunDir),
    }
}

func (l *Lifecycle) AcquireLock() error {
    if err := os.MkdirAll(l.runDir, 0755); err != nil {
        return err
    }

    lockFile := filepath.Join(l.runDir, "scriptmgr.lock")
    f, err := os.OpenFile(lockFile, os.O_CREATE|os.O_EXCL, 0644)
    if err != nil {
        return fmt.Errorf("另一个实例正在启动")
    }
    f.Close()
    return nil
}

func (l *Lifecycle) ReleaseLock() {
    lockFile := filepath.Join(l.runDir, "scriptmgr.lock")
    os.Remove(lockFile)
}

func (l *Lifecycle) CheckRunning() (int, error) {
    pidFile := filepath.Join(l.runDir, "scriptmgr.pid")
    data, err := os.ReadFile(pidFile)
    if err != nil {
        return 0, nil // 没有运行
    }

    pid, _ := strconv.Atoi(string(data))

    // 检查进程是否存在
    process, err := os.FindProcess(pid)
    if err != nil {
        return 0, nil
    }

    err = process.Signal(syscall.Signal(0))
    if err != nil {
        // 进程不存在，清理残留
        l.Cleanup()
        return 0, nil
    }

    return pid, nil
}

func (l *Lifecycle) WritePID(pid int) error {
    pidFile := filepath.Join(l.runDir, "scriptmgr.pid")
    return os.WriteFile(pidFile, []byte(strconv.Itoa(pid)), 0644)
}

func (l *Lifecycle) WritePort(port int) error {
    portFile := filepath.Join(l.runDir, "scriptmgr.port")
    return os.WriteFile(portFile, []byte(strconv.Itoa(port)), 0644)
}

func (l *Lifecycle) ReadPort() (int, error) {
    portFile := filepath.Join(l.runDir, "scriptmgr.port")
    data, err := os.ReadFile(portFile)
    if err != nil {
        return 0, err
    }
    return strconv.Atoi(string(data))
}

func (l *Lifecycle) Cleanup() {
    pidFile := filepath.Join(l.runDir, "scriptmgr.pid")
    portFile := filepath.Join(l.runDir, "scriptmgr.port")
    os.Remove(pidFile)
    os.Remove(portFile)
}
```

**集成到 serve 命令**:
```go
func runServe(args []string) error {
    lc := lifecycle.New()

    // 检查锁
    if err := lc.AcquireLock(); err != nil {
        return err
    }
    defer lc.ReleaseLock()

    // 检查是否已运行
    if pid, _ := lc.CheckRunning(); pid > 0 {
        return fmt.Errorf("服务已在运行 (PID: %d)", pid)
    }

    // 写入 PID
    lc.WritePID(os.Getpid())

    // 启动服务...
}
```

**验收标准**:
- [ ] 重复启动返回错误
- [ ] 僵尸进程自动清理
- [ ] Tauri 可读取 port 文件连接服务

**提交**: `feat(lifecycle): add PID/Port file management`

---

## Phase 2 检查点

完成标志:
- [ ] MCP Server 可启动
- [ ] 动态工具加载/卸载可用
- [ ] HTTP API 可用
- [ ] 进程管理可用

验收命令:
```bash
# 启动 MCP Server
./scriptmgr serve

# 在另一个终端测试
curl http://127.0.0.1:9527/api/scripts
```

Claude Code 配置:
```json
{
  "mcpServers": {
    "scriptmgr": {
      "command": "E:/Github/AllScripts/scriptmgr-go/scriptmgr.exe",
      "args": ["mcp"]
    }
  }
}
```

---

## Phase 3: GUI 完善 (2 周)

**目标**: 扩展 Tauri shell 支持管理功能

### 3.1 前端连接 HTTP API

**任务**: Tauri 前端直连 Go HTTP API

**文件**: `ui/api.js`

```javascript
const API_BASE = 'http://127.0.0.1:9527/api';

async function fetchAPI(endpoint, options = {}) {
    const response = await fetch(`${API_BASE}${endpoint}`, {
        headers: { 'Content-Type': 'application/json' },
        ...options
    });
    if (!response.ok) throw new Error(response.statusText);
    return response.json();
}

export const api = {
    // 脚本
    listScripts: () => fetchAPI('/scripts'),
    getScript: (id) => fetchAPI(`/scripts/${id}`),
    updateScript: (id, data) => fetchAPI(`/scripts/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    }),

    // 任务
    listTasks: (status) => fetchAPI(`/tasks?status=${status}`),
    getTask: (id) => fetchAPI(`/tasks/${id}`),
    cancelTask: (id) => fetchAPI(`/tasks/${id}`, { method: 'POST' }),

    // 配置
    getMCPConfig: () => fetchAPI('/mcp/config'),
    updateMCPConfig: (data) => fetchAPI('/mcp/config', {
        method: 'PUT',
        body: JSON.stringify(data)
    })
};
```

### 3.2 脚本管理页面

**文件**: `ui/pages/scripts.html`

```html
<div class="page" id="scripts-page">
    <div class="toolbar">
        <select id="category-filter">
            <option value="">全部类别</option>
        </select>
        <input type="text" id="search" placeholder="搜索脚本...">
    </div>

    <div class="split-view">
        <div class="script-list" id="script-list"></div>
        <div class="script-detail" id="script-detail">
            <h2 id="script-name"></h2>
            <p id="script-desc"></p>
            <div id="script-params"></div>
            <label>
                <input type="checkbox" id="mcp-exposed">
                允许 AI 调用
            </label>
            <div class="actions">
                <button id="btn-edit">编辑元数据</button>
                <button id="btn-run">立即执行</button>
            </div>
        </div>
    </div>
</div>
```

### 3.3 MCP 配置页面

**文件**: `ui/pages/mcp-config.html`

```html
<div class="page" id="mcp-config-page">
    <div class="status-bar">
        <span id="mcp-status">● 运行中</span>
        <span id="mcp-port">端口: 9527</span>
        <button id="btn-restart">重启</button>
        <button id="btn-stop">停止</button>
    </div>

    <section>
        <h3>动态加载规则</h3>
        <div class="checkbox-group">
            <label><input type="checkbox" name="initial" value="list_categories" checked> list_categories</label>
            <label><input type="checkbox" name="initial" value="load_category" checked> load_category</label>
            <label><input type="checkbox" name="initial" value="search_scripts" checked> search_scripts</label>
        </div>
    </section>

    <section>
        <h3>默认加载类别</h3>
        <div id="category-checkboxes"></div>
    </section>

    <section>
        <h3>单个脚本覆盖</h3>
        <table id="script-override-table">
            <thead>
                <tr><th>脚本</th><th>MCP 暴露</th><th>备注</th></tr>
            </thead>
            <tbody></tbody>
        </table>
    </section>
</div>
```

### 3.4 执行监控页面

**文件**: `ui/pages/monitor.html`

```html
<div class="page" id="monitor-page">
    <div class="toolbar">
        <h3>运行中任务 <span id="running-count">(0)</span></h3>
        <button id="btn-cancel-all">全部取消</button>
    </div>

    <div id="running-tasks" class="task-list"></div>

    <h3>最近完成</h3>
    <div id="completed-tasks" class="task-list"></div>
</div>
```

**文件**: `ui/monitor.js`

```javascript
import { api } from './api.js';

async function refreshTasks() {
    const running = await api.listTasks('running');
    const completed = await api.listTasks('success,failed,cancelled');

    renderTasks('running-tasks', running);
    renderTasks('completed-tasks', completed.slice(0, 20));
}

function renderTasks(containerId, tasks) {
    const container = document.getElementById(containerId);
    container.innerHTML = tasks.map(t => `
        <div class="task-item ${t.status}">
            <span class="task-id">${t.task_id}</span>
            <span class="task-script">${t.script_id}</span>
            <span class="task-status">${t.status}</span>
            <span class="task-time">${formatDuration(t.duration_ms)}</span>
            <button onclick="showLog('${t.task_id}')">日志</button>
            ${t.status === 'running' ? `<button onclick="cancelTask('${t.task_id}')">取消</button>` : ''}
        </div>
    `).join('');
}

// 定时刷新
setInterval(refreshTasks, 2000);
refreshTasks();
```

---

## Phase 3 检查点

完成标志:
- [ ] GUI 可查看脚本列表和详情
- [ ] GUI 可配置 MCP 暴露规则
- [ ] GUI 可查看任务状态和历史

---

## 总结

### 开发周期

| Phase | 周期 | 核心产出 |
|-------|------|----------|
| Phase 1 | 1-2 周 | 模块化代码 + SQLite + 异步执行 + 校验 |
| Phase 2 | 2-3 周 | MCP Server + 动态加载 + HTTP API |
| Phase 3 | 2 周 | Tauri GUI 扩展 |

**总计**: 5-7 周

### 提交策略

每个子任务完成后立即提交，保持原子性：
```
refactor: modularize scriptmgr-go structure
feat(store): add SQLite task storage
feat(executor): add async execution framework
feat(validator): add JSON Schema validation
feat(output): add truncation and log reading
feat(mcp): add MCP server with initial tools
feat(mcp): add dynamic tool loading with LRU unloading
feat(http): add HTTP API for Tauri frontend
feat(lifecycle): add PID/Port file management
feat(gui): add script management page
feat(gui): add MCP config page
feat(gui): add execution monitor page
```

### 测试策略

1. **单元测试**: Core API、Validator、Store 模块
2. **集成测试**: MCP 工具调用流程
3. **手动测试**: CLI 命令、GUI 页面、AI 调用

---

*计划创建: 2026-03-15*
