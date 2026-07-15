# Marix Nested Workflow Redesign

## Purpose

This note defines the target architecture for replacing the current flat
Task/Plan/Step workflow with a nested Intent/Plan workflow.

`Task` remains the boundary for one user request and all of its continuations.
The rewrite is centered on these rules:

- Task stores every Intent actor.
- Task owns one root Intent whose immutable content initially equals the user
  request.
- Intent can execute many Steps over time or delegate itself to one Plan.
- Plan manages an ordered set of child Intent signatures.
- Step owns parallel Invocations.
- Relay is not a child of Intent, Plan, or Step. It is a transient model verdict
  triggered when an Intent or Plan needs a decision.

```text
Task
├── root IntentSignature
└── WorkQueue<IntentSignature, Intent>

Intent
├── immutable content
├── WorkQueue<StepSignature, Step>
├── optional Plan
└── triggers Relay verdicts when needed

Plan
└── ordered child IntentSignatures

Step
└── parallel Invocations
```

## Core Ownership

### Task

A Task represents one user request and every continuation required to complete
that request.

Task initially creates one root Intent:

```text
Task.user_request -> root Intent.content
```

Task stores every root or nested Intent:

```text
Task.intents: WorkQueue<IntentSignature, Intent>
```

Neither Plan nor Step owns Intent instances. They refer to Intents by signature,
and Task resolves the actor.

Task succeeds when the root Intent succeeds. Task ends unsuccessfully when the
root Intent returns an infeasible `IntentResult`.

### Intent

Intent is the immutable goal unit:

```text
Intent.content: String
```

Intent content never changes. Its execution strategy can evolve.

Intent stores:

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

An Intent may execute several Steps before discovering that the goal is more
complex than expected. It can then create a Plan.

Once an Intent delegates to a Plan:

- the Intent stops creating or running direct Steps;
- the Intent is driven only by that Plan;
- the Plan's terminal result resumes the Intent.

### Plan

Plan decomposes one parent Intent into ordered child Intents.

Its signature includes the parent Intent signature:

```text
PlanSignature {
    intent: IntentSignature,
    ...
}
```

Plan does not own child Intent instances. It stores only ordered signatures,
historical non-viable results, and its current terminal result:

```text
PlanState {
    signature: PlanSignature,
    intents: Vec<IntentSignature>,
    failures: Vec<PlanResult>,
    result: Option<PlanResult>,
}
```

The vector preserves model-defined order. Task remains the single Intent actor
store and provides the current status/result of every child signature.

`failures` is append-only and contains the `PlanResult` of every prior Plan
shape that was judged non-viable during reconstruction. `result` is reserved
for the current Plan's terminal result and remains `None` while the Plan is
running.

Only one child Intent runs at a time. The next child starts after the current
child succeeds.

### Step

Step is a parallel tool-call batch belonging to one Intent.

Its signature includes the Intent signature:

```text
StepSignature {
    intent: IntentSignature,
    ...
}
```

An Intent can own many executed or active Steps:

```text
Intent.steps: WorkQueue<StepSignature, Step>
```

Step owns a group of parallel Invocations. Step aggregates Invocation output and
reports one result to its Intent.

### Invocation

Invocation performs one tool call. Its signature contains its Step signature:

```text
InvocationSignature {
    step: StepSignature,
    ...
}
```

Invocation retains the current Host/Executor/Execution behavior.

Tool-returned error content is still an execution result. It is not by itself a
workflow-level failure.

### Relay

Relay is a transient model verdict mechanism.

It is not structurally owned by Intent, Plan, or Step. Intent or Plan triggers a
Relay when a decision is required, then pauses until the Relay reports back.

Relay signature contains the Intent signature:

```text
RelaySignature {
    intent: IntentSignature,
    ...
}
```

Relay lifecycle resources may remain in a Task-level runtime registry, but Relay
must not become part of the persistent Intent or Plan tree.

## Intent Verdict

