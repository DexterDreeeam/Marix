# Prompt Benchmark

This benchmark evaluates Marix workflow selection with the production-shaped
single-stage request:

- three Workflow Tools and fourteen ordinary tools are available together;
- `tool_choice` is `required`;
- the model must return native tool calls;
- no `workflow_go` or separate routing stage exists;
- tool-call summarization is not part of this benchmark.

The benchmark has two suites:

| Suite | Purpose | Pass rule |
|---|---|---|
| `smoke` | Broad synthetic coverage across workflow classes and context depths | Category thresholds |
| `practice` | Regressions captured from real failed runs | Every case must pass |

Practice is the **required prerequisite** for Smoke. Always run the complete
Practice suite first with the same Run ID and candidate. Do not start Smoke
until Practice has finished and passed 100%. If Practice is incomplete or any
Practice case fails, stop: running Smoke is unnecessary regardless of the
candidate's expected Smoke percentages.

## Required metrics

The final cumulative Smoke result must satisfy:

| Category | Minimum |
|---|---:|
| `workflow_plan` | 95% |
| `workflow_complete` | 99% |
| `workflow_infeasible` | 50% |
| Ordinary tools | 90% |

Practice has one rule: **100% of cases must pass**. A single Practice failure
fails the benchmark regardless of Smoke percentages.

Thresholds live in `benchmark.json`, not in the runner.

## Layout

```text
benchmark/prompt/
├── README.md
├── README.cn.md
├── benchmark.json
├── run.py
├── update_manifest.py
├── tools.json
├── prompts/
│   ├── candidate-007.json
│   └── candidate-023.json
├── smoke/
│   ├── generate.py
│   ├── cases.json
│   └── manifest.json
├── practice/
│   ├── cases.json
│   └── manifest.json
└── results/
    └── .gitignore
```

Generated results are intentionally ignored by Git.

## Tool surface

`tools.json` contains the fixed 17-tool production surface:

- `workflow_plan`
- `workflow_complete`
- `workflow_infeasible`
- fourteen ordinary tools

Workflow Tools are evaluated like every other supplied tool. They are not
placed in a special request, and no separate classifier is called first.

Do not modify `tools.json` in the middle of a run. The runner records its
SHA-256 and rejects subsequent batches when it changes.

## Smoke suite

Smoke contains 400 cases:

- 100 `workflow_plan`
- 100 `workflow_complete`
- 100 `workflow_infeasible`
- 100 ordinary-tool cases

Each category has 25 curated semantic families rendered at context depths 1,
2, 3, and 4. The four depths therefore contain 100 cases each.

Depth means:

1. Current Task only.
2. One parent Plan plus Current Task.
3. Two parent Plans plus Current Task.
4. Three parent Plans plus Current Task.

Parent Plans include completed work, an executing child, and later parent-only
work. This measures whether the model selects for Current Task without absorbing
parent requirements.

### Regenerating Smoke

Run from the repository root:

```powershell
python benchmark\prompt\smoke\generate.py
```

The generator writes `smoke/cases.json` and `smoke/manifest.json`. Review the
diff. Regeneration intentionally changes the suite SHA-256 when case content
changes.

## Practice suite

Practice stores failures taken from real tasks and Telemetry. Practice cases
may belong to any workflow or ordinary category, but they are not averaged:
every one must pass.

Current real-world cases include:

```text
complete-telemetry-098446153288258912138544578473665883451
ordinary-write-file-telemetry-070981585674218474111614146741974199136
ordinary-replace-file-telemetry-297338054689605191884651027567795393139
```

It reproduces Telemetry request key
`098446153288258912138544578473665883451` and response record `1921`. The
Current Task had already obtained all RFC 2324 facts, so the expected tool is
`workflow_complete`. The observed production response incorrectly created a
Plan containing repeated retrieval and parent-level file work.

The second case reproduces Model Relay request
`070981585674218474111614146741974199136` and response
`019882607918864850667207055831898822010`. Its Current Task is one
known-content file write with no Current Task completion evidence, so the
expected ordinary tool is `write_file`. The observed response incorrectly
called `workflow_complete` and claimed the file had been written.

The third case reproduces Model Relay request
`297338054689605191884651027567795393139` and response
`069872082689642533084430657110518194371`. The current implementation and
required replacement behavior are already known, so the expected ordinary tool
is `replace_in_file`. The observed response incorrectly called
`workflow_complete` while explicitly admitting that the modification remained
unfinished.

### Adding a Practice case

1. Append the full model-visible context to `practice/cases.json`.
2. Include a stable unique `id`, `category`, `depth`, `family`,
   `expected_tools`, and all rendered context data.
3. Add a `source` object with the Telemetry key, record IDs, session/task IDs,
   and observed wrong tool when available.
