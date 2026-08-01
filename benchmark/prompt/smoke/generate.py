import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent


PLAN_SCENARIOS = [
    ("research_write", "Find the current stable {product} version from its official release page and {secondary}, reconcile the values, then write the confirmed version and date to {path}."),
    ("read_edit_verify", "Read {path}, change only the configured port from {old} to {new} while preserving every other field, write it back, then verify the saved value."),
    ("inspect_fix_check", "Inspect {path} to locate why {symbol} rejects empty input, modify the responsible implementation, then verify the corrected source contains the intended guard."),
    ("start_wait_read", "Start {program} with {args}, wait for it to finish, then read its captured output and report the exit result."),
    ("download_hash_install", "Fetch the release metadata from {url}, obtain the named installer, verify its published SHA-256 value, install it, then report the installed version."),
    ("list_read_summarize", "List all JSON files under {directory}, read each listed file, then write a consolidated summary to {path}."),
    ("compare_sources", "Search for the current {product} support policy, fetch both the vendor policy and the independent compatibility table, then report only the points on which they agree."),
    ("environment_configure", "Inspect the safe environment values, choose the correct architecture-specific package for this machine, fetch its release page, then write the selected package URL to {path}."),
    ("outline_replace_verify", "Extract the outline of {path}, identify the public function responsible for {symbol}, replace its exact obsolete block, then re-read the outline to verify the public shape."),
    ("search_read_patch", "Search {directory} for the exact token {symbol}, read the matching source file, replace the obsolete value {old} with {new}, then confirm no old token remains."),
    ("multi_target_update", "Read the common payload from {path}, upload its exact content to the two independently named destination files {path_a} and {path_b}, then verify both destinations."),
    ("service_deploy_ready", "Read the deployment manifest at {path}, run its deployment command, start the resulting service, then confirm its readiness endpoint {url} reports success."),
    ("process_restart_verify", "Stop the registered process {process_id}, start {program} again with {args}, then read the new process output to confirm startup."),
    ("inventory_transform", "Recursively list {directory}, collect the outlines of every Rust source file found there, then write a module inventory to {path}."),
    ("web_collect_transform", "Fetch {url} and {secondary_url}, extract the release dates from both pages, normalize them to YYYY-MM-DD, then write the comparison to {path}."),
    ("command_parse_write", "Run the PowerShell command {command}, parse the returned version and architecture, then write those two values as JSON to {path}."),
    ("read_branch_actions", "Read {path}; if its mode is production run {command_a}, otherwise run {command_b}; then record which branch ran in {path_b}."),
    ("search_count_report", "Search {directory} for {symbol}, read every matching file to distinguish definitions from references, then write both counts to {path}."),
    ("fetch_followup", "Search the web for the official {product} download page, fetch the selected official result, then fetch its checksum link and report the checksum."),
    ("config_migration", "Read the legacy configuration at {path}, convert its keys to the documented new schema from {url}, write the migrated file to {path_b}, then validate all required keys."),
    ("two_commands_compare", "Run {command_a} and {command_b} independently, compare their reported versions, then write whether they match to {path}."),
    ("directory_select_read", "List {directory}, select the newest file by the timestamp shown in its name, read that file, then report its status field."),
    ("repair_after_failure", "Determine a materially different way to obtain {product} after the failed approaches, obtain it, then verify the resulting version."),
    ("evidence_then_side_effect", "Obtain the exact required value for {symbol} from {url}, then replace the placeholder in {path} with that value and verify the saved file."),
    ("audit_then_remediate", "Inspect {directory} for files containing {old}, produce the exact affected-file list, replace each occurrence with {new}, then confirm the old value is absent."),
]


