# {{proj}} Host、Client 与 Agent 重写说明

## 范围

下一版源码应该围绕 3 个核心 lib 组织：

- `host`
- `client`
- `agent`

这 3 个 lib 可以运行在 3 台不同设备上，也可以任意组合在同一台设备或同一个进程里。设计上应该保持边界清晰，方便未来自由组合部署拓扑。

旧源码树已经归档到：

`{{repo_root}}\.archive\src.20260702`

本文档只说明 3 个核心 lib 的用途和交互方式。类似 model backend、config、external、logging、tests 等已有可复用模块，可以在重写时继续沿用，不在本文档里重复展开。

## 核心角色

### Host

Host 是受控制的环境。工具执行、文件修改、进程管理等会影响机器状态的操作都发生在 Host。

Host 持有 `HostSession` 和 `Executor`。

`Executor` 在构造时收集当前 Host 可用的工具。每个 Tool 应该通过编译生成独立可运行程序，让 Host 进程和 Tool 进程完全隔离。

### Client

Client 是用户控制 Agent 的入口。用户通过 Client 发送指令，并通过 Client 接收 task、model、tool 的执行状态和输出。

Client 持有 `ClientSession`。`ClientSession` 通过网络 channel 连接到 Agent 的 `Session`。同时，`ClientSession` 对外部用户暴露一个用户侧 channel，该 channel 的事件类型是 `ClientEvent`。

### Agent

Agent 负责任务调度、Step 调度、模型交互，以及 Client 与 Host 之间的事件路由。

Agent 持有核心 `Session`。Agent `Session` 可以接收 `ClientSession` 和 `HostSession` 的连接。Agent `Session` 应该支持重新连接：某一个 `ClientSession` 断开后，Agent `Session` 仍然存活，并可以接收后续新的 Client 连接。

## Session 级通信

`ClientSession` 和 `HostSession` 都通过网络 channel 连接到 Agent `Session`。

跨 Session 传递的顶层事件类型是 `SessionEvent`。它可以包含 task/tool 的命名空间子事件，但顶层 Session 边界应该主要负责连接生命周期和路由，不应该直接执行 task 或 tool 的内部逻辑。

## Host 通信流程

Host `Executor` 接收 Agent 发来的 tool 相关 `SessionEvent`，并向 Agent 返回 tool 相关 `SessionEvent`。

预期 tool 事件流：

| 方向 | 事件 | 用途 |
| --- | --- | --- |
| Agent -> Host | `SessionEvent::Tool::PreviewQuery` | 查询 Host 上可用 Tool 的概况。 |
| Host -> Agent | `SessionEvent::Tool::Preview` | 返回可用 Tool 的 preview 列表。 |
| Agent -> Host | `SessionEvent::Tool::ExecutionEvoke` | 启动一次 Tool 执行。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Started` | 告知 Agent 执行已开始。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Failed` | 告知 Agent 启动失败或执行失败。 |
| Agent -> Host | `SessionEvent::Tool::ExecutionQuery` | 查询某次 Tool 执行的当前状态。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::*` | 返回某次 Tool 执行的当前状态。 |
| Agent -> Host | `SessionEvent::Tool::ExecutionCancel` | 请求温和取消某次 Tool 执行。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Canceled` | 返回取消结果。 |
| Agent -> Host | `SessionEvent::Tool::ExecutionKill` | 强制 kill 某个 Tool 进程。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Killed` | 返回强制终止结果。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus` | 主动推送重要执行状态变化。 |
| Host -> Agent | `SessionEvent::Tool::ExecutionUpdate` | 主动推送用户可见的执行输出。 |

Agent 不需要消费每一段中间 tool status 流来驱动内部状态机。Agent 需要的是：终态、按需查询到的当前状态，以及可转发给 Client 的用户可见执行更新。

## Client 通信流程

Client 只有一个面向 Agent 的连接：`ClientSession`。

`ClientSession` 同时暴露一个用户侧 channel，用于向外部 UI 或用户入口发送 `ClientEvent`。外部用户界面通过 `ClientEvent` 展示 task 进度、model 输出、tool 执行更新和最终状态。

预期 task 事件流：