4. Refresh the manifest:

   ```powershell
   python benchmark\prompt\update_manifest.py practice
   ```

5. Run the entire Practice suite. Do not batch or sample Practice.

Never weaken Practice expectations to make a prompt pass. Fix the candidate or
the workflow behavior.

## Prompt candidates

A candidate JSON controls every mutable prompt section used by the benchmark:

- first System message;
- Workflow Policy System message;
- Background Context header;
- Current Task header;
- Goal and Plan headings;
- Completed Calls heading and notice;
- Fail Plans heading and reason label.

`prompts/candidate-023.json` is the current default candidate. Its completed
Run `candidate023-compact-a` scored:

| Category | Validated 400-case result |
|---|---:|
| Plan | 96% |
| Complete | 100% |
| Infeasible | 96% |
| Ordinary | 93% |

The required Practice suite also passed 3/3 before Smoke started. These results
are tied to the frozen candidate, case, tool, and model hashes recorded by that
Run ID; rerun the benchmark after changing any input.

Once a candidate starts a run, do not edit it. Create a new candidate file and
a new Run ID for another prompt combination.

## Model configuration

The runner requires Python 3.11 or later and reads the selected model from a
Marix TOML configuration:

```text
MARIX_CONFIG=<path-to-config.toml>
```

The runner never prints the API key. Because direct Windows connections to the
DeepSeek endpoint have previously failed TLS negotiation, running on the
deployed Server host is recommended.

Example on Linux:

```bash
cd /path/to/Marix
MARIX_CONFIG=/opt/marix/server/config.toml \
  python3 benchmark/prompt/run.py practice \
  --run-id candidate023-20260802 \
  --candidate candidate-023
```

## Running Practice

Practice must be run first and in full:

```powershell
$env:MARIX_CONFIG = 'C:\path\to\config.toml'
python benchmark\prompt\run.py practice `
  --run-id candidate023-20260802 `
  --candidate candidate-023
```

The command exits nonzero if any Practice case fails.

Proceed to Smoke only when this command finishes successfully with every
Practice case passing. An incomplete or failed Practice run blocks Smoke.

## Running Smoke incrementally

Smoke is optional until the required Practice suite has completed successfully
for the same Run ID and candidate. Never use Smoke results to compensate for,
or continue past, a Practice failure.

Smoke runs ten batches. Each batch contains ten cases from every category,
forty calls total.

```powershell
$env:MARIX_CONFIG = 'C:\path\to\config.toml'

python benchmark\prompt\run.py smoke `
  --run-id candidate023-20260802 `
  --candidate candidate-023 `
  --batch 0

python benchmark\prompt\run.py smoke `
  --run-id candidate023-20260802 `
  --candidate candidate-023 `
  --batch 1
```

Continue through `--batch 9` only while cumulative gates pass.

Early token-saving gates are:

| Batch | Plan | Complete | Infeasible | Ordinary |
|---:|---:|---:|---:|---:|
| 0 | 70% | 80% | 30% | 70% |
| 1 cumulative | 85% | 90% | 40% | 80% |
| 2–9 cumulative | 95% | 99% | 50% | 90% |

If a batch fails its gate, stop that candidate. Do not edit the candidate and
continue the same Run ID. Create a new candidate and Run ID.

## Frozen-run guarantees

Each Run ID creates `results/<run-id>/run.json` containing:

- candidate filename and SHA-256;
- Smoke and Practice case SHA-256 values;
- tool-list SHA-256;
- model name.

Later batches fail closed if any frozen input changes. Smoke batches must run
sequentially from 0 and cannot be overwritten. Practice can run only once per
Run ID.

## Scoring

A case passes when the returned tool-name list exactly equals
`expected_tools`.

Additional Plan checks:

- at least two goals must be returned;
- goals must not exactly repeat failed Plan goals.

Returning multiple ordinary tools fails a one-tool case. Returning the correct
Workflow Tool with an invalid Plan body also fails.

## Reports

Each invocation writes JSON and Markdown under:

```text
benchmark/prompt/results/<run-id>/
```

JSON retains calls, arguments, token usage, timing, source metadata, and failure
details. Markdown provides cumulative category metrics and failures.

Results are local artifacts and are not committed by default.

## Maintenance rules

- Keep `README.md` and `README.cn.md` synchronized.
- Never edit a candidate during a run.
- Never change Smoke or Practice cases without refreshing the corresponding
  manifest.
- Run every Practice case before Smoke; never use Practice percentages or
  sampling, and do not run Smoke when Practice is incomplete or failed.
- Keep all three Workflow Tools and ordinary tools in the same request.
- Do not add tool-call summaries to this benchmark.
- Preserve raw real-world context in Practice, correcting only secret material.
