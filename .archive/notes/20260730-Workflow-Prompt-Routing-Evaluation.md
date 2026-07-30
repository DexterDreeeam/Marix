# Workflow Prompt Routing Evaluation and Two-Stage Relay Proposal

## Status

- Date recorded: 2026-07-30
- Evaluation model: `deepseek-v4-flash`
- Evaluation artifacts: `.temp/prompt-eval/`
- Production status: proposal only; no production prompt or runtime code was
  changed by this evaluation
- Recommended candidate: four-way router plus a separate ordinary-tool executor
- Best aggregate router result: 282/300, 94.0%
- End-to-end two-stage Relay result: 93/100, 93.0%

This note preserves the complete reasoning, experiment history, prompts, results,
failure analysis, and proposed production architecture from the workflow prompt
evaluation.

## Objective

The original goal was to find a general workflow policy that reliably selects
one of these outcomes for the current task:

1. `workflow_plan`: the current task requires decomposition.
2. `workflow_complete`: confirmed prior results already satisfy the current
   task.
3. `workflow_infeasible`: the current task cannot be completed with available
   capabilities or any materially different plan.
4. Ordinary execution: the current task can proceed directly with a native
   tool.

The policy must decide only for `[CURRENT TASK]`. Parent plans and
`[USER OVERALL GOAL]` provide context but must not prevent a completed child task
from completing or cause the model to execute work outside the current scope.

The desired behavioral distinctions include:

- A complex task such as research from multiple sources and then writing a
  report should create a Plan.
- A single direct read, write, fetch, command, retry, or verification should
  execute directly.
- Sufficient confirmed results should complete the current task.
- A permanent capability or indispensable-input gap should be infeasible.
- A failed Plan should lead to a materially different Plan when an alternative
  remains.
- Claimed intent is not evidence that a write, installation, edit, deployment,
  or other side effect happened.

## Production Context at the Time of Evaluation

The production Decision profile exposed 17 tools in one model turn:

- Three workflow tools:
  - `workflow_plan`
  - `workflow_complete`
  - `workflow_infeasible`
- Fourteen ordinary execution tools:
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

The profile used native tool calls with `tool_choice=required`. The prompt
provided `[CURRENT TASK]`, optional `[COMPLETED CALLS]`, optional
`[FAIL PLANS]`, parent context, and `[USER OVERALL GOAL]`.

## Evaluation Harness

The Python harness lives under `.temp/prompt-eval/` and intentionally remains
outside committed source:

- `run_prompt_eval.py`: sends real native-tool requests and scores selections.
- `run_relay_eval.py`: simulates the proposed Router-to-Executor Relay.
- `tools.json`: production 17-tool schema captured for evaluation.
- `router-tools.json`: four routing tools used in the proposed first stage.
- `cases.json`: initial prompt-development cases.
- `holdout-cases.json`: first single-stage holdout cases.
- `router-cases.json`: router development matrix.
- `router-holdout-cases.json`: 20 router holdout cases.
- `relay-cases.json`: ten end-to-end two-stage Relay cases.
- `templates/*.prompt`: evaluated policy candidates.
- `report-*.md` and `relay-report-*.md`: generated reports.

Experiments ran on the remote Marix Server with its deployed DeepSeek
configuration because direct local requests to `api.deepseek.com` encountered a
TLS handshake failure. The harness read `MARIX_CONFIG` and did not print the API
key.

Requests used:

```json
{
  "thinking": {"type": "disabled"},
  "stream": false,
  "tool_choice": "required"
}
```

Selection correctness was based on the called tool name. Plan cases also
checked that newly proposed goals did not repeat forbidden failed goals exactly.

## Experiment History

### 1. Single-Stage Baseline

The initial matrix compared three prompts using all 17 production tools:

| Template | Passed | Total | Accuracy |
|---|---:|---:|---:|
| State Classifier | 27 | 30 | 90.0% |
| Current production-style prompt | 26 | 30 | 86.7% |
| Complete First | 20 | 30 | 66.7% |