COMPLETE_SCENARIOS = [
    ("read_value", "Return the exact contents of {path}.", ["read_file ({path}):\n{content}"]),
    ("find_token", "Find all occurrences of {symbol} under {directory}.", ["search_text ({directory} --query {symbol}):\nFound 3 matches in alpha.rs lines 8, 19, and beta.rs line 4."]),
    ("list_entries", "List the direct entries under {directory}.", ["list_directory ({directory}):\nalpha.txt\nbeta.json\nsrc"]),
    ("fetch_page", "Return the title from {url}.", ["web_fetch ({url}):\nPage title: {product} Release Notes"]),
    ("search_web", "Find the official download page for {product}.", ["web_search ({product} official download):\nOfficial download: {url}"]),
    ("write_done", "Write {content} to {path}.", ["write_file ({path}):\nSuccessfully wrote {bytes} bytes to {path}."]),
    ("replace_done", "Replace {old} with {new} in {path}.", ["replace_in_file ({path}):\nReplaced exactly one occurrence of {old} with {new}."]),
    ("command_done", "Run {command} and return its output.", ["powershell ({command}):\nExit code: 0\nOutput: {content}"]),
    ("cmd_done", "Run the cmd command {command_a}.", ["command_prompt ({command_a}):\nExit code: 0\nOutput: completed"]),
    ("outline_done", "Return the public outline of {path}.", ["get_code_outline ({path}):\npub struct Config\npub fn load(path: &str) -> Config"]),
    ("env_done", "Report the safe operating-system and architecture values.", ["os_env:\nplatform=Windows\narch=amd64"]),
    ("process_started", "Start {program} with {args}.", ["start_process ({program} {args}):\nProcess started with id {process_id}."]),
    ("process_output", "Return the captured output for process {process_id}.", ["read_process_output ({process_id}):\nstdout: ready\nstderr:\nexit_code: 0"]),
    ("process_stopped", "Stop registered process {process_id}.", ["stop_process ({process_id}):\nProcess and child tree stopped successfully."]),
    ("verified_write", "Write {content} to {path} and verify its saved contents.", ["write_file ({path}):\nSuccessfully wrote {bytes} bytes.", "read_file ({path}):\n{content}"]),
    ("verified_install", "Install {product} and verify version {version}.", ["powershell (installer):\nExit code: 0; installation succeeded.", "powershell ({product} --version):\n{product} {version}"]),
    ("research_answer", "Determine the stable {product} version using {url} and {secondary_url}.", ["web_fetch ({url}):\nStable {product} is {version}.", "web_fetch ({secondary_url}):\nCurrent stable version is {version}."]),
    ("deployment_ready", "Deploy {product} and confirm {url} is ready.", ["powershell (deploy {product}):\nExit code: 0; service started.", "web_fetch ({url}):\nHTTP 200; status=ready"]),
    ("comparison_done", "Report whether {command_a} and {command_b} return the same version.", ["powershell ({command_a}):\nversion={version}", "powershell ({command_b}):\nversion={version}"]),
    ("file_count_done", "Count JSON files under {directory}.", ["powershell (count JSON files under {directory}):\nExit code: 0\nCount: 7"]),
    ("parent_pending_child_done", "Return the enabled value from {path}.", ["read_file ({path}):\n{{\"enabled\": false}}"]),
    ("failure_then_success", "Fetch {url} and return its response body.", ["web_fetch ({url}):\nFirst attempt timed out.", "web_fetch ({url} retry):\nHTTP 200\n{content}"]),
    ("exact_date_done", "Return the release date for {product} {version}.", ["web_fetch ({url}):\n{product} {version} was released on 2026-07-16."]),
    ("multi_file_done", "Write the same payload to {path_a} and {path_b}.", ["write_file ({path_a}):\nSuccessfully wrote payload.", "write_file ({path_b}):\nSuccessfully wrote payload."]),
    ("plan_child_done", "Confirm the generated report at {path} contains version {version}.", ["read_file ({path}):\n{{\"version\":\"{version}\"}}"]),
]


