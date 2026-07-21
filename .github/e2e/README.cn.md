# Marix 确定性 E2E 任务

## 范围

`tasks.json` 定义六项可重复任务：两项联网搜索、两项隔离的本地代码任务和两项图片理解探测。现在每项任务都在 VM 内的 `C:\MarixE2E\workspaces\<case-id>` 使用唯一工作目录。联网任务禁止读取 `.credential` 文件。图片任务有意记录当前预期的视觉能力缺口。

Schema 版本 3 保留 `repository_root`，但它只用于定位物理机上的不可变 fixture 源 `.github\e2e\fixtures\...`；同时新增面向 VM 的字段：

- `vm_name`：承载 Marix Host 工具执行的固定 Hyper-V 来宾；
- `vm_workspace_root`：固定的来宾侧 E2E 工作区根目录；
- `vm_working_directory`：每个 case 在来宾内使用的绝对工作目录。

两个 nullable TaskRequest guardrail 保持不变：

- `max_completion_time_secs`：任务最长完成时间；
- `max_relay_count`：最大 relay 次数。

两个字段的 schema 默认值都是 `null`，表示 `None`、无限制。值非空时，Server 会把小于 10 秒的完成时间钳制为 10 秒，把小于 5 的 relay 次数钳制为 5。Client 与 smoke harness 会原样传递配置值，不重复执行 Server 钳制。当前六个 case 使用明确且合理的 smoke 预算，不使用故意设置的极小值，也不包含专门的 guardrail 负向 case。

建议任务耗时和 `suggested_outer_watchdog_minutes` 仅是元数据。外层 watchdog 只能作为 Agent 或 harness 进程失控时的安全兜底；它既不能替代两个 TaskRequest guardrail，也不能决定 case 是否成功。

## VM 托管执行模型

Marix 的原生工具（如 `write_file`、`web_search`、`cargo test`）只会经由 Host 执行，而 Host 只部署在 Hyper-V VM `Marix_TestVm` 内。物理机仍然运行已部署的 Client CLI：`C:\MarixClient\Cli\marix-client-cli.exe`；但所有任务工作区操作、产物读取、哈希校验和验证命令都必须通过 PowerShell Direct 指向来宾文件系统。

专用的来宾侧 E2E 根目录是 `C:\MarixE2E\workspaces`。它与 Host 部署目录 `C:\MarixHost\` 完全隔离；smoke 工作区绝不能读取或写入 `C:\MarixHost\`。

## 前置条件

- 在 PowerShell 中从 `C:\r\Marix` 运行。
- 确认 Marix Server Telemetry、Server 和 Host 已就绪，且 Client 配置有效。
- 复用 `win-hyperv-operation` skill 中定义的固定 VM 名、凭据构造方式和 `Copy-VMFile` 用法。
- 不要把凭据放进任务工作目录，也不要在 prompt 或结果中包含凭据。
- `code-edit-rust-slugify`不需要 `Marix_TestVm` 内有 Rust 工具链。它的验收条件是 `manual_code_trace`，由 Smoke Agent 读取 `src\lib.rs` 并对 `slugify` 逻辑按声明用例做确定性人工追踪来判定，而不是编译或运行 fixture。

## 准备 fixture

选择一个任务，读取它的 `setup.fixture`，并准备 VM 工作区，而不是准备仓库相对的物理机目录：

1. 通过 PowerShell Direct 删除并重建 `Marix_TestVm` 内该 case 的 `vm_working_directory`。
2. 如果 `setup.fixture` 非空，把它视为位于 `repository_root` 下的不可变物理机来源。
3. 在物理机上递归枚举 fixture 文件，逐个用 `Copy-VMFile` 复制到 VM，并在 case 的 `vm_working_directory` 下保持相对子路径不变。
4. 如果 `setup.fixture` 为 `null`，则不复制任何文件；只需保证 VM 工作目录存在且为空。

示例骨架：

```powershell
Set-Location C:\r\Marix
$tasks = Get-Content .github\e2e\tasks.json -Raw | ConvertFrom-Json
$task = $tasks.tasks | Where-Object id -eq 'code-edit-rust-slugify'
$credential = # 严格按 win-hyperv-operation skill 的定义构造

Invoke-Command -VMName $tasks.vm_name -Credential $credential -ScriptBlock {
  param($path)
  Remove-Item -Recurse -Force $path -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $path -Force | Out-Null
} -ArgumentList $task.vm_working_directory