| 方向 | 事件 | 用途 |
| --- | --- | --- |
| Client -> Agent | `SessionEvent::Task::Create` | 根据用户请求创建 task。 |
| Agent -> Client | `SessionEvent::Task::Status::Started` | 确认 task 已开始，并返回 `TaskSignature`。 |
| Agent -> Client | `SessionEvent::Task::CreateFailed` | 返回 task 创建失败。 |
| Agent -> Client | `SessionEvent::Task::Status::Update` | 流式返回当前 task 输出或进度。 |
| Client -> Agent | `SessionEvent::Task::Query` | 查询 task preview 或当前状态。 |
| Agent -> Client | `SessionEvent::Task::Preview` | 返回 task preview 或当前状态。 |
| Client -> Agent | `SessionEvent::Task::Cancel` | 请求取消 task。 |
| Agent -> Client | `SessionEvent::Task::Status::Canceled` | 返回 task 已取消。 |
| Agent -> Client | `SessionEvent::Task::Status::Succeed` | 返回 task 成功。 |
| Agent -> Client | `SessionEvent::Task::Status::Failed` | 返回 task 失败。 |
| Agent -> Client | `SessionEvent::Tool::ExecutionStatus` | 转发与当前 task 相关的 tool 执行状态。 |
| Agent -> Client | `SessionEvent::Tool::ExecutionUpdate` | 转发用户可见的 tool 执行输出。 |

`TaskSignature` 应该包含一个用户可读的 task 名称和一个 GUID。

## WorkQueue

`WorkQueue<K, V>` 是一个泛型、线程安全的工作队列结构。

内部包含两个 RBTree KV map：

- `working`
- `complete`

需要的接口：

```text
complete_list() -> [V]
working_list() -> [V]
complete_size() -> usize
working_size() -> usize
size() -> usize
insert(K, V)
insert_or_update(K, V) -> bool
get(K) -> V
complete(K)
```

规则：

- `insert(K, V)` 默认插入 `working`。
- `insert(K, V)` 如果 key 已存在则 panic。
- `insert_or_update(K, V)` 插入或更新，并返回是否更新了已有值。
- `complete(K)` 把一个 item 从 `working` 移动到 `complete`。
- 整个结构必须线程安全。

## Agent 内部设计

### Session

Agent `Session` 是 Agent 的核心模块。

它持有：

- `ClientSender`
- `ClientReceiver`
- `HostSender`
- `HostReceiver`
- `WorkQueue<TaskId, Task>`

`Session` 会启动一个线程，以阻塞方式监听来自 Client 和 Host channel 的 `SessionEvent`。

来自 Host 和 Client 的大多数 `SessionEvent` 如果属于某个具体 task，都应该携带 `TaskId`。`Session` 根据 `TaskId` 找到对应 `Task`，并把事件转发到该 `Task` 的 channel。

`Session` 负责创建每个 `Task`，并启动该 `Task` 的线程。

### Task

`Task` 是 Agent 侧具体执行资源调度的核心单元。

它由 `Session` 创建。

它持有：

- 根据 config 创建的 model backend 实例，
- `ClientSender`,
- `HostSender`,
- 接收 Session 路由事件的 task receiver channel，
- `WorkQueue<ExeId, Execution>`,
- `WorkQueue<int, Step>`。

`Task` 创建后会启动自己的线程。该线程阻塞监听从 `Session` 路由过来的事件。

`Task` 应该提供 `sender()` 接口，让 `Session` 可以拿到该 Task 的 sender 并向它路由 task 相关事件。

`Task` 线程可以通过从 Agent `Session` 的核心 channel fork 出来的 sender，向 Host 和 Client 发送指令。

### Step

`Step` 是最小执行单元。

Step 类型：

```text
StepKind::Tool::Evoke
StepKind::Tool::Query
StepKind::Tool::Kill
StepKind::Model::Initial
StepKind::Model::UserIntentAnalysis
StepKind::Model::ContentSummarization
StepKind::Model::TaskPlanning
StepKind::Model::ResponseComposition
StepKind::User::Decision
StepKind::User::Authorize
```

`Task::raise(step)` 应该：

1. 按 Step 的 sequence number，把 Step 插入 `WorkQueue<int, Step>`；
2. 为该 Step spawn 一个 worker thread；
3. 执行该 Step 对应操作。

如果某个 Step 需要等待后续 `SessionEvent` 路由才能完成，该 worker 在发出外部请求后应该立刻结束，后续进度由 task receiver thread 驱动。否则，该 worker 应该等待执行完成后再结束。

执行或恢复 Step 所需的参数都应该存放在 `Step` 结构里。

## Host 内部设计

Host `Executor` 是 Host 侧执行核心。

构造时，它会发现或注册 Host 上所有可用 Tool。

每个 Tool 都应该编译为独立可运行程序。这样 Host 进程与每个 Tool 进程相互隔离，force-kill 语义也更清晰。

Executor 负责跟踪 active tool executions，支持状态查询、温和取消、强制 kill，并向 Agent 发送 execution status/update 事件。