The State Classifier looked promising, but the sample was small and reused
development cases.

### 2. Prompt-Only Iteration and Overfitting

Successive prompt-only versions refined ordering, mandatory Plan language,
scope, completion evidence, and failed-Plan handling. The sixth version reached
50/50 on its development matrix.

That result did not generalize. On ten unseen holdout cases repeated five times,
the same candidate achieved only 28/50, 56.0%.

| Holdout case | Passed | Runs | Dominant wrong behavior |
|---|---:|---:|---|
| Install then verify | 1 | 5 | Started a command directly |
| Read, modify, verify | 0 | 5 | Read the file directly |
| Single command action | 4 | 5 | Planned once unnecessarily |
| Installed and verified | 5 | 5 | Correctly completed |
| Downloaded but not installed | 1 | 5 | Started installation directly |
| Missing hardware | 4 | 5 | Attempted a command once |
| Failed deployment requiring replan | 0 | 5 | Listed a directory directly |
| Child complete, parent pending | 3 | 5 | Re-read instead of completing |
| Single web lookup | 5 | 5 | Correct direct execution |
| Known-content single write | 5 | 5 | Correct direct execution |

This exposed a systematic behavior: when workflow and execution tools are
available together, the model often starts the first plausible operation instead
of first classifying whether the complete current task requires a Plan.

### 3. Explicit-Action Single-Stage Prompt

A seventh single-stage prompt made multi-action examples mechanical:

- research then write
- read then modify
- install then verify
- deploy then check readiness

It still exposed all 17 tools and achieved approximately 33/50, 66%, on the
holdout matrix. More wording did not resolve the tool-choice competition.

### 4. Four-Way Router

The next design removed ordinary tools from the classification turn and exposed
only:

- `workflow_plan`
- `workflow_complete`
- `workflow_infeasible`
- `workflow_go`

`workflow_go` does not execute work. It authorizes the Server to open a separate
ordinary-tool execution turn.

Router v1 achieved:

| Matrix | Passed | Total | Accuracy |
|---|---:|---:|---:|
| Development | 96 | 100 | 96.0% |
| Holdout | 182 | 200 | 91.0% |

Its strongest stable classes were direct single operations, confirmed
completion, permanent hardware-capability gaps, and child completion independent
of unfinished parent work.

Its main failures were:

- a forbidden but indispensable missing input was not recognized as infeasible;
- some read-modify, deploy-readiness, and multi-target tasks selected Go;
- some unconfirmed side-effect cases selected Plan instead of Go;
- occasional replanning selected Go.

### 5. Router v2

Router v2 changed the rule to count "top-level actions." This made the language
shorter but allowed the model to treat a compound operation as one action.

| Matrix | Passed | Total | Accuracy |
|---|---:|---:|---:|
| Development | 95 | 100 | 95.0% |
| Holdout | 180 | 200 | 90.0% |

Notably, read-transform-write fell to 1/10 and deploy-readiness to 4/10. The
phrase "top-level action" was therefore rejected.

### 6. Router v3

Router v3 restored "distinct operations or dependent stages," explicitly
forbade collapsing sequential goal stages into one compound shell command, and
required all inputs to be known before Go.

| Matrix | Passed | Total | Accuracy |
|---|---:|---:|---:|
| Development | 93 | 100 | 93.0% |
| Holdout | 189 | 200 | 94.5% |
| Combined | 282 | 300 | 94.0% |

Important holdout outcomes:

| Case | Passed | Runs |
|---|---:|---:|
| Two independent lookups | 10 | 10 |
| Read, transform, write | 10 | 10 |
| Deploy then readiness check | 6 | 10 |
| Single status command | 10 | 10 |
| Single delete | 10 | 10 |
| Single fetch | 10 | 10 |
| One step left after complex work | 10 | 10 |
| Verify after installation | 9 | 10 |
| Verify after write | 10 | 10 |
| Factual completion | 10 | 10 |
| Side-effect completion | 10 | 10 |
| Claimed but unconfirmed side effect | 7 | 10 |
| Alternative after two failed plans | 10 | 10 |
| Transient retry | 10 | 10 |
| Permanent hardware gap | 10 | 10 |
| Forbidden indispensable input | 7 | 10 |
| Failed Plan but direct solution now exists | 10 | 10 |
| Child complete while parent still has work | 10 | 10 |
| Multiple independent targets | 10 | 10 |
| One compound filesystem operation | 10 | 10 |

