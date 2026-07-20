# Marix deterministic E2E tasks

## Scope

`tasks.json` defines six repeatable tasks: two network searches, two isolated local-code tasks, and two image-understanding probes. Each task has a unique directory under `.github\e2e\workspaces`. Network tasks forbid reading `.credential` files. Image tasks intentionally record the current expected vision capability gap.

Schema version 2 adds two nullable TaskRequest guardrails to every case:

- `max_completion_time_secs`: maximum task completion time;
- `max_relay_count`: maximum relay count.

The schema default for either field is `null`, meaning `None` and unlimited. For a non-null value, Server clamps completion times below 10 seconds to 10 seconds and relay counts below 5 to 5. Client and the smoke harness pass configured values unchanged; they do not repeat Server clamping. The current six cases use explicit smoke budgets rather than deliberately tiny values, and no dedicated negative guardrail case is included.

Estimated task time and `suggested_outer_watchdog_minutes` are metadata only. An outer watchdog is solely a safety fallback for a runaway agent or harness process. It neither replaces the two TaskRequest guardrails nor determines case success.

## Prerequisites

- Run from `C:\r\Marix` in PowerShell.
- Ensure Marix Server Telemetry, Server, and Host are ready, and the client configuration is valid.
- Install Rust only for `code-edit-rust-slugify`.
- Do not place credentials in a task workspace or include them in prompts or results.

## Prepare a fixture

Select one task and run its `setup.commands` from `tasks.json` in order. Setup always deletes only that task's workspace, then creates or copies a fresh fixture. Never run two instances with the same task ID concurrently.

Example:

```powershell
Set-Location C:\r\Marix
$task = (Get-Content .github\e2e\tasks.json -Raw | ConvertFrom-Json).tasks |
  Where-Object id -eq 'code-edit-rust-slugify'
$task.setup.commands | ForEach-Object { Invoke-Expression $_ }
```

Do not run a code task against `.github\e2e\fixtures` directly. The fixture is the immutable baseline. Image setup copies `scene.png`; network setup creates an empty workspace.

## Run manually

Pass the selected prompt and both configured guardrails to the oneshot client:

```powershell
$cli = 'C:\MarixClient\Cli\marix-client-cli.exe'
$arguments = @('--oneshot')
if ($null -ne $task.max_completion_time_secs) {
  $arguments += @('--max-completion-time-secs', "$($task.max_completion_time_secs)")
}
if ($null -ne $task.max_relay_count) {
  $arguments += @('--max-relay-count', "$($task.max_relay_count)")
}
$arguments += $task.prompt
& $cli @arguments
if ($LASTEXITCODE -ne 0) { throw "oneshot failed: $LASTEXITCODE" }
```

The CLI flags are `--max-completion-time-secs` and `--max-relay-count`. Omitting a flag for `null` preserves the corresponding TaskRequest value as `None`. The oneshot client waits for the terminal outcome. For unattended runs, a harness may use `suggested_outer_watchdog_minutes` only as a process-runaway fallback; report its activation as an environment error, separately from task and assertion failures.

## Run with the Smoke Agent

Select the `tester-of-e2e-smoke` custom agent and ask it to run all deterministic E2E smoke cases, or name a subset. The agent reads `tasks.json`, executes cases serially in array order, and performs setup, oneshot submission, terminal-state waiting, criteria validation, result collection, and cleanup for each case. It does not deploy or start services, modify fixture baselines, run git, read `.credential`, or run cases concurrently.

Do not ask the Smoke Agent to test the guardrail gates themselves. Its goal is only to judge whether each smoke case reaches its declared expected outcome.

## Judge the result

Apply every entry in `success_criteria` and fail on any `failure_criteria`:

1. Parse JSON artifacts with `Get-Content <path> -Raw | ConvertFrom-Json`.
2. Compare JSON semantically, not by whitespace or property order.
3. For `sha256_unchanged`, use `Get-FileHash -Algorithm SHA256`.
4. For the Rust task, run `cargo test --quiet` in its workspace and require exit code 0.
5. Check that only `allowed_changed_paths` changed relative to the copied fixture.
6. For network tasks, validate URL host groups, dates, source agreement, and live-access evidence. A network outage is an environmental failure.
7. Image tasks pass only when an image-capable tool actually inspected the PNG. The expected current outcome is a capability-gap report, not a fabricated answer.

Expected values are intentionally present in `tasks.json` for the harness. The image prompts tell the agent not to read that file; isolate prompt execution from the judge when possible.

## Result classification

Each case reports its duration, configured maximum time and relay count, terminal-state summary, assertion results, cleanup result, and one primary status:

- `PASS`: successful terminal outcome and all criteria pass.
- `FAIL`: task failure or assertion failure; the report distinguishes these subtypes.
- `UNSUPPORTED`: a required capability is unavailable under the case's `current_support` rule. Missing image capability is reported here, never fabricated as success.
- `ENVIRONMENT_ERROR`: setup, prerequisites, Client/transport, service availability, network infrastructure, harness, or cleanup prevents a reliable judgment.

The final report totals every status and total elapsed time. Environment failures, task failures, and assertion failures remain separately identified.

## Isolation and cleanup

Run only inside the task's declared `working_directory`. Do not allow access to repository `src\`, `overview\`, `.git\`, or credential files. Snapshot the fresh workspace before execution when enforcing changed-path rules.

After collecting logs and artifacts, run the selected task's `cleanup` commands. Cleanup removes only its unique workspace. To reset all test state after no tasks are running:

```powershell
Remove-Item -Recurse -Force .github\e2e\workspaces -ErrorAction SilentlyContinue
```

Keep fixtures unchanged between runs.
