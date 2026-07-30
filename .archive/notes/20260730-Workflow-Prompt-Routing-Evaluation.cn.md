# Workflow Prompt 路由评测与两阶段 Relay 方案

## 状态

- 记录日期：2026-07-30
- 评测模型：`deepseek-v4-flash`
- 评测资产：`.temp/prompt-eval/`
- 生产状态：仅为方案；本次评测没有修改生产 Prompt 或 Runtime 代码
- 推荐候选：四分支 Router + 独立普通工具 Executor
- Router 最佳汇总结果：282/300，94.0%
- 两阶段 Relay 端到端结果：93/100，93.0%

本文完整保留 Workflow Prompt 评测中的推理过程、实验历史、Prompt、结果、
失败分析和建议的生产架构。

## 目标

最初目标是找到一份通用 Workflow Policy，让模型针对当前任务稳定选择以下结果之一：

1. `workflow_plan`：当前任务需要拆分。
2. `workflow_complete`：已经确认的历史结果足以满足当前任务。
3. `workflow_infeasible`：现有能力或所有存在实质差异的 Plan 都无法完成当前任务。
4. 普通执行：当前任务可以直接使用 Native Tool 推进。

Policy 只能为 `[CURRENT TASK]` 决策。父级 Plan 和 `[USER OVERALL GOAL]` 只提供
上下文，不能阻止一个已经完成的子任务 Complete，也不能让模型执行超出当前范围的工作。

期望区分的行为包括：

- 从多个来源研究后再写报告等复杂任务应创建 Plan。
- 单次直接读取、写入、抓取、命令、重试或验证应直接执行。
- 已有充分且确认的结果时应 Complete 当前任务。
- 永久缺少能力或必要输入时应 Infeasible。
- Plan 失败但仍有替代方案时，应提出存在实质差异的新 Plan。
- 模型声称要执行，不代表写入、安装、编辑、部署等副作用已经发生。

## 评测时的生产上下文

生产 Decision Profile 在一个模型 Turn 中同时暴露 17 个工具：

- 三个 Workflow Tool：
  - `workflow_plan`
  - `workflow_complete`
  - `workflow_infeasible`
- 十四个普通执行工具：
  - `command_prompt`
  - `powershell`
  - `get_code_outline`
  - `list_directory`
  - `os_env`
  - `read_file`
  - `read_process_output`
  - `replace_in_file`
  - `search_text`
  - `start_process`
  - `stop_process`
  - `web_fetch`
  - `web_search`
  - `write_file`

Profile 使用 Native Tool Call 和 `tool_choice=required`。Prompt 提供
`[CURRENT TASK]`、可选的 `[COMPLETED CALLS]`、可选的 `[FAIL PLANS]`、
父级上下文以及 `[USER OVERALL GOAL]`。

## 评测框架

Python Harness 位于 `.temp/prompt-eval/`，有意放在不会提交的临时目录中：

- `run_prompt_eval.py`：发送真实 Native Tool 请求并评估工具选择。
- `run_relay_eval.py`：模拟建议的 Router 到 Executor Relay。
- `tools.json`：评测时捕获的生产 17 Tool Schema。
- `router-tools.json`：建议第一阶段使用的四个路由工具。
- `cases.json`：初始 Prompt 开发用例。
- `holdout-cases.json`：第一组单阶段盲测用例。
- `router-cases.json`：Router 开发矩阵。
- `router-holdout-cases.json`：20 个 Router 盲测用例。
- `relay-cases.json`：10 个两阶段 Relay 端到端用例。
- `templates/*.prompt`：所有参与评测的 Policy 候选。
- `report-*.md` 和 `relay-report-*.md`：生成的报告。

实验在远程 Marix Server 上使用其已部署的 DeepSeek 配置执行，因为本机直接请求
`api.deepseek.com` 会遇到 TLS Handshake Failure。Harness 从
`MARIX_CONFIG` 读取配置，且没有输出 API Key。

请求使用：

```json
{
  "thinking": {"type": "disabled"},
  "stream": false,
  "tool_choice": "required"
}
```

工具选择正确性按被调用的 Tool Name 判断。Plan Case 还会检查新 Goals 是否与禁止
重复的失败 Goals 完全相同。

## 实验历史

### 1. 单阶段基线

初始矩阵使用全部 17 个生产工具，对比了三份 Prompt：