Router v3 was selected as the best balanced prompt, not because it was perfect,
but because it generalized better than v1 and v2 on the unseen matrix.

### 7. End-to-End Two-Stage Relay

The final test simulated the complete transition:

```text
Current Task
    |
    v
Router turn: 4 routing tools only
    |
    +-- workflow_plan       -> create/validate Plan
    +-- workflow_complete   -> finish Current Task
    +-- workflow_infeasible -> fail Current Task
    +-- workflow_go         -> Executor turn
                                  |
                                  v
                         14 ordinary tools only
```

Ten cases were repeated ten times:

| Case | Passed | Runs | Expected execution |
|---|---:|---:|---|
| Read file | 10 | 10 | `read_file` |
| Write file | 10 | 10 | `write_file` |
| Fetch web page | 10 | 10 | `web_fetch` |
| Search local text | 7 | 10 | `search_text` |
| Run status command | 10 | 10 | `powershell` |
| Verify written file | 10 | 10 | `read_file` |
| Complex task | 10 | 10 | `workflow_plan` |
| Completed factual task | 9 | 10 | `workflow_complete` |
| Permanent capability gap | 10 | 10 | `workflow_infeasible` |
| Replan after failure | 7 | 10 | `workflow_plan` |

Overall: 93/100, 93.0%.

Whenever the Router correctly selected Go, the Executor was highly reliable for
the tested read, write, fetch, PowerShell, and verification operations. The
search failures happened in the Router, which unnecessarily selected Plan; the
Executor did not select the wrong native tool.

## Recommended Architecture

### Decision 1: Separate Routing from Execution

Do not expose Workflow Tools and ordinary execution tools in the same decision
turn.

Use two profiles:

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
  tools = all ordinary tools allowed for this task
```

The Server, not the model, owns the transition between profiles.

### Decision 2: Make `workflow_go` a Routing Verdict

`workflow_go` means:

> The current task should proceed without creating a Plan. Open an execution
> turn with ordinary tools.

It must not contain or perform the intended ordinary operation. This preserves a
strict control boundary and prevents a mixed workflow/execution response.

Suggested schema:

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

### Decision 3: Keep Workflow State in the Server FSM

The Server should enforce:

- Router turns can call exactly one routing tool.
- Executor turns cannot see or call Workflow Tools.
- A Plan must contain at least two clear sub-goals.
- Every sub-goal must remain within the current-task scope.
- A new Plan must not repeat or closely resemble a failed Plan.
- Complete must be supported by confirmed evidence relevant to the current task.
- Side-effect completion requires matching execution evidence, not assistant
  narration.
- Infeasible requires a permanent capability/input constraint or exhausted
  materially different plans.
- Go only transitions state; it does not imply success.
- Repeated identical invocations remain subject to the existing Server-side hard
  limit.

If a model returns an invalid routing verdict, the Server should reject it with
a precise state error and request another routing decision. It should not
silently reinterpret the verdict or expose ordinary tools as a fallback in the
same turn.

### Decision 4: Return to the Router After Execution

After an ordinary tool result:

1. Append the confirmed result to `[COMPLETED CALLS]`.
2. Re-enter the Router profile.
3. Let the Router select Complete, Plan, Infeasible, or Go for the remaining
   current task.

This creates a bounded state machine:

```text
ROUTE
  -> PLAN
  -> COMPLETE
  -> INFEASIBLE
  -> EXECUTE_ONE
       -> ROUTE
