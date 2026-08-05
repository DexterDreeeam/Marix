# Intent stage prompt benchmark

This benchmark tests a proposed six-stage Intent controller without changing
Marix source code:

1. `PlanningStage`
2. `ToolCallingStage`
3. `ReplanStage`
4. `InfeasibleStage`
5. `SubIntentCompleteStage`
6. `CompleteStage`

The benchmark sends ordinary tool schemas on every request. Only
`ToolCallingStage` uses `tool_choice: required`; every other stage uses
`tool_choice: none` and returns JSON. This preserves an identical tool surface
for future prompt-cache reuse while preventing non-tool stages from invoking
tools.

## Run

Run on the Ubuntu server, not the Windows host:

```bash
export MARIX_CONFIG=/root/marix-stage-benchmark/marix-config.toml
python3 run.py required --candidate candidate-008 --run-id check-001
python3 run.py guide --candidate candidate-008 --run-id check-001
```

`temperature` is pinned to zero in `benchmark.json`.

## Suites

- `required`: two critical cases per stage; all must pass.
- `guide`: broader stage boundaries and summary-preservation cases.

Each case is one JSON file under:

```text
cases/<suite>/<stage>/<meaningful_name>.json
```

The file stem is the case id, must be snake_case, and may contain at most five
underscore-separated segments.

## Candidate protocols

| Candidate | Protocol | Result |
|---|---|---|
| 001 | stage-specific discriminated-union JSON | 42/44 before refinement |
| 002 | stage-specific boolean JSON | loses concrete summary values |
| 003 | unified `next_stage` JSON | 42/44 before refinement |
| 004 | requirements-first completion JSON | good evidence, compressed summaries |
| 005 | union contract as a system message | over-conservative replanning |
| 006 | union JSON without provider JSON mode | planning boundary regression |
| 007 | fixed `Intent-*` token inside JSON | weaker planning and summaries |
| **008** | **unified `next_stage` JSON with refined boundaries** | **56/56, repeated three times** |
| 009 | refined discriminated-union JSON | 55/56 |

Candidate 008 maps naturally onto a future `IntentRuntime::run_stage`: the
runtime dispatches the returned `next_stage`, while stage-specific payloads
carry `subintents`, `context_summary`, or `summary`.

## Regenerate

```bash
python3 generate_cases.py
python3 generate_candidates.py
```

Generation is deterministic and does not call a model.
