# Marix deterministic E2E tasks

## Scope

`tasks.json` defines six repeatable tasks: two network searches, two isolated local-code tasks, and two image-understanding probes. Each task now uses a unique VM workspace under `C:\MarixE2E\workspaces\<case-id>`. Image tasks intentionally record the current expected vision capability gap.

Schema version 3 keeps `repository_root` only for locating immutable host-side fixture sources under `.github\e2e\fixtures\...`, and adds VM-targeting fields:

- `vm_name`: the fixed Hyper-V guest that hosts Marix Host tool execution;
- `vm_workspace_root`: the fixed guest-side E2E workspace root;
- `vm_working_directory`: the per-case absolute workspace inside the guest.

The two nullable TaskRequest guardrails remain unchanged:

- `max_completion_time_secs`: maximum task completion time;
- `max_relay_count`: maximum relay count.

The schema default for either field is `null`, meaning `None` and unlimited. For a non-null value, Server clamps completion times below 10 seconds to 10 seconds and relay counts below 5 to 5. Client and the smoke harness pass configured values unchanged; they do not repeat Server clamping. The current six cases use explicit smoke budgets rather than deliberately tiny values, and no dedicated negative guardrail case is included.

Estimated task time and `suggested_outer_watchdog_minutes` are metadata only. An outer watchdog is solely a safety fallback for a runaway agent or harness process. It neither replaces the two TaskRequest guardrails nor determines case success.

## VM-hosted execution model

Marix native tools such as `write_file`, `web_search`, and `cargo test` run only through Host, and Host is deployed only inside the Hyper-V VM `Marix_TestVm`. The physical machine still runs the deployed Client CLI at `C:\MarixClient\Cli\marix-client-cli.exe`, but every task workspace operation, artifact read, hash check, and validation command must target the guest filesystem through PowerShell Direct.

The dedicated guest E2E root is `C:\MarixE2E\workspaces`. It is intentionally isolated from Host's deployment directory `C:\MarixHost\`; smoke workspaces must never read from or write to `C:\MarixHost\`.

## Prerequisites

- Run from `C:\r\Marix` in PowerShell.
- Ensure Marix Server Telemetry, Server, and Host are ready, and the client configuration is valid.
- Reuse the fixed VM name, credential construction pattern, and `Copy-VMFile` usage defined by the `win-hyperv-operation` skill.
- Do not place credentials in a task workspace or include them in prompts or results.
- `code-edit-rust-slugify` does not require a Rust toolchain in `Marix_TestVm`. Its success criterion is a `manual_code_trace` judged by the Smoke Agent reading `src\lib.rs` and deterministically tracing the `slugify` logic against the declared cases, not by compiling or running the fixture.

## Prepare a fixture

Select one task, read its `setup.fixture`, and prepare the VM workspace instead of a repository-relative host directory:

1. Use PowerShell Direct to delete and recreate the case `vm_working_directory` inside `Marix_TestVm`.
2. If `setup.fixture` is non-null, treat it as an immutable source rooted at `repository_root` on the physical machine.
3. Recursively enumerate the fixture files on the physical machine and copy each file into the VM with `Copy-VMFile`, preserving relative subpaths under the case `vm_working_directory`.
4. If `setup.fixture` is `null`, do not copy anything; only ensure the VM workspace exists and is empty.

Example skeleton:

```powershell
Set-Location C:\r\Marix
$tasks = Get-Content .github\e2e\tasks.json -Raw | ConvertFrom-Json
$task = $tasks.tasks | Where-Object id -eq 'code-edit-rust-slugify'
$credential = # build exactly as defined by the win-hyperv-operation skill

Invoke-Command -VMName $tasks.vm_name -Credential $credential -ScriptBlock {
  param($path)
  Remove-Item -Recurse -Force $path -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $path -Force | Out-Null
} -ArgumentList $task.vm_working_directory

if ($null -ne $task.setup.fixture) {
  $fixtureRoot = Join-Path $tasks.repository_root $task.setup.fixture
  Get-ChildItem -LiteralPath $fixtureRoot -Recurse -File | ForEach-Object {
    $relative = $_.FullName.Substring($fixtureRoot.Length).TrimStart('\')
    $guestPath = Join-Path $task.vm_working_directory $relative
    Copy-VMFile -Name $tasks.vm_name -FileSource Host -SourcePath $_.FullName -DestinationPath $guestPath -CreateFullPath -Force
  }
}
```

Do not run a code task against `.github\e2e\fixtures` directly. The fixture is the immutable baseline. Image setup copies `scene.png` into the guest workspace. Network setup copies nothing and leaves an empty guest workspace.

## Run manually