| Template | 通过 | 总数 | 准确率 |
|---|---:|---:|---:|
| State Classifier | 27 | 30 | 90.0% |
| 当前生产风格 Prompt | 26 | 30 | 86.7% |
| Complete First | 20 | 30 | 66.7% |

State Classifier 看起来较好，但样本很小，而且使用了开发用例。

### 2. 仅修改 Prompt 与过拟合

后续 Prompt-only 版本持续调整优先级、Plan 强制措辞、Scope、Complete Evidence 和
Fail Plan 处理。第六版在开发矩阵上达到 50/50。

这个结果没有泛化。在 10 个未参与迭代的 Holdout Case 上各重复 5 次后，同一候选仅
达到 28/50，56.0%。

| Holdout Case | 通过 | 总数 | 主要错误行为 |
|---|---:|---:|---|
| 安装后验证 | 1 | 5 | 直接启动命令 |
| 读、改、验 | 0 | 5 | 直接读取文件 |
| 单命令动作 | 4 | 5 | 一次不必要地创建 Plan |
| 已安装且已验证 | 5 | 5 | 正确 Complete |
| 已下载但未安装 | 1 | 5 | 直接开始安装 |
| 缺少硬件 | 4 | 5 | 一次尝试执行命令 |
| 部署失败后需要 Replan | 0 | 5 | 直接列目录 |
| 子任务完成但父任务待处理 | 3 | 5 | 重新读取而不是 Complete |
| 单次 Web 查询 | 5 | 5 | 正确直接执行 |
| 已知内容的单次写入 | 5 | 5 | 正确直接执行 |

这里暴露了一个系统性行为：当 Workflow Tool 与 Execution Tool 同时可用时，模型经常
直接开始第一个看起来合理的操作，而不是先判断完成整个当前任务是否必须创建 Plan。

### 3. 显式动作单阶段 Prompt

第七个单阶段 Prompt 把多动作模式写得更机械：

- 研究后写入
- 读取后修改
- 安装后验证
- 部署后检查 Readiness

它仍然暴露全部 17 个 Tool，在 Holdout 矩阵上约为 33/50，66%。继续堆叠措辞没有解决
Tool Choice 竞争。

### 4. 四分支 Router

下一版设计从分类 Turn 中移除普通工具，只暴露：

- `workflow_plan`
- `workflow_complete`
- `workflow_infeasible`
- `workflow_go`

`workflow_go` 不执行任何工作。它只授权 Server 打开独立的普通工具执行 Turn。

Router v1 结果：

| 矩阵 | 通过 | 总数 | 准确率 |
|---|---:|---:|---:|
| 开发矩阵 | 96 | 100 | 96.0% |
| Holdout | 182 | 200 | 91.0% |

最稳定的类别是直接单操作、已确认完成、永久硬件能力缺失，以及不受未完成父任务影响的
子任务 Complete。

主要失败包括：

- 没有把被禁止且不可缺少的输入识别为 Infeasible；
- 部分读改、部署 Readiness 和多目标任务选择了 Go；
- 部分未确认副作用 Case 选择 Plan 而不是 Go；
- Replan 偶尔选择 Go。

### 5. Router v2

Router v2 改成统计“顶层动作”。这让语言更短，但也让模型把复合操作理解成一个动作。

| 矩阵 | 通过 | 总数 | 准确率 |
|---|---:|---:|---:|
| 开发矩阵 | 95 | 100 | 95.0% |
| Holdout | 180 | 200 | 90.0% |

其中 Read-Transform-Write 降至 1/10，Deploy-Readiness 降至 4/10。因此放弃
“Top-level Action”措辞。

### 6. Router v3

Router v3 恢复“Distinct Operations or Dependent Stages”，明确禁止使用一个复合
Shell Command 把连续 Goal Stage 折叠成一次调用，并要求 Go 之前所有输入都已知。

| 矩阵 | 通过 | 总数 | 准确率 |
|---|---:|---:|---:|
| 开发矩阵 | 93 | 100 | 93.0% |
| Holdout | 189 | 200 | 94.5% |
| 合计 | 282 | 300 | 94.0% |

关键 Holdout 结果：