INFEASIBLE_SCENARIOS = [
    ("missing_camera", "Take a new photograph with a physical camera attached to this machine; no camera or image-capture device exists and generated images are forbidden.", "No supplied tool can operate absent physical camera hardware."),
    ("missing_nfc", "Read the UID of a physical NFC card; no NFC reader or device bridge exists and a simulated UID is unacceptable.", "No NFC hardware or capture capability exists."),
    ("missing_secret", "Decrypt {path}; the required password is irretrievably unavailable, brute force and password recovery are forbidden, and no unencrypted copy exists.", "The indispensable decryption secret cannot be obtained by an allowed action."),
    ("deleted_no_backup", "Recover the exact original contents of {path}; it was securely erased, no backup or snapshot exists, and reconstruction from guesses is forbidden.", "The required information no longer exists in any accessible source."),
    ("offline_private_host", "Read {path} from an isolated powered-off computer that has no network, remote-management channel, shared storage, or available operator.", "No supplied tool can reach or power the isolated computer."),
    ("physical_signature", "Place an original handwritten signature on a physical paper contract; no printer, pen plotter, robotic device, or human operator is available.", "The required physical-world action has no available actuator."),
    ("contradictory_bytes", "Write a file at {path} that is simultaneously exactly 0 bytes and exactly {bytes} bytes, with no alternate representation allowed.", "The required final state is logically contradictory."),
    ("unknown_random", "Return the exact 256-bit value generated yesterday by a disconnected hardware random generator; the value was never recorded or transmitted.", "The exact random value is absent from every accessible source."),
    ("destroyed_key", "Sign {path} with a specific private key that was destroyed; substitution, regeneration, and a different key are forbidden.", "The indispensable private key no longer exists."),
    ("future_fact", "Return the exact winning lottery numbers for a drawing that has not happened yet, without prediction or approximation.", "The requested fact does not yet exist."),
    ("unavailable_sensor", "Measure the current temperature inside a sealed remote freezer with a physical sensor; no sensor or remote connection exists.", "No measurement capability can observe the required physical state."),
    ("airgapped_usb", "Copy {path} onto a USB drive physically located in another building; no connected USB device, network path, or human operator exists.", "No available tool can affect the remote physical media."),
    ("forbidden_network", "Fetch {url} while all network access is explicitly forbidden and no cached copy exists; fabricated content is unacceptable.", "The only possible information source is disallowed and no local copy exists."),
    ("nonexistent_process", "Read captured output for process {process_id}; the process was never started or registered and recreating it is forbidden.", "The required historical process output does not exist."),
    ("unsupported_device_flash", "Flash firmware to a disconnected embedded board; no board, programmer, serial bridge, or operator is available.", "No physical target or programming interface exists."),
    ("lost_encryption_key", "Recover the exact plaintext of {path}, encrypted with a properly implemented cipher whose sole key was destroyed; attacks and guessing are forbidden.", "The only decryption key is permanently unavailable."),
    ("exact_unlogged_history", "Report the exact stdout of a command run last month; logging was disabled, the terminal closed, and rerunning the command is forbidden.", "No record of the requested historical output exists."),
    ("inaccessible_account", "Read a private document in an account for which no credentials, session, recovery channel, public copy, or authorized operator exists.", "No authorized access path or recoverable credential exists."),
    ("physical_reboot", "Press the hardware reset button on an unnetworked remote appliance; no remote controller or person is present.", "No available actuator can perform the required physical action."),
    ("mutually_exclusive_date", "Return one date that is strictly before and strictly after 2026-08-01 under the same calendar and comparison rules.", "No date can satisfy both constraints."),
    ("missing_biometric", "Authenticate using the absent account owner's live fingerprint; no owner, scanner, delegated method, or bypass is available.", "The required live biometric and capture device are unavailable."),
    ("destroyed_database", "Query the exact deleted row from a database whose storage and replicas were securely destroyed and whose value appeared nowhere else.", "The requested data has no remaining accessible representation."),
    ("no_location_signal", "Report the exact current GPS coordinates of an unpowered tracker with no GPS fix, radio, stored location, or nearby observer.", "No source can observe or report the tracker's location."),
    ("prohibited_execution", "Execute {program}, but execution of every process and command is forbidden by the task and no precomputed result exists.", "The task requires and simultaneously forbids its only possible operation."),
    ("failed_all_plans", "Obtain the exact proprietary artifact {path}; every distinct authorized source has confirmed permanent deletion and unauthorized sources are forbidden.", "Every materially different authorized retrieval plan has failed permanently."),
]


ORDINARY_SCENARIOS = [
    ("read_file", "Read {path} and return its exact UTF-8 contents."),
    ("write_file", "Write the exact text {content} to {path}."),
    ("web_fetch", "Fetch {url} and return the page content."),
    ("web_search", "Search the web for {product} official release notes."),
    ("powershell", "Run the PowerShell command {command} and return its output."),
    ("command_prompt", "Run the cmd.exe command {command_a} and return its output."),
    ("get_code_outline", "Return the code outline of {path}."),
    ("list_directory", "List the direct entries under {directory}."),
    ("os_env", "Report the safe allowlisted system environment."),
    ("read_process_output", "Read the captured output of registered process {process_id}."),
    ("replace_in_file", "In {path}, replace the exact text {old} with {new}."),
    ("search_text", "Search {directory} recursively for the exact text {symbol}."),
    ("start_process", "Start {program} with arguments {args} and return the registered process id."),
    ("stop_process", "Stop the registered process {process_id} and its child tree."),
    ("read_file", "Read the configuration file {path}."),
    ("write_file", "Create {path} with the known JSON content {{\"mode\":\"safe\",\"port\":{new}}}."),
    ("web_fetch", "Fetch the known official URL {secondary_url}."),
    ("web_search", "Search the web for the exact query \"{product} {version} checksum\"."),
    ("powershell", "Run Get-Service {product} in PowerShell and return its status."),
    ("command_prompt", "Run ver in cmd.exe and return the Windows version line."),
    ("list_directory", "Recursively list entries under {directory}."),
    ("search_text", "Search the single file {path} for {symbol}, case-sensitive."),
    ("replace_in_file", "Replace the exact configuration value port={old} with port={new} in {path}."),
    ("read_process_output", "Read at most 4096 bytes of output from registered process {process_id}, starting from offset zero."),
    ("start_process", "Start {program} without additional arguments."),
]