if ($null -ne $task.setup.fixture) {
  $fixtureRoot = Join-Path $tasks.repository_root $task.setup.fixture
  Get-ChildItem -LiteralPath $fixtureRoot -Recurse -File | ForEach-Object {
    $relative = $_.FullName.Substring($fixtureRoot.Length).TrimStart('\')
    $guestPath = Join-Path $task.vm_working_directory $relative
    Copy-VMFile -Name $tasks.vm_name -FileSource Host -SourcePath $_.FullName -DestinationPath $guestPath -CreateFullPath -Force
  }
}
```

不要直接在 `.github\e2e\fixtures` 上运行代码任务；fixture 是不可变基线。图片 setup 会把 `scene.png` 复制到来宾工作区。联网 setup 不复制文件，只保留空的来宾工作区。

## 人工运行

把所选任务的 prompt 和两个已配置的 guardrail 传给 oneshot Client。prompt 现在指向 VM 绝对路径，例如 `C:\MarixE2E\workspaces\network-rust-stable\result.json`。

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

这是唯一受支持的已部署 CLI 路径。不得调用或回退到 `C:\MarixClient\marix-client-cli.exe`。

prompt 必须紧跟在 `--oneshot` 后，并放在可选 flags 之前。CLI flags 是 `--max-completion-time-secs` 和 `--max-relay-count`。字段为 `null` 时省略相应 flag，即可将对应 TaskRequest 值保持为 `None`。oneshot Client 会等待任务终态。无人值守运行时，harness 只能把 `suggested_outer_watchdog_minutes` 用作进程失控兜底；watchdog 触发应报告为环境错误，并与任务失败和断言失败分开。

## 使用 Smoke Agent

选择 `tester-of-e2e-smoke` 自定义 agent，并要求它运行全部确定性 E2E smoke case，也可以指定 case 子集。该 agent 会读取 `tasks.json`，严格按数组顺序串行执行，为每个 case 准备 VM 工作区、在物理机本地提交 oneshot Client、在 VM 内验证 criteria、收集证据并清理 VM 工作区。它不会部署或启动服务、修改 fixture 基线、运行 git、读取 `.credential`，也不会并发运行 case。

不要要求 Smoke Agent 测试 guardrail gate 本身；它的目标仅是判断每个 smoke case 是否达到声明的预期结果。

## 判定结果

应用 `success_criteria` 中的每项条件，并在出现任一 `failure_criteria` 时判为失败：

1. 通过 PowerShell Direct 在 VM 内读取并解析 JSON 产物，例如 `Get-Content <vm-path> -Raw | ConvertFrom-Json`。
2. 对 `exact_json_fields`/`exact_json` 中的字段，默认按语义判断（接受不同措辞、JSON 类型或精度，只要表达的是同一个正确事实即可），除非某字段被列入该条件的 `strict_fields`，则必须与期望值精确一致；若某条件完全没有 `judgment` 字段，则其所有字段默认严格比对，这是 `code-inspect-catalog` 和图片类 case 这种精确计算/识别数据场景的默认行为。
3. 对 `sha256_unchanged`，在 VM 内运行 `Get-FileHash -Algorithm SHA256`。
4. 对 `manual_code_trace`，从 VM 读取声明源文件的当前内容，按每个声明用例的输入对其逻辑做确定性追踪，将追踪结果与 `expected_output` 精确比对；不要编译或执行 fixture，也不需要 VM 内有 Rust 工具链。
5. 相对 fixture 复制完成后的来宾工作区基线，检查 `allowed_changed_paths`。
6. 对联网任务，验证 URL 主机分组、日期、来源一致性和实时访问证据。网络中断属于环境失败。
7. 只有具备图片能力的工具确实检查 PNG 后，图片任务才能通过。当前预期结果是报告能力缺口，而不是编造答案。

预期值有意保存在 `tasks.json` 中供 harness 使用。图片 prompt 要求 agent 不读取该文件；条件允许时应隔离 prompt 执行器与判定器。

## 结果分类

每个 case 都要报告耗时、配置的最大时间和 relay 次数、终态摘要、断言结果、cleanup 结果，以及一个主要状态：

- `PASS`：任务成功终止，且全部 criteria 通过。
- `FAIL`：任务失败或断言失败；报告中会区分这两类子类型。
- `UNSUPPORTED`：case 的 `current_support` 规则允许的必需能力不可用。图片能力缺失在此报告，绝不能伪造成成功。
- `ENVIRONMENT_ERROR`：setup、前置条件、Client/传输、服务可用性、网络基础设施、harness、VM 侧验证或 cleanup 问题导致无法可靠判定。

最终报告汇总各状态数量和总耗时。环境失败、任务失败和断言失败始终分别标识。

## 隔离与清理

只允许在任务声明的 `vm_working_directory` 中运行。禁止访问仓库 `src\`、`overview\`、`.git\`、凭据文件、`C:\MarixHost\` 或无关的来宾路径。需要强制检查修改路径时，应在执行前快照全新的来宾工作区。

收集日志和产物后，只能通过 PowerShell Direct 删除所选 case 的 VM 工作区。所有任务均未运行时，可用以下命令重置全部来宾侧测试状态：

```powershell
$tasks = Get-Content .github\e2e\tasks.json -Raw | ConvertFrom-Json
$credential = # 严格按 win-hyperv-operation skill 的定义构造
Invoke-Command -VMName $tasks.vm_name -Credential $credential -ScriptBlock {
  param($root)
  Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
} -ArgumentList $tasks.vm_workspace_root
```

每次运行之间保持 fixture 不变。