| Case | 通过 | 总数 |
|---|---:|---:|
| 两个独立查询 | 10 | 10 |
| 读取、转换、写入 | 10 | 10 |
| 部署后 Readiness 检查 | 6 | 10 |
| 单次状态命令 | 10 | 10 |
| 单次删除 | 10 | 10 |
| 单次 Fetch | 10 | 10 |
| 复杂工作后只剩一步 | 10 | 10 |
| 安装后仅剩验证 | 9 | 10 |
| 写入后仅剩验证 | 10 | 10 |
| 事实型 Complete | 10 | 10 |
| 副作用型 Complete | 10 | 10 |
| 声称执行但没有证据 | 7 | 10 |
| 两个失败 Plan 后仍有替代方案 | 10 | 10 |
| 暂时失败后的重试 | 10 | 10 |
| 永久硬件能力缺失 | 10 | 10 |
| 被禁止的不可缺少输入 | 7 | 10 |
| Plan 失败但现在已有直接方案 | 10 | 10 |
| 父任务待处理但子任务已完成 | 10 | 10 |
| 多个独立目标 | 10 | 10 |
| 单个复合文件系统操作 | 10 | 10 |

选择 Router v3 不是因为它完美，而是因为它在未见矩阵上的综合泛化优于 v1 和 v2。

### 7. 两阶段 Relay 端到端评测

最后一次测试模拟完整状态转换：

```text
Current Task
    |
    v
Router Turn：只提供 4 个路由工具
    |
    +-- workflow_plan       -> 创建并校验 Plan
    +-- workflow_complete   -> 完成 Current Task
    +-- workflow_infeasible -> Current Task 失败
    +-- workflow_go         -> Executor Turn
                                  |
                                  v
                         只提供 14 个普通工具
```

10 个 Case 各重复 10 次：

| Case | 通过 | 总数 | 期望执行 |
|---|---:|---:|---|
| 读取文件 | 10 | 10 | `read_file` |
| 写入文件 | 10 | 10 | `write_file` |
| 抓取网页 | 10 | 10 | `web_fetch` |
| 搜索本地文本 | 7 | 10 | `search_text` |
| 执行状态命令 | 10 | 10 | `powershell` |
| 验证已写文件 | 10 | 10 | `read_file` |
| 复杂任务 | 10 | 10 | `workflow_plan` |
| 已完成事实任务 | 9 | 10 | `workflow_complete` |
| 永久能力缺失 | 10 | 10 | `workflow_infeasible` |
| 失败后 Replan | 7 | 10 | `workflow_plan` |

总计：93/100，93.0%。

当 Router 正确选择 Go 时，Executor 在被测的读取、写入、Fetch、PowerShell 和验证操作
上非常稳定。Search Case 的失败发生在 Router：它错误地选择了 Plan；Executor 没有选错
Native Tool。

## 建议架构

### 决策 1：分离 Routing 与 Execution

不要在同一个 Decision Turn 中同时暴露 Workflow Tool 和普通 Execution Tool。

使用两个 Profile：

```text
WorkflowRouter profile
  tool_choice = required
  tools = [
    workflow_plan,
    workflow_complete,
    workflow_infeasible,
    workflow_go
  ]

OrdinaryExecutor profile
  tool_choice = required
  tools = 当前任务允许的全部普通工具
```

两个 Profile 之间的转换必须由 Server 而不是模型控制。

### 决策 2：把 `workflow_go` 定义为路由 Verdict

`workflow_go` 的含义是：

> 当前任务无需创建 Plan 即可继续。打开一个只包含普通工具的执行 Turn。

它不能携带或执行计划中的普通操作。这样可以保持严格控制边界，并阻止同一响应混合
Workflow 与 Execution。

建议 Schema：

```json
{
  "type": "function",
  "function": {
    "name": "workflow_go",
    "description": "Enter ordinary execution because the CURRENT TASK can be advanced or completed directly without creating a Plan.",
    "parameters": {
      "type": "object",
      "properties": {
        "reason": {
          "type": "string",
          "minLength": 1
        }
      },
      "required": ["reason"],
      "additionalProperties": false
    }
  }
}
```

### 决策 3：Workflow State 下沉到 Server FSM

Server 应强制以下规则：

