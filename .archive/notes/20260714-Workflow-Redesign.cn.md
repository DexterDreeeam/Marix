# Marix 嵌套式 Workflow 重构

## 目的

本文定义将当前扁平 Task/Plan/Step Workflow 改为嵌套 Intent/Plan Workflow 的目标架构。

`Task` 继续表示一个 user request 及其所有后续流程。重构遵循以下核心规则：

- Task 存储所有 Intent Actor。
- Task 只持有一个根 Intent，其不可变内容初期就是 user request。
- Intent 可以先后执行多个 Step，也可以把自身交给一个 Plan。
- Plan 管理有序的子 IntentSignature。
- Step 持有并行 Invocation。
- Relay 不属于 Intent、Plan 或 Step。它是 Intent 或 Plan 在需要决断时临时触发的 Model Verdict。

```text
Task
├── root IntentSignature
└── WorkQueue<IntentSignature, Intent>

Intent
├── immutable content
├── WorkQueue<StepSignature, Step>
├── optional Plan
└── 按需触发 Relay verdict

Plan
└── ordered child IntentSignatures

Step
└── parallel Invocations
```

## 核心所有权

### Task

一个 Task 表示一个 user request 以及完成该请求所需的全部后续流程。

Task 初始创建一个根 Intent：

```text
Task.user_request -> root Intent.content
```

Task 存储所有根 Intent 和嵌套 Intent：

```text
Task.intents: WorkQueue<IntentSignature, Intent>
```

Plan 和 Step 都不拥有 Intent 实例。它们只保存 IntentSignature，由 Task 解析到具体 Actor。

根 Intent 成功时 Task 成功。根 Intent 返回不可行的 `IntentResult` 时 Task 结束并返回未完成结果。

### Intent

Intent 是不可变的目标单元：

```text
Intent.content: String
```

Intent 内容永远不变，但其执行策略可以演化。

Intent 存储：

```text
IntentState {
    signature: IntentSignature,
    content: String,
    steps: WorkQueue<StepSignature, Step>,
    plan: Option<Plan>,
    status: IntentStatus,
    result: Option<IntentResult>,
}
```

Intent 可以先执行多个 Step，然后发现目标复杂度比预期高，再创建 Plan。

Intent 一旦交给 Plan：

- Intent 停止直接创建或执行 Step；
- Intent 只能由该 Plan 驱动；
- Plan 返回终态结果后，Intent 才恢复执行并完成。

### Plan

Plan 将一个 Parent Intent 拆解为有序的子 Intent。

PlanSignature 包含 Parent IntentSignature：

```text
PlanSignature {
    intent: IntentSignature,
    ...
}
```

Plan 不拥有子 Intent 实例。它只保存有序 Signature、历史不可行结果和当前终态结果：

```text
PlanState {
    signature: PlanSignature,
    intents: Vec<IntentSignature>,
    failures: Vec<PlanResult>,
    result: Option<PlanResult>,
}
```

Vector 保留模型给出的顺序。Task 始终是唯一 Intent Actor 存储，并提供每个子 Signature 的当前状态与结果。

`failures` 只追加，保存重制过程中每一个被判定为不可行的历史 Plan 结构对应的 `PlanResult`。`result` 只保存当前 Plan 的终态结果；Plan 运行期间保持 `None`。

同一时间只执行一个子 Intent。当前子 Intent 成功后才启动下一个。

### Step

Step 是属于某个 Intent 的并行工具调用批次。

StepSignature 包含 IntentSignature：

```text
StepSignature {
    intent: IntentSignature,
    ...
}
```

Intent 可以持有多个已经执行或正在执行的 Step：

```text
Intent.steps: WorkQueue<StepSignature, Step>
```

Step 持有一组并行 Invocation。Step 汇总 Invocation 输出，并向 Intent 返回一个整体结果。

### Invocation

Invocation 执行一次工具调用。InvocationSignature 包含 StepSignature：

```text
InvocationSignature {
    step: StepSignature,
    ...
}
```

Invocation 继续复用当前 Host/Executor/Execution 行为。

工具返回的 error 内容仍然是执行结果，不会自动成为 Workflow 层失败。

### Relay

Relay 是临时 Model Verdict 机制。

它不属于 Intent、Plan 或 Step 的持久结构。Intent 或 Plan 需要决断时触发 Relay，然后暂停，直到 Relay 回报。

RelaySignature 包含 IntentSignature：

```text
RelaySignature {
    intent: IntentSignature,
    ...
}
```

Relay 的运行期资源可以继续由 Task 级 Runtime registry 管理，但 Relay 不能成为持久 Intent/Plan 树的一部分。

## Intent Verdict

Intent 调用 Model Relay，并从 3 种结果中选择一种：

