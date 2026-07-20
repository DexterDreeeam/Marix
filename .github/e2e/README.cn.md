# Marix 确定性 E2E 任务

## 范围

`tasks.json` 定义六项可重复任务：两项联网搜索、两项隔离的本地代码任务和两项图片理解探测。每项任务在 `.github\e2e\workspaces` 下使用唯一目录。联网任务禁止读取 `.credential` 文件。图片任务有意记录当前预期的视觉能力缺口。

Schema 版本 2 为每个 case 新增两个 nullable TaskRequest guardrail：

- `max_completion_time_secs`：任务最长完成时间；
- `max_relay_count`：最大 relay 次数。

两个字段的 schema 默认值都是 `null`，表示 `None`、无限制。值非空时，Server 会把小于 10 秒的完成时间钳制为 10 秒，把小于 5 的 relay 次数钳制为 5。Client 与 smoke harness 会原样传递配置值，不重复执行 Server 钳制。当前六个 case 使用明确且合理的 smoke 预算，不使用故意设置的极小值，也不包含专门的 guardrail 负向 case。

建议任务耗时和 `suggested_outer_watchdog_minutes` 仅是元数据。外层 watchdog 只能作为 Agent 或 harness 进程失控时的安全兜底；它既不能替代两个 TaskRequest guardrail，也不能决定 case 是否成功。

## 前置条件

- 在 PowerShell 中从 `C:\r\Marix` 运行。
- 确认 Marix Server Telemetry、Server 和 Host 已就绪，且 Client 配置有效。
- 只有 `code-edit-rust-slugify` 需要安装 Rust。
- 不要把凭据放进任务工作目录，也不要在 prompt 或结果中包含凭据。

## 准备 fixture

选择一项任务，按顺序运行 `tasks.json` 中的 `setup.commands`。Setup 只会删除该任务自己的工作目录，然后创建或复制全新的 fixture。不得并发运行具有同一任务 ID 的两个实例。

示例：

```powershell
Set-Location C:\r\Marix
$task = (Get-Content .github\e2e\tasks.json -Raw | ConvertFrom-Json).tasks |
  Where-Object id -eq 'code-edit-rust-slugify'
$task.setup.commands | ForEach-Object { Invoke-Expression $_ }
```

不要直接在 `.github\e2e\fixtures` 上运行代码任务；fixture 是不可变基线。图片 setup 复制 `scene.png`，联网 setup 创建空工作目录。

## 人工运行

把所选任务的 prompt 和两个已配置的 guardrail 传给 oneshot Client：

```powershell
$cli = 'C:\MarixClient\Cli\marix-client-cli.exe'
$arguments = @('--oneshot', $task.prompt)
if ($null -ne $task.max_completion_time_secs) {
  $arguments += @('--max-completion-time-secs', "$($task.max_completion_time_secs)")
}
if ($null -ne $task.max_relay_count) {
  $arguments += @('--max-relay-count', "$($task.max_relay_count)")
}
& $cli @arguments
if ($LASTEXITCODE -ne 0) { throw "oneshot failed: $LASTEXITCODE" }
```

这是唯一受支持的已部署 CLI 路径。不得调用或回退到
`C:\MarixClient\marix-client-cli.exe`。

prompt 必须紧跟在 `--oneshot` 后，并放在可选 flags 之前。CLI flags 是
`--max-completion-time-secs` 和 `--max-relay-count`。字段为 `null` 时省略相应 flag，即可将对应 TaskRequest 值保持为 `None`。oneshot Client 会等待任务终态。无人值守运行时，harness 只能把 `suggested_outer_watchdog_minutes` 用作进程失控兜底；watchdog 触发应报告为环境错误，并与任务失败和断言失败分开。

## 使用 Smoke Agent

选择 `tester-of-e2e-smoke` 自定义 agent，并要求它运行全部确定性 E2E smoke case，也可以指定 case 子集。该 agent 会读取 `tasks.json`，严格按数组顺序串行执行，并为每个 case 完成 setup、oneshot 提交、等待终态、验证 criteria、收集结果和 cleanup。它不会部署或启动服务、修改 fixture 基线、运行 git、读取 `.credential`，也不会并发运行 case。

不要要求 Smoke Agent 测试 guardrail gate 本身；它的目标仅是判断每个 smoke case 是否达到声明的预期结果。

## 判定结果

应用 `success_criteria` 中的每项条件，并在出现任一 `failure_criteria` 时判为失败：

1. 使用 `Get-Content <path> -Raw | ConvertFrom-Json` 解析 JSON 产物。
2. 按 JSON 语义比较，不比较空白或属性顺序。
3. 对 `sha256_unchanged` 使用 `Get-FileHash -Algorithm SHA256`。
4. 对 Rust 任务，在其工作目录运行 `cargo test --quiet` 并要求退出码为 0。
5. 相对复制后的 fixture 检查只有 `allowed_changed_paths` 发生变化。
6. 对联网任务，验证 URL 主机分组、日期、来源一致性和实时访问证据。网络中断属于环境失败。
7. 只有具备图片能力的工具确实检查 PNG 后，图片任务才能通过。当前预期结果是报告能力缺口，而不是编造答案。

预期值有意保存在 `tasks.json` 中供 harness 使用。图片 prompt 要求 agent 不读取该文件；条件允许时应隔离 prompt 执行器与判定器。

## 结果分类

每个 case 都要报告耗时、配置的最大时间和 relay 次数、终态摘要、断言结果、cleanup 结果，以及一个主要状态：

- `PASS`：任务成功终止，且全部 criteria 通过。
- `FAIL`：任务失败或断言失败；报告中会区分这两类子类型。
- `UNSUPPORTED`：case 的 `current_support` 规则允许的必需能力不可用。图片能力缺失在此报告，绝不能伪造成成功。
- `ENVIRONMENT_ERROR`：setup、前置条件、Client/传输、服务可用性、网络基础设施、harness 或 cleanup 问题导致无法可靠判定。

最终报告汇总各状态数量和总耗时。环境失败、任务失败和断言失败始终分别标识。

## 隔离与清理

只允许在任务声明的 `working_directory` 中运行。禁止访问仓库 `src\`、`overview\`、`.git\` 或凭据文件。需要强制检查修改路径时，应在执行前快照全新的工作目录。

收集日志和产物后，运行所选任务的 `cleanup` 命令。Cleanup 只删除其唯一工作目录。所有任务均未运行时，可用以下命令重置全部测试状态：

```powershell
Remove-Item -Recurse -Force .github\e2e\workspaces -ErrorAction SilentlyContinue
```

每次运行之间保持 fixture 不变。