Intent invokes a model Relay to choose exactly one of three outcomes:

1. **Tool execution**
   - Create another Step.
   - The Step runs one or more parallel Invocations.
   - When the Step completes, Intent may request another verdict.

2. **Plan decomposition**
   - Create a Plan containing ordered child Intent signatures.
   - Store the Plan in the Intent.
   - Stop direct Step execution for this Intent.
   - Resume only when the Plan reports a result.

3. **Complete**
   - Produce an `IntentResult`.
   - Mark the Intent terminal.

An Intent may therefore follow this path:

```text
Relay verdict -> Step -> Relay verdict -> Step -> Relay verdict -> Plan
```

After the Plan is created, the Intent cannot return to direct Step execution.

## Plan Execution

1. Plan creates child Intent actors through Task.
2. Task stores every child Intent in `Task.intents`.
3. Plan stores only their signatures in execution order.
4. Plan starts the first child Intent.
5. Plan records the successful child result and advances in signature order.
6. Plan starts the next child Intent.
7. When all child Intents succeed, Plan produces `PlanResult`.
8. Parent Intent consumes the PlanResult and completes.

## Result Types

Do not create separate failure-only structures. Successful and infeasible
outcomes are represented by the same result types in `protocol/`.

Suggested protocol-owned structures:

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

Exact fields can be refined during protocol design, but both positive and
negative outcomes use these same result types. Do not create separate failure
DTOs.

For a Plan result:

- `intents` contains the flattened child Intent content in order;
- `output` explains the achieved result or why the shape was not viable;
- `status` distinguishes successful and infeasible terminal outcomes.

Intent and Plan result types belong to `protocol/`, not server-private modules.

## Tool Errors and Intent Feasibility

A Step can contain several parallel Invocations. An Invocation can return a tool
error, but that output is information.

After Step completion, Intent triggers a Relay verdict with the accumulated Step
results. The verdict can:

- request another Step using newly learned information;
- decompose the Intent into a Plan;
- complete the Intent;
- declare the Intent infeasible.

Intent is infeasible only when the model determines that neither additional
Steps nor a Plan can complete the immutable goal.

System failures remain separate:

- broken transport;
- missing actor;
- malformed event;
- unavailable runtime;
- corrupted state.

These should not be converted into ordinary tool output.

## Plan Reconstruction

Intent content is immutable, but Plan shape may be reconstructed.

When a child Intent returns an infeasible `IntentResult`, Plan pauses and
triggers a Relay verdict.

The verdict receives:

- the parent Intent;
- the current ordered child Intent strings;
- every `PlanResult` in `Plan.failures`;
- successful child `IntentResult` records that may be reused.

The model chooses exactly one:

1. The parent Intent is infeasible.
2. Return a replacement `PlanDraft`.

Before replacing the Plan, record the current non-viable shape as a
`PlanResult` and append it to `Plan.failures`.

The replacement Plan may reuse successful previous Intents:

- match by exact normalized Intent content in the first implementation;
- resolve reusable results from Task's Intent store;
- mark reusable child Intent signatures complete;
- continue from the first child Intent without a reusable result.

If the model declares the parent Intent infeasible, Plan reports that result to
the parent Intent. The parent Intent then terminates with an infeasible
IntentResult.

## Routing

The event hierarchy follows Task's central Intent store:

```text
TaskEvent
└── Intent(IntentSignature, IntentEvent)
    ├── Step(StepSignature, StepEvent)
    │   └── Invocation(InvocationSignature, InvocationEvent)
    ├── Plan(PlanSignature, PlanEvent)
    └── Relay(RelaySignature, RelayEvent)
```

Relay events route to Intent first.

When Intent receives a Relay result:

1. If the Intent has an active, incomplete Plan, route the verdict to that Plan.
2. Otherwise, handle the verdict as the Intent's own execution decision.

This keeps Relay signatures stable around Intent identity while allowing either
Intent or its active Plan to trigger a verdict.