1. **工具执行**
   - 创建另一个 Step。
   - Step 运行一个或多个并行 Invocation。
   - Step 完成后，Intent 可以再次请求 Verdict。

2. **拆解为 Plan**
   - 创建一个包含有序子 IntentSignature 的 Plan。
   - 将 Plan 存入 Intent。
   - Intent 停止直接执行 Step。
   - 只在 Plan 返回结果时恢复。

3. **完成**
   - 生成 `IntentResult`。
   - 将 Intent 标记为终态。

因此一个 Intent 可以经历：

```text
Relay verdict -> Step -> Relay verdict -> Step -> Relay verdict -> Plan
```

Plan 创建后，Intent 不能再回到直接 Step 执行。

## Plan 执行

1. Plan 通过 Task 创建子 Intent Actor。
2. Task 将所有子 Intent 存入 `Task.intents`。
3. Plan 只按执行顺序存储子 IntentSignature。
4. Plan 启动第一个子 Intent。
5. Plan 记录成功的子 Intent Result，并按 Signature 顺序推进。
6. Plan 启动下一个子 Intent。
7. 所有子 Intent 成功后，Plan 生成 `PlanResult`。
8. Parent Intent 消费 PlanResult 并完成。

## Result 类型

不创建只表达失败的独立结构。成功与不可行结果统一使用 `protocol/` 中的相同结果类型。

建议的 protocol 结构：

```text
IntentResult {
    intent: String,
    status: IntentStatus,
    output: String,
}

PlanResult {
    intents: Vec<String>,
    status: PlanStatus,
    output: String,
}
```

精确字段可以在 Protocol 设计阶段继续收敛，但正向和负向结果都使用同一种 Result 类型，不再创建独立 failure DTO。

对于 PlanResult：

- `intents` 按顺序保存扁平子 Intent 内容；
- `output` 描述已取得的结果或当前 Plan 结构不可行的原因；
- `status` 区分成功与不可行终态。

IntentResult 和 PlanResult 必须放在 `protocol/`，不能放在 Server 私有模块。

## 工具错误与 Intent 可行性

一个 Step 可以包含多个并行 Invocation。Invocation 可以返回工具 error，但该输出仍然是信息。

Step 完成后，Intent 带着累积结果触发 Relay Verdict。Verdict 可以：

- 根据新信息请求另一个 Step；
- 将 Intent 拆解为 Plan；
- 完成 Intent；
- 判定 Intent 不可行。

只有模型判定后续 Step 和 Plan 都无法完成不可变目标时，Intent 才不可行。

系统失败必须单独处理：

- 传输中断；
- Actor 缺失；
- 事件格式错误；
- Runtime 不可用；
- 状态损坏。

系统失败不能被转换成普通工具输出。

## Plan 重制

Intent 内容不可变，但 Plan 结构可以重制。

当子 Intent 返回不可行 `IntentResult` 时，Plan 暂停并触发 Relay Verdict。

Verdict 输入：

- Parent Intent；
- 当前有序子 Intent 字符串；
- `Plan.failures` 中的所有 `PlanResult`；
- 可复用的成功子 `IntentResult`。

模型只选择一种结果：

1. Parent Intent 不可行。
2. 返回替代 `PlanDraft`。

替换 Plan 前，先将当前不可行结构记录为 `PlanResult` 并追加到 `Plan.failures`。

替代 Plan 可以复用之前成功的 Intent：

- 第一版按 normalized Intent 内容完全相同进行匹配；
- 从 Task 的 Intent store 中解析可复用结果；
- 将可复用子 IntentSignature 标记为 complete；
- 从第一个没有可复用结果的子 Intent 继续执行。

如果模型判定 Parent Intent 不可行，Plan 将结果报告给 Parent Intent。Parent Intent 随后以不可行 IntentResult 终止。

## Routing

事件层级围绕 Task 的中央 Intent store：

```text
TaskEvent
└── Intent(IntentSignature, IntentEvent)
    ├── Step(StepSignature, StepEvent)
    │   └── Invocation(InvocationSignature, InvocationEvent)
    ├── Plan(PlanSignature, PlanEvent)
    └── Relay(RelaySignature, RelayEvent)
```

Relay Event 先 route 到 Intent。

Intent 收到 Relay Result 后：

1. 如果 Intent 存在未完成的活动 Plan，将 Verdict 转发给该 Plan。
2. 否则，将 Verdict 作为 Intent 自己的执行决断。

这样 RelaySignature 始终围绕 Intent 身份，同时允许 Intent 或其活动 Plan 触发 Verdict。

Plan 只观察子 Intent Result，不直接处理 Step、Invocation 或 Execution 细节。

## Signature 血缘

必须满足：

```text
TaskSignature
└── IntentSignature
    ├── PlanSignature
    ├── StepSignature
    │   └── InvocationSignature
    └── RelaySignature
```