PRODUCTS = ["Rust", "Git", "Node.js", "Python", "CMake", "PowerShell", "MarixServer", "serde"]
URLS = [
    "https://example.com/releases",
    "https://example.org/status",
    "https://vendor.example/downloads",
    "https://docs.example/versions",
]
SECONDARY_URLS = [
    "https://mirror.example/releases",
    "https://compat.example/table",
    "https://registry.example/package",
    "https://community.example/support",
]


def values(index, depth):
    product = PRODUCTS[index % len(PRODUCTS)]
    return {
        "product": product,
        "secondary": f"the independent {product} compatibility index",
        "path": f"C:\\workflow400\\d{depth}\\case-{index:03d}\\input-{index:03d}.json",
        "path_a": f"C:\\workflow400\\d{depth}\\case-{index:03d}\\target-a.txt",
        "path_b": f"C:\\workflow400\\d{depth}\\case-{index:03d}\\target-b.txt",
        "directory": f"C:\\workflow400\\d{depth}\\case-{index:03d}\\src",
        "old": str(3000 + index),
        "new": str(8000 + index),
        "symbol": f"WorkflowSymbol{index:03d}",
        "program": "powershell.exe",
        "args": f"-NoProfile -Command Write-Output case-{index:03d}",
        "url": URLS[index % len(URLS)] + f"?case={index:03d}",
        "secondary_url": SECONDARY_URLS[index % len(SECONDARY_URLS)] + f"?case={index:03d}",
        "command": f"Get-Item 'C:\\workflow400\\case-{index:03d}.txt' | Select-Object Name,Length",
        "command_a": f"echo case-{index:03d}",
        "command_b": f"Write-Output case-{index:03d}",
        "process_id": f"00000000-0000-4000-8000-{index:012d}",
        "content": f"confirmed-value-{index:03d}",
        "bytes": 64 + index,
        "version": f"{1 + index % 9}.{index % 20}.{depth}",
    }


def make_ancestors(category, current_task, depth, index):
    ancestors = []
    child_goal = current_task
    for level in range(depth - 1, 0, -1):
        parent_goal = (
            f"Complete {category} workflow {index:03d} at hierarchy level {level}, "
            f"including the delegated child and a later parent-only archival step."
        )
        ancestors.append(
            {
                "goal": parent_goal,
                "plan": [
                    {
                        "goal": f"Prepare prerequisites for hierarchy level {level}",
                        "status": "completed",
                        "result": f"Prerequisites for level {level} are confirmed.",
                    },
                    {
                        "goal": child_goal,
                        "status": "executing",
                    },
                    {
                        "goal": f"Archive the parent-level result for level {level}",
                        "status": "pending",
                    },
                ],
                "completed_calls": [
                    f"powershell (prepare hierarchy level {level}):\nExit code: 0; prerequisites ready."
                ],
                "fail_plans": [],
            }
        )
        child_goal = parent_goal
    ancestors.reverse()
    return ancestors


def plan_case(base_index, depth):
    family, template = PLAN_SCENARIOS[base_index]
    index = base_index * 4 + depth
    data = values(index, depth)
    task = template.format(**data)
    fail_plans = []
    if base_index in {4, 17, 22}:
        fail_plans = [
            {
                "goals": [
                    f"Use unavailable source A for {data['product']}",
                    f"Apply the result to {data['path']}",
                ],
                "reason": "Source A is permanently unavailable; a different source and sequence are required.",
            }
        ]
    return {
        "suite": "smoke",
        "id": f"plan-{base_index + 1:03d}-d{depth}",
        "category": "workflow_plan",
        "depth": depth,
        "family": family,
        "overall_goal": f"Finish planned workflow {base_index + 1:03d} and preserve its confirmed result.",
        "ancestors": make_ancestors("planning", task, depth, index),
        "current_task": task,
        "completed_calls": [],
        "fail_plans": fail_plans,
        "expected_tools": ["workflow_plan"],
    }