Pass the selected prompt and both configured guardrails to the oneshot client. Prompts now point at VM absolute paths such as `C:\MarixE2E\workspaces\network-rust-stable\result.json`.

```powershell
$cli = 'C:\MarixClient\Cli\marix-client-cli.exe'
$arguments = @('--oneshot', $task.prompt)
if ($null -ne $task.max_completion_time_secs) {
  $arguments += @('--max-completion-time-secs', "$($task.max_completion_time_secs)")
}
if ($null -ne $task.max_relay_count) {
  $arguments += @('--max-relay-count', "$($task.max_relay_count)")
}
& $cli @arguments
if ($LASTEXITCODE -ne 0) { throw "oneshot failed: $LASTEXITCODE" }
```

This is the only supported deployed CLI path. Do not invoke or fall back to `C:\MarixClient\marix-client-cli.exe`.

Place the prompt immediately after `--oneshot`, before the optional flags. The CLI flags are `--max-completion-time-secs` and `--max-relay-count`. Omitting a flag for `null` preserves the corresponding TaskRequest value as `None`. The oneshot client waits for the terminal outcome. For unattended runs, a harness may use `suggested_outer_watchdog_minutes` only as a process-runaway fallback; report its activation as an environment error, separately from task and assertion failures.

## Run with the Smoke Agent

Select the `tester-of-e2e-smoke` custom agent and ask it to run all deterministic E2E smoke cases, or name a subset. The agent reads `tasks.json`, executes cases serially in array order, prepares VM workspaces, submits the oneshot client locally, evaluates criteria inside the VM, collects evidence, and cleans up the VM workspace for each case. It does not deploy or start services, modify fixture baselines, run git, read `.credential`, or run cases concurrently.

Do not ask the Smoke Agent to test the guardrail gates themselves. Its goal is only to judge whether each smoke case reaches its declared expected outcome.

## Judge the result

Apply every entry in `success_criteria` and fail on any `failure_criteria`:

1. Read and parse JSON artifacts inside the VM with PowerShell Direct, for example `Get-Content <vm-path> -Raw | ConvertFrom-Json`.
2. For `exact_json_fields`/`exact_json` fields, judge by semantic meaning by default (accept different wording, JSON type, or precision that still conveys the same correct fact), except any field a criterion lists under `strict_fields`, which must match the expected value exactly; a criterion with no `judgment` field is fully strict on every field, which is the default for precise computed or recognized data such as `code-inspect-catalog` and the image cases.
3. For `sha256_unchanged`, run `Get-FileHash -Algorithm SHA256` inside the VM.
4. For `manual_code_trace`, read the declared source file's current content from the VM and deterministically trace its logic against each declared case's input, comparing the traced output exactly against `expected_output`; do not compile or execute the fixture, and do not require a Rust toolchain in the VM.
5. Check `allowed_changed_paths` relative to a baseline snapshot taken from the guest workspace after fixture copy.
6. For network tasks, validate URL host groups, dates, source agreement, and live-access evidence. A network outage is an environmental failure.
7. Image tasks pass only when an image-capable tool actually inspected the PNG. The expected current outcome is a capability-gap report, not a fabricated answer.

Expected values are intentionally present in `tasks.json` for the harness. The image prompts tell the agent not to read that file; isolate prompt execution from the judge when possible.

## Result classification

Each case reports its duration, configured maximum time and relay count, terminal-state summary, assertion results, cleanup result, and one primary status:

- `PASS`: successful terminal outcome and all criteria pass.
- `FAIL`: task failure or assertion failure; the report distinguishes these subtypes.
- `UNSUPPORTED`: a required capability is unavailable under the case's `current_support` rule. Missing image capability is reported here, never fabricated as success.
- `ENVIRONMENT_ERROR`: setup, prerequisites, Client/transport, service availability, network infrastructure, harness, VM-side validation, or cleanup prevents a reliable judgment.

The final report totals every status and total elapsed time. Environment failures, task failures, and assertion failures remain separately identified.

## Isolation and cleanup

Run only inside the task's declared `vm_working_directory`. Do not allow access to repository `src\`, `overview\`, `.git\`, credential files, `C:\MarixHost\`, or unrelated guest paths. Snapshot the fresh guest workspace before execution when enforcing changed-path rules.

After collecting logs and artifacts, remove only the selected case's VM workspace through PowerShell Direct. To reset all guest test state after no tasks are running:

```powershell
$tasks = Get-Content .github\e2e\tasks.json -Raw | ConvertFrom-Json
$credential = # build exactly as defined by the win-hyperv-operation skill
Invoke-Command -VMName $tasks.vm_name -Credential $credential -ScriptBlock {
  param($root)
  Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
} -ArgumentList $tasks.vm_workspace_root
```

Keep fixtures unchanged between runs.