Plan only observes child Intent results. It does not consume Step, Invocation,
or Execution events directly.

## Signature Lineage

Required lineage:

```text
TaskSignature
└── IntentSignature
    ├── PlanSignature
    ├── StepSignature
    │   └── InvocationSignature
    └── RelaySignature
```

Rules:

- `PlanSignature` contains `IntentSignature`.
- `StepSignature` contains `IntentSignature`.
- `InvocationSignature` contains `StepSignature`.
- `RelaySignature` contains `IntentSignature`.
- Plan child lists contain `IntentSignature`, never `Intent` instances.

## Pause and Resume Semantics

Intent and Plan use explicit waiting states:

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

When Relay is triggered:

1. Set the caller to `WaitingRelay`.
2. Record the expected RelaySignature.
3. Do not start another child action.
4. Resume only for the matching Relay result.
5. Clear the expected Relay before processing the verdict.

Every waiting state must reference one active child or expected event.

## State Invariants

1. Task owns and stores every Intent actor.
2. Task owns exactly one root Intent signature.
3. Intent content is immutable.
4. Intent owns its Step WorkQueue and optional Plan.
5. Plan stores child Intent signatures, never Intent instances.
6. Plan child Intents execute sequentially.
7. Step Invocations execute in parallel.
8. Relay is transient and not part of the persistent workflow tree.
9. Intent or Plan pauses while waiting for its Relay verdict.
10. Once Intent creates a Plan, only that Plan drives the Intent.
11. Tool error output is information.
12. Plan shape may be replaced; parent Intent content may not.
13. Prior non-viable Plan shapes are stored as `PlanResult` values in
    `Plan.failures`; the current terminal outcome is stored in `Plan.result`.
14. Reused results are explicit and traceable through Task's Intent store.
15. Every terminal state is emitted once.
16. No actor may remain waiting without an active child or expected signature.

## Minimal State Ownership

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

Avoid storing duplicate Intent instances, copied Plan trees, or redundant
failure-only structures.

## Open Decisions

1. Exact fields and status enums for `IntentResult` and `PlanResult`.
2. Exact Relay verdict JSON for Step, PlanDraft, complete, and infeasible.
3. Whether Plan reconstruction mutates one Plan actor or replaces it with a new
   Plan signature.
4. Maximum Plan reconstruction attempts.
5. Exact normalization used for reusable Intent matching.
6. Whether previously successful Intent results can become stale after
   environment-changing tools.
7. Cancellation behavior while waiting for Relay.
8. Whether one Intent verdict may create one Step or multiple sequential Steps.

## TODO

### Phase 1: Protocol

- [ ] Define `IntentSignature`.
- [ ] Add `IntentSignature` to `PlanSignature`.
- [ ] Add `IntentSignature` to `StepSignature`.
- [ ] Make `InvocationSignature` derive lineage from `StepSignature`.
- [ ] Add `IntentSignature` to `RelaySignature`.
- [ ] Define `IntentStatus` and `PlanStatus` waiting/terminal states.
- [ ] Define protocol-owned `IntentResult`.
- [ ] Define protocol-owned `PlanResult`.
- [ ] Define `IntentEvent` and update nested event routing.
- [ ] Define model verdict response: Step, PlanDraft, complete, or infeasible.

### Phase 2: Task Intent Store

- [ ] Add Task root Intent signature.
- [ ] Add `WorkQueue<IntentSignature, Intent>` to Task.
- [ ] Create root Intent from user request.
- [ ] Route all Intent events through Task.
- [ ] Remove Task storage for Plan instances if no longer required.
- [ ] Resolve Plan child Intent signatures through Task.

### Phase 3: Intent Actor

- [ ] Add `Intent`, `IntentState`, and `IntentRuntime`.
- [ ] Make `Intent.content` immutable.
- [ ] Add Intent Step WorkQueue.
- [ ] Add optional Plan storage.
- [ ] Add expected Relay signature and waiting state.
- [ ] Implement the three-way Relay verdict.
- [ ] Allow several Step rounds before Plan decomposition.
- [ ] Prevent direct Step execution after Plan creation.