def complete_case(base_index, depth):
    family, template, completed_templates = COMPLETE_SCENARIOS[base_index]
    index = 100 + base_index * 4 + depth
    data = values(index, depth)
    task = template.format(**data)
    completed = [item.format(**data) for item in completed_templates]
    return {
        "suite": "smoke",
        "id": f"complete-{base_index + 1:03d}-d{depth}",
        "category": "workflow_complete",
        "depth": depth,
        "family": family,
        "overall_goal": f"Finish evidence-backed workflow {base_index + 1:03d}; parent archival may remain.",
        "ancestors": make_ancestors("completion", task, depth, index),
        "current_task": task,
        "completed_calls": completed,
        "fail_plans": [],
        "expected_tools": ["workflow_complete"],
    }


def infeasible_case(base_index, depth):
    family, template, reason = INFEASIBLE_SCENARIOS[base_index]
    index = 200 + base_index * 4 + depth
    data = values(index, depth)
    task = template.format(**data)
    fail_plans = [
        {
            "goals": [
                f"Attempt authorized approach {attempt + 1} for {family}",
                "Produce the exact required result",
            ],
            "reason": f"{reason} Attempt {attempt + 1} cannot remove the permanent constraint.",
        }
        for attempt in range(base_index % 4)
    ]
    completed = [
        f"powershell (capability inspection):\nConfirmed: {reason}"
    ] if base_index % 2 == 0 else []
    return {
        "suite": "smoke",
        "id": f"infeasible-{base_index + 1:03d}-d{depth}",
        "category": "workflow_infeasible",
        "depth": depth,
        "family": family,
        "overall_goal": f"Attempt constrained workflow {base_index + 1:03d} without fabricating results.",
        "ancestors": make_ancestors("infeasibility", task, depth, index),
        "current_task": task,
        "completed_calls": completed,
        "fail_plans": fail_plans,
        "expected_tools": ["workflow_infeasible"],
    }


def ordinary_case(base_index, depth):
    tool, template = ORDINARY_SCENARIOS[base_index]
    index = 300 + base_index * 4 + depth
    data = values(index, depth)
    task = template.format(**data)
    return {
        "suite": "smoke",
        "id": f"ordinary-{base_index + 1:03d}-d{depth}",
        "category": "ordinary",
        "depth": depth,
        "family": tool,
        "overall_goal": f"Finish direct-operation workflow {base_index + 1:03d} and later archive its result.",
        "ancestors": make_ancestors("ordinary execution", task, depth, index),
        "current_task": task,
        "completed_calls": [],
        "fail_plans": [],
        "expected_tools": [tool],
    }


def main():
    cases = []
    for depth in range(1, 5):
        cases.extend(plan_case(index, depth) for index in range(len(PLAN_SCENARIOS)))
        cases.extend(complete_case(index, depth) for index in range(len(COMPLETE_SCENARIOS)))
        cases.extend(infeasible_case(index, depth) for index in range(len(INFEASIBLE_SCENARIOS)))
        cases.extend(ordinary_case(index, depth) for index in range(len(ORDINARY_SCENARIOS)))
    cases.sort(key=lambda case: (case["category"], case["id"]))
    payload = json.dumps(cases, ensure_ascii=False, indent=2) + "\n"
    output = ROOT / "cases.json"
    payload_bytes = payload.encode("utf-8")
    output.write_bytes(payload_bytes)
    digest = hashlib.sha256(payload_bytes).hexdigest()
    counts = {}
    depths = {}
    for case in cases:
        counts[case["category"]] = counts.get(case["category"], 0) + 1
        depths[case["depth"]] = depths.get(case["depth"], 0) + 1
    manifest = {
        "schema_version": 1,
        "suite": "smoke",
        "case_file": output.name,
        "sha256": digest,
        "total": len(cases),
        "category_counts": counts,
        "depth_counts": depths,
        "semantic_families_per_category": 25,
        "generation_rule": "25 curated semantic families x 4 context depths per category",
    }
    (ROOT / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