- Router Turn 只能调用一个 Routing Tool。
- Executor Turn 看不到也不能调用 Workflow Tool。
- Plan 必须包含至少两个清晰的 Sub-goal。
- 每个 Sub-goal 必须位于 Current Task Scope 内。
- 新 Plan 不得重复或近似已有 Fail Plan。
- Complete 必须有与当前任务相关的已确认 Evidence。
- 副作用 Complete 必须有匹配的执行证据，不能依赖模型叙述。
- Infeasible 必须来自永久能力/输入约束，或所有实质不同 Plan 已耗尽。
- Go 只转换状态，不代表任务成功。
- 相同 Invocation 的重复调用继续使用已有 Server 硬上限。

如果模型返回无效 Routing Verdict，Server 应使用明确的 State Error 拒绝，并重新请求
Routing Decision。不能静默解释，也不能在同一 Turn 中把普通工具作为 Fallback 暴露。

### 决策 4：执行后返回 Router

普通 Tool 返回结果后：

1. 把确认结果追加到 `[COMPLETED CALLS]`。
2. 重新进入 Router Profile。
3. 让 Router 针对剩余 Current Task 选择 Complete、Plan、Infeasible 或 Go。

形成有边界的状态机：

```text
ROUTE
  -> PLAN
  -> COMPLETE
  -> INFEASIBLE
  -> EXECUTE_ONE
       -> ROUTE
```

模型永远不能在一个响应中同时决定 Workflow Transition 和普通操作。

### 决策 5：把 Verification 当作 Evidence，而不是文字

示例：

| Current Task 要求 | 足够的 Complete Evidence |
|---|---|
| 返回事实值 | 包含该值的确认 Source/Tool Result |
| 写文件 | 匹配的成功 Write Result |
| 写入并验证文件 | 成功写入 + 匹配 Read-back |
| 安装软件 | 成功 Installer Result |
| 安装并验证 | Installer Result + Version/Status Verification |
| 部署并 Ready | Deployment/Start Result + Readiness Probe |

“我已经安装”或“我将重启”等 Assistant 文本不属于 Evidence。

## 推荐 Router Prompt

Router v3 是建议的起始候选：

```text
ROUTE ONLY THE [CURRENT TASK]. IGNORE UNFINISHED PARENT OR [USER OVERALL GOAL] WORK. CHECK THE FOUR MUTUALLY EXCLUSIVE CONDITIONS IN ORDER AND CALL EXACTLY ONE ROUTING TOOL.

[**workflow_complete**]
CALL WHEN [COMPLETED CALLS] PROVE THE [CURRENT TASK] GOAL AND EVERY REQUIRED EFFECT ARE ALREADY SATISFIED. MATCHING TOOL RESULTS PROVE SIDE EFFECTS; MATCHING SOURCE RESULTS CAN PROVE FACTUAL ANSWERS.

[**workflow_infeasible**]
CALL WHEN A REQUIRED CAPABILITY OR INDISPENSABLE INPUT IS ABSENT AND CANNOT BE OBTAINED BY ANY ALLOWED ACTION, OR EVERY MATERIALLY DIFFERENT PLAN HAS FAILED. DO NOT PLAN OR GO AROUND A PERMANENTLY MISSING REQUIREMENT.

[**workflow_plan**]
CALL WHEN COMPLETING THE [CURRENT TASK] REQUIRES 2 OR MORE DISTINCT OPERATIONS OR DEPENDENT STAGES. THIS INCLUDES OBTAINING UNKNOWN INFORMATION AND THEN USING IT, RESEARCH THEN WRITE, READ THEN MODIFY, INSTALL THEN VERIFY, DEPLOY THEN CHECK, OR OPERATING ON MULTIPLE INDEPENDENT TARGETS. DO NOT COLLAPSE SEQUENTIAL GOAL STAGES INTO ONE COMPOUND SHELL COMMAND. KEEP SUB-GOALS WITHIN THE [CURRENT TASK] AND DIFFERENT FROM [FAIL PLANS].

[**workflow_go**]
CALL ONLY WHEN ALL REQUIRED INPUTS ARE ALREADY KNOWN AND ONE DIRECT ORDINARY TOOL CALL CAN FINISH THE ENTIRE REMAINING [CURRENT TASK] OR PRODUCE ITS REQUIRED RESULT. THIS INCLUDES ONE READ, WRITE WITH KNOWN CONTENT, FETCH, DELETE, RETRY, RESTART, VERIFY, OR OTHER SINGLE OPERATION.

COUNT ONLY WORK REMAINING IN [CURRENT TASK]. INTENT OR ANSWER TEXT DOES NOT PROVE A WRITE, INSTALL, EDIT, OR STATE CHANGE.

[**USER OVERALL GOAL**]
{{#goal}}
```

