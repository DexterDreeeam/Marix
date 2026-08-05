---
name: benchmark-prompt-test
description: Run the Marix stage-prompt benchmark on the Ubuntu server, compare prompt protocols, and add stage cases.
---

## Purpose

Evaluate the proposed six-stage Intent controller without modifying Marix
source code:

- `PlanningStage`
- `ToolCallingStage`
- `ReplanStage`
- `InfeasibleStage`
- `SubIntentCompleteStage`
- `CompleteStage`

The benchmark checks stage transitions, ordinary-tool selection, plan payloads,
completion summaries, and preservation of concrete facts.

## Location

The active benchmark is `benchmark/prompt/`.

The former workflow-tool benchmark is archived at `benchmark/prompt_bak/`. Do
not edit or run it unless the user explicitly asks for a historical comparison.

## Environment

**Always run model cases on the Ubuntu server. Never run them on Windows.**
The Windows host cannot reliably reach `api.deepseek.com`.

Use `deployment/win/_deploy-shared.ps1` to create the SSH context, upload
`benchmark/prompt/` to `/root/marix-stage-benchmark/`, and upload a config with
resolved model credentials to:

```text
/root/marix-stage-benchmark/marix-config.toml
```

`.temp/package/server/config.toml` is suitable after a build. The repository
`config.toml` is not, because it contains placeholders.

## Run

```bash
cd /root/marix-stage-benchmark/prompt
export MARIX_CONFIG=/root/marix-stage-benchmark/marix-config.toml

python3 run.py required --candidate candidate-008 --run-id <id> --workers 8
python3 run.py guide --candidate candidate-008 --run-id <id> --workers 12
```

Run long comparisons under `nohup`, write a log file, and poll the log. Do not
leave the local session idle while silently waiting.

`temperature` is fixed at zero in `benchmark.json`; do not remove it.

## Tool behavior

Every request sends the same ordinary tool set:

- `ToolCallingStage`: `tool_choice: required`, native ordinary tool call only.
- All other stages: `tool_choice: none`, JSON response.

Keeping tools present on non-tool stages is intentional: future production
requests can preserve the same tool-schema prefix for prompt-cache reuse.

No workflow tools exist in this benchmark.

## Case layout

```text
benchmark/prompt/cases/
├── required/
│   ├── planning/
│   ├── tool_calling/
│   ├── replan/
│   ├── infeasible/
│   ├── subintent_complete/
│   └── complete/
└── guide/
    └── (same six stage directories)
```

One case per JSON file. The stem is the case id, must be meaningful snake_case,
and may contain at most five underscore-separated segments.

Required cases are release gates; guide cases are scored by stage rate.

## Case schema

```json
{
  "id": "tool_result_missing_value",
  "stage": "complete",
  "intent": "Return both version and release date.",
  "plan": [],
  "subintent_results": [],
  "tool_calls": [
    {
      "tool": "web_fetch",
      "arguments": "https://example.org/releases",
      "result": "Version is 1.97.1; date was not found."
    }
  ],
  "failed_plans": [],
  "tool_call_count": 1,
  "expected": {
    "decision": "continue"
  }
}
```

Tool-calling cases use:

```json
{
  "expected": {
    "tools": ["web_fetch"]
  }
}
```

Completion cases can require exact facts in the summary:

```json
{
  "expected": {
    "decision": "complete",
    "summary_contains": ["api.example.org", "8443"]
  }
}
```

## Candidate protocols

Candidates live under `benchmark/prompt/prompts/`.

The benchmark intentionally compares multiple output forms:

- discriminated union (`decision`)
- stage-specific booleans
- unified `next_stage`
- requirements-first completion
- system-role versus last-user-role stage contract
- provider JSON mode versus prompt-only JSON
- fixed `Intent-*` marker tokens inside JSON

Candidate 008 is the current experimental winner: unified `next_stage`, JSON
mode, last user-message stage contract, plus explicit boundaries for
start/wait/read-output planning and unsaved side effects.

Do not port a candidate into `src/` merely because it passes once. It must pass
required and guide repeatedly at temperature zero.

## Validation

The runner verifies:

- exact abstract stage decision;
- exact ordinary tool name;
- Planning plans contain at least two subintents;
- Replan contains at least one new subintent;
- completion summaries retain required concrete values;
- requirements-first candidates produce a requirement inventory.

Run promising candidates at least three times. Report per-stage metrics and
actual failed outputs, not just the overall Boolean.

## Regeneration

```bash
python3 generate_cases.py
python3 generate_candidates.py
```

These scripts are deterministic and do not contact a model.

When adding cases, prefer stage-boundary counterexamples:

- one operation returning several values is not automatically a Plan;
- start then wait/read output is a Plan;
- retryable failure is not infeasible;
- a generated but unsaved side effect is incomplete;
- partial tool evidence must continue;
- completion summaries must preserve concrete facts.
