# {{proj}} Host, Client, and Agent Rewrite Notes

## Scope

The next source rewrite should organize the project around three core libraries:

- `host`
- `client`
- `agent`

These libraries may run on three different devices, or any two or three of them
may be linked into the same process or deployed on the same device. The design
should keep their boundaries explicit so deployment topology stays flexible.

The previous source tree has been archived at
`{{repo_root}}\.archive\src.20260702`.

This note only describes the three core libraries and their communication
contracts. Existing reusable areas such as model backend implementations,
configuration loading, external dependency wrappers, logging, and tests should be
carried forward where appropriate and do not need to be redesigned here.

## Core Roles

### Host

The host is the controlled environment. Tool execution, file changes, process
management, and other machine-affecting operations happen on the host.

The host owns `HostSession` and `Executor`.

`Executor` collects the tools available on that host when it is constructed. Each
tool should be built as an independently runnable program so the host process and
tool processes remain isolated.

### Client

The client is the user-facing control entrypoint. Users send instructions through
the client and receive streamed task/tool/model updates through the client.

The client owns `ClientSession`. `ClientSession` connects to the agent `Session`
through the network channel. It also exposes a user-facing event channel whose
event type is `ClientEvent`.

### Agent

The agent performs task orchestration, step scheduling, model interaction, and
routing between client and host.

The agent owns the central `Session`. The agent `Session` accepts connections
from both `ClientSession` and `HostSession`. The agent session should be
reconnectable: when one `ClientSession` disconnects, the agent `Session` remains
alive and can accept a later client connection.

## Session-Level Communication

`ClientSession` and `HostSession` connect to the agent `Session` through network
channels.

The cross-session transport event type is `SessionEvent`. It may contain
namespaced sub-events for task and tool communication, but the top-level session
boundary should stay responsible for routing rather than doing task or tool work
directly.

## Host Communication Flow

The host `Executor` receives tool-related `SessionEvent`s from the agent and sends
tool-related `SessionEvent`s back to the agent.

Expected tool event flow:

| Direction | Event | Purpose |
| --- | --- | --- |
| Agent -> Host | `SessionEvent::Tool::PreviewQuery` | Ask the host for available tool summaries. |
| Host -> Agent | `SessionEvent::Tool::Preview` | Return the available tool preview list. |
| Agent -> Host | `SessionEvent::Tool::ExecutionEvoke` | Start one tool execution. |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Started` | Report that execution started. |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Failed` | Report that execution could not start or failed. |
| Agent -> Host | `SessionEvent::Tool::ExecutionQuery` | Query the current execution status. |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::*` | Return the current status. |
| Agent -> Host | `SessionEvent::Tool::ExecutionCancel` | Request graceful cancellation. |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Canceled` | Report graceful cancellation. |
| Agent -> Host | `SessionEvent::Tool::ExecutionKill` | Force-kill a tool process. |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus::Killed` | Report forced termination. |
| Host -> Agent | `SessionEvent::Tool::ExecutionStatus` | Push important execution status changes. |
| Host -> Agent | `SessionEvent::Tool::ExecutionUpdate` | Push user-visible execution output updates. |

The agent should not need a stream of every intermediate tool status for its own
state machine. It needs terminal status, current status on query, and
user-visible execution updates that can be forwarded to the client.

## Client Communication Flow

The client has one agent-facing connection: `ClientSession`.

`ClientSession` also exposes a user-facing channel that emits `ClientEvent`.
External user interfaces consume `ClientEvent` to show task progress, model
output, tool execution updates, and final status.

Expected task event flow:

| Direction | Event | Purpose |
| --- | --- | --- |
| Client -> Agent | `SessionEvent::Task::Create` | Create a task from a user request. |
| Agent -> Client | `SessionEvent::Task::Status::Started` | Confirm the task started and provide `TaskSignature`. |
| Agent -> Client | `SessionEvent::Task::CreateFailed` | Report task creation failure. |
| Agent -> Client | `SessionEvent::Task::Status::Update` | Stream current task output or progress. |
| Client -> Agent | `SessionEvent::Task::Query` | Query a task preview/current state. |
| Agent -> Client | `SessionEvent::Task::Preview` | Return task preview/current state. |
| Client -> Agent | `SessionEvent::Task::Cancel` | Request task cancellation. |
| Agent -> Client | `SessionEvent::Task::Status::Canceled` | Report task cancellation. |
| Agent -> Client | `SessionEvent::Task::Status::Succeed` | Report task success. |
| Agent -> Client | `SessionEvent::Task::Status::Failed` | Report task failure. |
| Agent -> Client | `SessionEvent::Tool::ExecutionStatus` | Forward tool execution status relevant to the task. |
| Agent -> Client | `SessionEvent::Tool::ExecutionUpdate` | Forward user-visible tool execution output. |

`TaskSignature` should contain a human-readable task name and a GUID.

## WorkQueue

`WorkQueue<K, V>` is a generic, thread-safe work container.

Internally it owns two RB-tree key-value maps:

- `working`
- `complete`

Required interface:

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

Rules:

- `insert(K, V)` inserts into `working`.
- `insert(K, V)` panics if the key already exists.
- `insert_or_update(K, V)` inserts or updates and returns whether an existing
  value was updated.
- `complete(K)` moves one item from `working` to `complete`.
- The whole structure must be thread-safe.

## Agent Internals

### Session

The agent `Session` is the core agent module.

It owns:

- `ClientSender`
- `ClientReceiver`
- `HostSender`
- `HostReceiver`
- `WorkQueue<TaskId, Task>`

The session starts a thread that blocks on incoming `SessionEvent`s from the
client and host channels.

Most `SessionEvent`s from host and client that belong to a specific task should
carry `TaskId`. The session uses `TaskId` to find the owning `Task` and forwards
the event to that task's channel.

The session creates each task and starts that task's thread.

### Task

`Task` is the agent-side resource scheduling unit.

It is created by `Session`.

It owns:

- a model backend instance built according to configuration,
- `ClientSender`,
- `HostSender`,
- a task receiver channel for routed `SessionEvent`s,
- `WorkQueue<ExeId, Execution>`,
- `WorkQueue<int, Step>`.

The task starts its own thread when created. The thread blocks on the receiver
channel for routed session events.

`Task` must expose a `sender()` interface so `Session` can get the task sender
and route task-specific events to it.

The task thread can send commands to host and client through forked sender
handles from the central agent session channels.

### Step

`Step` is the smallest execution unit.

Step kinds:

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

`Task::raise(step)` should:

1. insert the step into `WorkQueue<int, Step>` using the step sequence number as
   key,
2. spawn a worker thread for the step,
3. execute the step operation.

If a step must wait for routed `SessionEvent`s to complete, the worker should end
immediately after issuing the external request. Later progress is driven by the
task receiver thread. Otherwise, the worker waits for the operation to finish and
then exits.

All parameters needed to execute or resume the step should live in the `Step`
structure.

## Host Internals

The host `Executor` is the host-side execution core.

On construction, it discovers or registers all available tools on the host.

Each tool should be compiled into an independently runnable program. This keeps
the host process isolated from individual tool processes and makes force-kill
semantics clear.

The executor tracks active tool executions, supports status query, graceful
cancel, force kill, and emits execution status/update events to the agent.