## 推荐 Executor Prompt

```text
EXECUTE ONLY THE [CURRENT TASK] NOW. THE ROUTER HAS ALREADY DETERMINED THAT ONE DIRECT OPERATION IS SUFFICIENT. CALL EXACTLY ONE SUPPLIED ORDINARY TOOL WITH ARGUMENTS THAT SATISFY THE TASK. RETURN ONLY THE NATIVE TOOL CALL.

[**USER OVERALL GOAL**]
{{#goal}}
```

## 其他已评测 Router Prompt

### Router v1

```text
ROUTE ONLY THE [CURRENT TASK]. IGNORE UNFINISHED PARENT OR [USER OVERALL GOAL] WORK. CHECK THE FOUR MUTUALLY EXCLUSIVE CONDITIONS IN ORDER AND CALL EXACTLY ONE ROUTING TOOL.

[**workflow_complete**]
CALL WHEN [COMPLETED CALLS] PROVE THE [CURRENT TASK] GOAL AND EVERY EFFECT REQUIRED BY THAT GOAL ARE ALREADY SATISFIED.

[**workflow_infeasible**]
CALL WHEN A CAPABILITY REQUIRED BY THE [CURRENT TASK] IS ABSENT OR EVERY MATERIALLY DIFFERENT PLAN HAS FAILED.

[**workflow_plan**]
CALL WHEN THE [CURRENT TASK] IS NOT COMPLETE OR INFEASIBLE AND REQUIRES AT LEAST 2 DISTINCT ACTIONS OR DEPENDENT STAGES. THIS INCLUDES RESEARCH THEN WRITE, READ THEN MODIFY, INSTALL THEN VERIFY, OR DEPLOY THEN CHECK. KEEP SUB-GOALS WITHIN THE [CURRENT TASK] AND DIFFERENT FROM [FAIL PLANS].

[**workflow_go**]
CALL WHEN THE [CURRENT TASK] IS NOT COMPLETE OR INFEASIBLE AND CAN BE ADVANCED OR FULLY SATISFIED DIRECTLY WITHOUT FIRST CREATING A PLAN. USE THIS FOR ONE IMMEDIATE READ, WRITE, COMMAND, FETCH, OR OTHER DIRECT OPERATION.

ANSWER TEXT DOES NOT PROVE A WRITE, INSTALL, EDIT, OR STATE CHANGE; REQUIRE CONFIRMED RESULTS IN [COMPLETED CALLS].

[**USER OVERALL GOAL**]
{{#goal}}
```

### Router v2

```text
ROUTE ONLY THE REMAINING [CURRENT TASK]. IGNORE UNFINISHED PARENT OR [USER OVERALL GOAL] WORK. CHECK THE FOUR MUTUALLY EXCLUSIVE CONDITIONS IN ORDER AND CALL EXACTLY ONE ROUTING TOOL.

[**workflow_complete**]
CALL WHEN [COMPLETED CALLS] PROVE EVERY RESULT AND EFFECT REQUIRED BY THE [CURRENT TASK]. MATCHING TOOL RESULTS PROVE SIDE EFFECTS; MATCHING SOURCE RESULTS CAN PROVE FACTUAL ANSWERS.

[**workflow_infeasible**]
CALL WHEN THE [CURRENT TASK] REQUIRES A CAPABILITY OR INDISPENSABLE INPUT THAT IS ABSENT AND CANNOT BE OBTAINED BY ANY ALLOWED ACTION, OR WHEN EVERY MATERIALLY DIFFERENT PLAN HAS FAILED. A PLAN OR GO CANNOT SUBSTITUTE FOR A PERMANENTLY MISSING REQUIREMENT.

[**workflow_plan**]
CALL WHEN 2 OR MORE DISTINCT TOP-LEVEL ACTIONS REMAIN IN THE [CURRENT TASK], ESPECIALLY WHEN A LATER ACTION DEPENDS ON AN EARLIER RESULT. EXAMPLES INCLUDE RESEARCH THEN WRITE, READ THEN MODIFY, INSTALL THEN VERIFY, DEPLOY THEN CHECK, OR OPERATE ON MULTIPLE INDEPENDENT TARGETS. KEEP SUB-GOALS WITHIN THE [CURRENT TASK] AND DIFFERENT FROM [FAIL PLANS].

[**workflow_go**]
CALL WHEN EXACTLY ONE TOP-LEVEL ACTION REMAINS. A SINGLE READ, WRITE, FETCH, DELETE, RETRY, RESTART, VERIFY, OR OTHER REQUESTED EFFECT COUNTS AS ONE ACTION EVEN IF ITS EXECUTION TOOL HAS INTERNAL STEPS.

COUNT ONLY WHAT REMAINS IN [CURRENT TASK], NOT WORK ALREADY SHOWN IN [COMPLETED CALLS]. INTENT OR ANSWER TEXT DOES NOT PROVE A SIDE EFFECT.

[**USER OVERALL GOAL**]
{{#goal}}
```