### Phase 4: Step and Invocation

- [ ] Redefine Step as a parallel Invocation group.
- [ ] Store Invocations in Step WorkQueue.
- [ ] Aggregate Invocation results.
- [ ] Treat tool errors as output information.
- [ ] Return Step result to Intent.
- [ ] Remove model Relay ownership from Step.

### Phase 5: Sequential Plan

- [ ] Redefine Plan child list as `Vec<IntentSignature>`.
- [ ] Add `failures: Vec<PlanResult>` for prior non-viable Plan shapes.
- [ ] Add `result: Option<PlanResult>` for the current terminal outcome.
- [ ] Start one child Intent at a time.
- [ ] Advance on successful IntentResult.
- [ ] Complete after all child Intent signatures succeed.
- [ ] Return PlanResult to parent Intent.

### Phase 6: Relay Verdict Routing

- [ ] Make Relay transient and Task-runtime managed.
- [ ] Route RelayEvent to IntentSignature.
- [ ] Forward verdict to active incomplete Plan when present.
- [ ] Otherwise process verdict in Intent.
- [ ] Enforce one expected Relay per waiting actor.
- [ ] Ignore or reject stale Relay results.

### Phase 7: Plan Reconstruction

- [ ] Append the current non-viable PlanResult to `failures` before reconstruction.
- [ ] Include parent Intent and all `failures` entries in the verdict prompt.
- [ ] Include reusable successful IntentResults.
- [ ] Parse infeasible versus replacement PlanDraft.
- [ ] Replace ordered child Intent signatures.
- [ ] Reuse exact normalized Intent results.
- [ ] Resume from the first non-reused child.
- [ ] Add a bounded reconstruction count.

### Phase 8: Failure and Cancellation

- [ ] Propagate root infeasible IntentResult to Task termination.
- [ ] Route nested infeasible IntentResult to Plan verdict.
- [ ] Keep system failures separate from tool result errors.
- [ ] Propagate cancel through Intent -> Plan/Step -> child actors.
- [ ] Cancel pending Relay and reject late results.
- [ ] Ensure no waiting state lacks an active dependency.

### Phase 9: Prompt Contracts

- [ ] Add Intent verdict prompt.
- [ ] Add Plan reconstruction verdict prompt.
- [ ] Keep response schemas strict and minimal.
- [ ] Include Step history in Intent verdict.
- [ ] Include `Plan.failures` in the reconstruction verdict.
- [ ] Include reusable IntentResult summaries.
- [ ] Validate contradictory or malformed verdicts.

### Phase 10: Migration and Verification

- [ ] Keep external Task request/response contract unchanged.
- [ ] Introduce Intent behind Task first.
- [ ] Migrate existing tool Steps into Intent Step WorkQueue.
- [ ] Move current model Relay behavior from Step to Intent verdicts.
- [ ] Replace flat Plan call/model/future flow.
- [ ] Add multi-Step Intent tests.
- [ ] Add parallel Invocation tests.
- [ ] Add sequential nested Intent tests.
- [ ] Add Plan reconstruction tests.
- [ ] Add Intent result reuse tests.
- [ ] Add stale Relay routing tests.
- [ ] Add root infeasible workflow tests.
- [ ] Run three-endpoint nested Workflow E2E.

## Recommended Implementation Order

1. Protocol results, signatures, and events.
2. Task-level Intent store.
3. Root Intent actor.
4. Intent Step WorkQueue and parallel Invocation Step.
5. Intent Relay verdict with Step/complete choices.
6. Sequential Plan of Intent signatures.
7. Plan routing through Intent.
8. Plan reconstruction and result reuse.
9. Cancellation, stale events, and reconnect behavior.
10. Remove old flat workflow and run E2E.
