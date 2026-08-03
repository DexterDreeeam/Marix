---
name: benchmark-prompt-test
description: Run the Marix workflow-routing prompt benchmark on the Ubuntu server, and add or edit its cases. Use when evaluating a prompt candidate, investigating a routing defect, or adding benchmark cases.
---

## Purpose

The benchmark measures one thing: given an assembled decision prompt, does the
model select the correct native function? It replays fixed conversation
snapshots against the model and compares the selected tool against
`expected_tools`. It never runs Marix and never executes a tool.

## Execution environment

**Cases run on the Ubuntu server, never on the Windows host.** The host cannot
reach `api.deepseek.com` (TCP 443 times out), so a local run reports every case
as an `ERROR` and the numbers are meaningless.

Ship the suite over SSH using the deployment helpers:

```powershell
cd C:\r\Marix
. .\deployment\win\_deploy-shared.ps1
$ctx = New-DeploymentSshContext -RepoRoot 'C:\r\Marix'
try {
    & $ctx.ScpExe @($ctx.BaseArgs) -r 'C:\r\Marix\benchmark\prompt' `
        "root@$($ctx.HostIp):/root/marix-benchmark/"
    & $ctx.SshExe @($ctx.BaseArgs) "root@$($ctx.HostIp)" `
        'cd /root/marix-benchmark/prompt && MARIX_CONFIG=/root/marix-benchmark/marix-config.toml python3 run.py required --run-id <id> --workers 6'
}
finally { Remove-DeploymentSshContext -Context $ctx }
```

`MARIX_CONFIG` must point at a config whose model credentials are already
resolved; `.temp/package/server/config.toml` qualifies after a build, and the
repository's own `config.toml` does not because it still holds `{{PLACEHOLDER}}`
values.

A run takes minutes. Launch it under `nohup ... &` writing to a log file, then
poll that log, rather than holding one SSH session open.

## Suites

| Suite | Path | Gate |
|---|---|---|
| `required` | `benchmark/prompt/cases/required/` | every case must pass |
| `guide` | `benchmark/prompt/cases/guide/` | per-category pass rates from `benchmark.json` |

`required` holds cases distilled from observed production defects; a regression
there is a release blocker. `guide` is the broad behavioural sweep and is scored
as an aggregate rate.

```
python3 run.py required --run-id <id> [--candidate <name>] [--workers N]
python3 run.py guide --run-id <id> --batch 0 [--candidate <name>] [--workers N]
```

`guide` runs in batches of `batch_size_per_category` per category, numbered from
0 and required to run in order. Each `--run-id` is frozen: its candidate, tools,
and model are recorded in `run.json` and a suite cannot be re-run under the same
id. Use a fresh id for every attempt.

Results land in `benchmark/prompt/results/<run-id>/` and are gitignored.

## Case layout

```
benchmark/prompt/cases/
├── required/
│   ├── ordinary/
│   ├── workflow_plan/
│   ├── workflow_complete/
│   └── workflow_infeasible/
└── guide/
    └── (same four categories)
```

One case per file. The category directory is authoritative: a case's
`category` field must match the directory it lives in, and `id` must equal the
file stem. `run.py` enforces both.

## File naming

`<what_it_tests>_d<depth>.json`, snake_case, **at most 5 underscore-separated
segments including the depth segment**. The depth segment consumes one, so the
descriptive part may use at most four words.

```
read_file_d1.json                      4 segments
list_directory_recursive_d3.json       4 segments
child_done_parent_replanned_d2.json    5 segments, at the limit
```

The name must state what the case actually tests. Opaque sequence numbers such
as `ordinary_001_d1` are not acceptable. When two cases exercise the same tool
in different ways, the name carries the difference — `search_text_d1` versus
`search_text_single_file_d1`.

## Case schema

```json
{
  "id": "read_file_d1",
  "category": "ordinary",
  "depth": 1,
  "family": "read_file",
  "source": { "kind": "synthetic" },
  "overall_goal": "…the user's original request…",
  "ancestors": [
    {
      "goal": "…parent task goal…",
      "plan": [
        { "goal": "…", "status": "completed", "result": "…" },
        { "goal": "…", "status": "executing" },
        { "goal": "…", "status": "pending" }
      ],
      "completed_calls": [],
      "fail_plans": []
    }
  ],
  "current_task": "…the goal being decided right now…",
  "completed_calls": ["tool_name (args):\nresult text"],
  "fail_plans": [],
  "expected_tools": ["read_file"]
}
```

- `depth` is the nesting level: 1 means no ancestors, 4 means three ancestors.
  A `depth` of N carries N-1 entries in `ancestors`.
- `completed_calls` are calls already made **for this current task**. Ancestor
  entries carry their own separate `completed_calls`.
- `source.kind` is `telemetry` when distilled from a real run, in which case
  record the telemetry keys, session id, and the wrong tool that was observed.
  Otherwise it is `synthetic`.
- `expected_tools` is a list because a case may legitimately expect a specific
  multi-call response, but a single element is the norm.

`workflow_plan` cases are additionally checked for at least two goals, and for
not repeating any goal listed in `fail_plans`.

## Coverage rules for new cases

Add at most 50 cases per category beyond the existing 100, and keep every
category balanced across `completed_calls` being empty and non-empty.

This matters because of a real defect: the original suite had `completed_calls`
non-empty for **every** `workflow_complete` case and empty for **every**
`ordinary` and `workflow_plan` case. A model could score full marks by checking
only whether that section was empty, without reading it — which is exactly the
production failure the suite missed. Each category needs both shapes:

| Category | Empty `completed_calls` | Non-empty `completed_calls` |
|---|---|---|
| `ordinary` | nothing done yet, one call satisfies the goal | earlier call covered part of the goal, the rest still needs the same tool |
| `workflow_plan` | multi-step goal, nothing started | evidence gathered, decomposition still required |
| `workflow_complete` | ancestor facts alone already answer the goal | this task's own calls already answer the goal |
| `workflow_infeasible` | the blocker is stated in the goal | the blocker was discovered by an earlier call |

## Determinism

`run.py` pins `temperature: 0` and `tool_choice: "required"`. Do not remove
either. Without the pinned temperature the same candidate and case yield
different tools across runs, and any A/B comparison becomes noise — this
previously produced a candidate that looked like a fix and was in fact a
regression.

Confirm a suspected improvement by repeating the run; identical inputs must
produce identical per-case outcomes.

## Prompt candidates

`benchmark/prompt/prompts/candidate-*.json` hold the prompt variants. Fields
map onto production files:

| Field | Production file |
|---|---|
| `system` | `src/server/prompt/template/System.prompt` |
| `policy` | `src/server/prompt/template/WorkflowPolicy.prompt` |
| `background_header` | `src/server/prompt/module/BackgroundTaskContextHeader.prompt` |
| `current_header` | `src/server/prompt/module/CurrentTaskContextHeader.prompt` |
| `completed_notice` | `src/server/prompt/module/CompletedCalls.prompt` |

Candidates use `{{system}}` and `{{goal}}`; the production files use
`{{#system}}` and `{{#goal}}`. Normalise when porting in either direction, and
compare after converting CRLF to LF, since the production files are CRLF and
the JSON fields are LF.

A candidate is only worth porting once it holds `guide` at or above the
baseline **and** improves `required`. Changing one category's wording routinely
moves another; always report both.