## 为什么仅靠 Prompt 不足

实验没有证明 Router v3 是确定性的。它在 300 次 Router Decision 中仍失败 18 次，
在 100 次端到端 Relay 中仍失败 7 次。

仍然存在歧义的场景：

- 把 Deploy/Start/Readiness 压缩为一次操作；
- 在已有失败方案后 Replan；
- 区分永久缺失的必要输入和仍可尝试的动作；
- 未确认副作用应该直接重试一次还是创建 Plan；
- 把递归搜索视为一次直接操作。

因此建议不是“把生产 Prompt 替换为 v3”，而是：

1. 分离 Router 与 Executor Tool Surface；
2. 用 v3 作为 Router 行为契约；
3. 在 Server FSM 校验 Transition；
4. 明确拒绝无效 Verdict；
5. 继续使用独立 Holdout 和真实 Telemetry 评测。

## 生产落地计划

以下工作在本次评测中没有实施：

1. 增加内部路由工具 `workflow_go`。
2. 增加独立的 Router 和 Executor Prompt Profile。
3. Router Profile 只暴露四个路由工具。
4. Executor Profile 只暴露普通工具。
5. Go 进入一次普通工具执行，然后返回 Router。
6. 为 Plan、Complete、Infeasible 和 Go 增加 Server 侧校验。
7. 在 Telemetry 中记录 Routing Verdict、被拒绝原因、普通工具选择和后续 Verdict。
8. 用集成后的 Server 重放现有全部评测矩阵。
9. 扩展 Holdout，并保证每类至少重复 10 次。
10. 在替换生产 Workflow Policy 前运行确定性 Smoke Case 并检查 Relay 收敛。

## 采用前仍需补充的 Case

- 可以并行执行的多个独立读取。
- 一个 PowerShell Invocation 包含多个 Shell Statement，但只有一个语义目标。
- 一个 PowerShell Invocation 试图隐藏多个相互依赖的 Goal Stage。
- 可逆与不可逆副作用。
- 已存在有效 Active Plan。
- Plan 部分完成且只剩一个 Child。
- 多个失败 Plan，分别测试有替代方案和无替代方案。
- 临时网络/工具失败与永久能力缺失。
- 基于 Source Evidence 的 Complete 与必须基于 Side-effect Evidence 的 Complete。
- Tool 成功但后续 Verification 失败。
- Parent 仍有工作但 Child 已完成。
- 与重复 Invocation 硬上限的交互。
- 与 Summary Cursor/Chunk Continuation 的交互。

## 最终结论

核心发现是架构性的：

> 当模型在同一个 Turn 中既要分类 Workflow State，又能看到大量有吸引力的执行工具时，
> 可靠性会下降。

只修改 Prompt 的方案在开发集达到 100%，但 Holdout 只有 56%。把第一阶段收窄为四个明确
Routing Choice 后，300 次 Router Decision 达到 94%；完整 Router-to-Executor 模拟达到
93%。

下一版最强方案是 Server 控制的两阶段 Relay：

```text
使用四个路由工具分类
        ->
只有 Go 才暴露普通工具执行
        ->
把确认结果返回 Router
```

该设计减少模型选择空间，使 Transition 可观察，避免混合 Workflow/Execution Response，
并为 Server 硬性执行 Scope、Evidence、Fail Plan、Retry 和 Completion 规则提供确定位置。