规则：

- `PlanSignature` 包含 `IntentSignature`。
- `StepSignature` 包含 `IntentSignature`。
- `InvocationSignature` 包含 `StepSignature`。
- `RelaySignature` 包含 `IntentSignature`。
- Plan 子列表存储 `IntentSignature`，不能存储 Intent 实例。

## 暂停与恢复语义

Intent 和 Plan 使用显式等待状态：

```text
IntentStatus:
  Created
  Running
  WaitingRelay
  WaitingStep
  WaitingPlan
  Succeed
  Infeasible
  Canceled

PlanStatus:
  Created
  Running
  WaitingIntent
  WaitingRelay
  Succeed
  Infeasible
  Canceled
```

触发 Relay 时：

1. 调用者状态设为 `WaitingRelay`。
2. 记录预期 RelaySignature。
3. 不启动其他子操作。
4. 只有匹配的 Relay Result 才能恢复。
5. 处理 Verdict 前清空 expected Relay。

每个等待状态都必须指向一个活动子节点或预期事件。

## 状态不变量

1. Task 拥有并存储所有 Intent Actor。
2. Task 只拥有一个根 IntentSignature。
3. Intent content 不可变。
4. Intent 拥有 Step WorkQueue 和可选 Plan。
5. Plan 只存子 IntentSignature，不存 Intent 实例。
6. Plan 子 Intent 顺序执行。
7. Step 内 Invocation 并行执行。
8. Relay 是临时 Verdict，不属于持久 Workflow 树。
9. Intent 或 Plan 等待 Relay Verdict 时必须暂停。
10. Intent 创建 Plan 后，只能由该 Plan 驱动。
11. 工具 error output 是信息。
12. Plan 结构可替换，Parent Intent 内容不可变。
13. 历史不可行 Plan 结构以 `PlanResult` 存入 `Plan.failures`；当前终态结果存入 `Plan.result`。
14. 复用结果必须通过 Task Intent store 显式追踪。
15. 每个终态只发送一次。
16. 任何 Actor 都不能在没有活动子节点或预期 Signature 时保持等待。

## 最小状态所有权

```text
TaskState
  root_intent: IntentSignature
  intents: WorkQueue<IntentSignature, Intent>

IntentState
  signature: IntentSignature
  content: String
  steps: WorkQueue<StepSignature, Step>
  plan: Option<Plan>
  status: IntentStatus
  result: Option<IntentResult>
  expected_relay: Option<RelaySignature>

PlanState
  signature: PlanSignature
  intents: Vec<IntentSignature>
  failures: Vec<PlanResult>
  result: Option<PlanResult>
  status: PlanStatus
  expected_relay: Option<RelaySignature>

StepState
  signature: StepSignature
  invocations: WorkQueue<InvocationSignature, Invocation>
  output
  status
```

避免存储重复 Intent 实例、复制的 Plan 树或只用于失败场景的冗余结构。

## 待决定问题

1. `IntentResult` 与 `PlanResult` 的精确字段和 status enum。
2. Step、PlanDraft、完成、不可行 4 种 Relay Verdict 的精确 JSON。
3. Plan 重制是修改原 Actor，还是创建新 PlanSignature。
4. 最大 Plan 重制次数。
5. 可复用 Intent 的精确 normalized 规则。
6. 环境变更工具执行后，之前成功的 IntentResult 是否可能过期。
7. 等待 Relay 时的取消行为。
8. 一次 Intent Verdict 只创建一个 Step，还是可以创建多个顺序 Step。

## TODO

### 阶段 1：Protocol

- [ ] 定义 `IntentSignature`。
- [ ] 将 `IntentSignature` 加入 `PlanSignature`。
- [ ] 将 `IntentSignature` 加入 `StepSignature`。
- [ ] 让 `InvocationSignature` 从 `StepSignature` 获取血缘。
- [ ] 将 `IntentSignature` 加入 `RelaySignature`。
- [ ] 定义 `IntentStatus` 和 `PlanStatus` 的等待/终态。
- [ ] 定义 protocol-owned `IntentResult`。
- [ ] 定义 protocol-owned `PlanResult`。
- [ ] 定义 `IntentEvent` 并更新嵌套事件路由。
- [ ] 定义 Step、PlanDraft、完成、不可行 4 种 Model Verdict。

### 阶段 2：Task Intent Store

- [ ] 为 Task 增加根 IntentSignature。
- [ ] 为 Task 增加 `WorkQueue<IntentSignature, Intent>`。
- [ ] 根据 user request 创建根 Intent。
- [ ] 通过 Task route 所有 IntentEvent。
- [ ] 如果不再需要，从 Task 删除 Plan 实例存储。
- [ ] 让 Plan 通过 Task 解析子 IntentSignature。