```

The model never decides both a workflow transition and an ordinary operation in
the same response.

### Decision 5: Treat Verification as Evidence, Not Prose

Examples:

| Current-task requirement | Sufficient completion evidence |
|---|---|
| Return a factual value | Confirmed source/tool result containing the value |
| Write a file | Successful matching write result |
| Write and verify a file | Successful write plus matching read-back |
| Install software | Successful installer result |
| Install and verify | Installer result plus version/status verification |
| Deploy and become ready | Deployment/start result plus readiness probe |

An assistant sentence such as "I installed it" or "I will restart it" is not
evidence.

## Recommended Router Prompt

Router v3 is the recommended starting candidate:

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

## Recommended Executor Prompt

```text
EXECUTE ONLY THE [CURRENT TASK] NOW. THE ROUTER HAS ALREADY DETERMINED THAT ONE DIRECT OPERATION IS SUFFICIENT. CALL EXACTLY ONE SUPPLIED ORDINARY TOOL WITH ARGUMENTS THAT SATISFY THE TASK. RETURN ONLY THE NATIVE TOOL CALL.

[**USER OVERALL GOAL**]
{{#goal}}
```

## Other Evaluated Router Prompts

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

## Why Prompt-Only Routing Is Insufficient

The tests do not show that Router v3 is deterministic. It still failed 18 of
300 router decisions and 7 of 100 end-to-end Relay runs.

Observed ambiguity remains around:

- deploy/start/readiness being compressed into one operation;
- replanning after failed approaches;
- distinguishing an impossible required input from an action that might still
  be attempted;
- deciding whether an unconfirmed side effect needs one direct retry or a Plan;
- treating recursive search as one direct operation.

Therefore the recommendation is not "replace the production prompt with v3."
The recommendation is:

1. separate Router and Executor tool surfaces;
2. use v3 as the Router behavioral contract;
3. validate transitions in the Server FSM;
4. reject invalid verdicts explicitly;
5. continue evaluating with independent holdouts and real telemetry.

## Production Rollout Plan

This rollout was not implemented during the evaluation.

1. Add `workflow_go` as an internal routing tool.
2. Add distinct Router and Executor prompt profiles.
3. Ensure the Router profile exposes only four routing tools.
4. Ensure the Executor profile exposes only ordinary tools.
5. Route Go to one ordinary execution turn and then back to Router.
6. Add Server-side validation for Plan, Complete, Infeasible, and Go.
7. Record routing verdict, rejected verdict reason, selected ordinary tool, and
   subsequent routing verdict in telemetry.
8. Replay the existing evaluation matrices against the integrated Server.
9. Run broader holdouts with at least ten repetitions per class.
10. Run deterministic smoke cases and inspect Relay convergence before replacing
    the production Workflow Policy.

## Additional Cases Required Before Adoption

- Multiple independent reads that can run in parallel.
- One PowerShell invocation containing several shell statements but only one
  semantic goal.
- One PowerShell invocation attempting to hide multiple dependent goal stages.
- Reversible versus irreversible side effects.
- An already active valid Plan.
- Partial Plan completion with one remaining child.
- Several failed Plans with and without a viable alternative.
- Temporary network/tool failures versus permanent capability gaps.
- Completion based on source evidence versus completion requiring side-effect
  evidence.
- Tool success followed by failed verification.
- Child completion while parent work remains.
- Repeat-invocation limit interaction.
- Summary cursor/chunk continuation interaction.

## Final Conclusion

The central finding is architectural:

> A model is less reliable when asked to classify workflow state and choose from
> many attractive execution tools in the same turn.

Prompt wording alone reached 100% on a development set and then only 56% on
holdout cases. Reducing the first turn to four explicit routing choices produced
94% across 300 router decisions, and the full Router-to-Executor simulation
produced 93%.

The strongest next design is a Server-controlled two-stage Relay:

```text
classify with four routing tools
        ->
execute with ordinary tools only when routed to Go
        ->
return confirmed results to the Router
```

This design narrows model choice, makes transitions observable, prevents mixed
workflow/execution responses, and gives the Server a deterministic place to
enforce scope, evidence, failed-Plan, retry, and completion rules.