### 阶段 3：Intent Actor

- [ ] 增加 `Intent`、`IntentState`、`IntentRuntime`。
- [ ] 保证 `Intent.content` 不可变。
- [ ] 增加 Intent Step WorkQueue。
- [ ] 增加可选 Plan。
- [ ] 增加 expected RelaySignature 和等待状态。
- [ ] 实现 3 选 1 Relay Verdict。
- [ ] 允许在 Plan 拆解前执行多个 Step round。
- [ ] 创建 Plan 后禁止直接 Step 执行。

### 阶段 4：Step 与 Invocation

- [ ] 将 Step 重定义为并行 Invocation 组。
- [ ] 用 Step WorkQueue 存储 Invocation。
- [ ] 汇总 Invocation Result。
- [ ] 将工具 error 保留为信息。
- [ ] 将 Step Result 返回 Intent。
- [ ] 从 Step 移除 Model Relay 所有权。

### 阶段 5：顺序 Plan

- [ ] 将 Plan 子列表改为 `Vec<IntentSignature>`。
- [ ] 增加 `failures: Vec<PlanResult>` 保存历史不可行 Plan 结构。
- [ ] 增加 `result: Option<PlanResult>` 保存当前终态结果。
- [ ] 同时只启动一个子 Intent。
- [ ] 成功 IntentResult 后推进。
- [ ] 所有子 IntentSignature 成功后完成。
- [ ] 将 PlanResult 返回 Parent Intent。

### 阶段 6：Relay Verdict Routing

- [ ] 将 Relay 改为临时、由 Task Runtime 管理。
- [ ] 将 RelayEvent route 到 IntentSignature。
- [ ] Intent 有活动未完成 Plan 时，将 Verdict 转发给 Plan。
- [ ] 否则由 Intent 处理 Verdict。
- [ ] 每个等待 Actor 只允许一个 expected Relay。
- [ ] 忽略或拒绝过期 Relay Result。

### 阶段 7：Plan 重制

- [ ] 重制前将当前不可行 PlanResult 追加到 `failures`。
- [ ] Verdict Prompt 包含 Parent Intent 与全部 `failures`。
- [ ] 包含可复用的成功 IntentResult。
- [ ] 解析不可行或替代 PlanDraft。
- [ ] 替换有序子 IntentSignature。
- [ ] 复用 normalized 内容完全相同的 Intent Result。
- [ ] 从第一个不可复用子节点继续。
- [ ] 增加有界重制次数。

### 阶段 8：失败与取消

- [ ] 将根 Intent 的不可行 IntentResult 传播为 Task 结束。
- [ ] 将嵌套不可行 IntentResult route 到 Plan Verdict。
- [ ] 区分系统失败和工具 Result error。
- [ ] 通过 Intent -> Plan/Step -> 子 Actor 传播取消。
- [ ] 取消 pending Relay，并拒绝迟到 Result。
- [ ] 保证任何等待状态都有活动依赖。

### 阶段 9：Prompt 契约

- [ ] 增加 Intent Verdict Prompt。
- [ ] 增加 Plan 重制 Verdict Prompt。
- [ ] 保持严格、精简的响应 Schema。
- [ ] 在 Intent Verdict 中包含 Step 历史。
- [ ] 在重制 Verdict 中包含 `Plan.failures`。
- [ ] 包含可复用 IntentResult 摘要。
- [ ] 校验矛盾或格式错误的 Verdict。

### 阶段 10：迁移与验证

- [ ] 保持外部 Task 请求/响应契约不变。
- [ ] 先在 Task 内部引入 Intent。
- [ ] 将现有工具 Step 迁移到 Intent Step WorkQueue。
- [ ] 将当前 Model Relay 行为从 Step 迁移到 Intent Verdict。
- [ ] 替换扁平 Plan call/model/future 流程。
- [ ] 增加多 Step Intent 测试。
- [ ] 增加并行 Invocation 测试。
- [ ] 增加顺序嵌套 Intent 测试。
- [ ] 增加 Plan 重制测试。
- [ ] 增加 Intent Result 复用测试。
- [ ] 增加过期 Relay 路由测试。
- [ ] 增加根 Intent 不可行测试。
- [ ] 运行三端嵌套 Workflow E2E。

## 推荐实施顺序

1. Protocol Result、Signature 与 Event。
2. Task 级 Intent Store。
3. 根 Intent Actor。
4. Intent Step WorkQueue 与并行 Invocation Step。
5. Intent Relay Verdict 的 Step/完成分支。
6. 顺序 IntentSignature Plan。
7. Plan 通过 Intent Routing。
8. Plan 重制与结果复用。
9. 取消、过期事件与重连。
10. 删除旧扁平 Workflow 并执行 E2E。
