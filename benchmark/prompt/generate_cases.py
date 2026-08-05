import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
STAGES = [
    "planning",
    "tool_calling",
    "replan",
    "infeasible",
    "subintent_complete",
    "complete",
]


def write(suite, stage, name, case):
    root = ROOT / "cases" / suite / stage
    root.mkdir(parents=True, exist_ok=True)
    payload = {
        "id": name,
        "stage": stage,
        "intent": case["intent"],
        "plan": case.get("plan", []),
        "subintent_results": case.get("subintent_results", []),
        "tool_calls": case.get("tool_calls", []),
        "failed_plans": case.get("failed_plans", []),
        "tool_call_count": case.get("tool_call_count"),
        "expected": case["expected"],
    }
    (root / f"{name}.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def cases():
    return {
        "planning": [
            (
                "single_url_fetch",
                "Fetch https://example.org/releases and return the page.",
                {"decision": "tool_calling"},
            ),
            (
                "research_then_write",
                "Research the latest stable release from two sources, "
                "reconcile them, then write result.json.",
                {"decision": "plan"},
            ),
            (
                "known_file_write",
                "Write the known text 'ready' to C:\\work\\status.txt.",
                {"decision": "tool_calling"},
            ),
            (
                "inspect_fix_verify",
                "Inspect parser.rs, fix its parsing bug, then verify the "
                "saved behaviour.",
                {"decision": "plan"},
            ),
            (
                "multi_target_update",
                "Update the version in a.json and b.json.",
                {"decision": "plan"},
            ),
            (
                "single_command",
                "Run `git status --short` and return its output.",
                {"decision": "tool_calling"},
            ),
            (
                "one_source_two_values",
                "Fetch one named release page and return both its version "
                "and date.",
                {"decision": "tool_calling"},
            ),
            (
                "start_then_read_output",
                "Start worker.exe, wait for it to finish, then return its "
                "first output line.",
                {"decision": "plan"},
            ),
        ],
        "tool_calling": [
            (
                "fetch_exact_url",
                "Fetch https://example.org/releases and return the page.",
                {"tools": ["web_fetch"]},
            ),
            (
                "write_known_content",
                "Write 'ready' to C:\\work\\status.txt.",
                {"tools": ["write_file"]},
            ),
            (
                "read_known_file",
                "Read C:\\work\\config.json and return its contents.",
                {"tools": ["read_file"]},
            ),
            (
                "run_known_command",
                "Run `git status --short` and return its output.",
                {"tools": ["command_prompt"]},
            ),
            (
                "search_known_text",
                "Search C:\\work\\src for the exact text RETRY_LIMIT.",
                {"tools": ["search_text"]},
            ),
            (
                "replace_known_block",
                "In C:\\work\\app.json replace port=3000 with port=8000.",
                {"tools": ["replace_in_file"]},
            ),
            (
                "list_known_directory",
                "List the direct entries under C:\\work\\src.",
                {"tools": ["list_directory"]},
            ),
            (
                "start_known_process",
                "Start C:\\work\\worker.exe and return its process id.",
                {"tools": ["start_process"]},
            ),
        ],
        "replan": [
            (
                "alternate_source_exists",
                "Find the stable release date from an independent source.",
                {"decision": "replan"},
                {
                    "failed_plans": [
                        "Fetch source A; source A is permanently unavailable."
                    ]
                },
            ),
            (
                "all_sources_blocked",
                "Read the release date from the only permitted private "
                "source.",
                {"decision": "infeasible_review"},
                {
                    "failed_plans": [
                        "The only permitted source requires a destroyed "
                        "credential with no recovery path."
                    ]
                },
            ),
            (
                "different_method_available",
                "Extract the version from the local repository.",
                {"decision": "replan"},
                {
                    "failed_plans": [
                        "The package command is unavailable; the source file "
                        "has not been inspected."
                    ]
                },
            ),
            (
                "failed_plans_exhausted",
                "Recover the deleted encryption key.",
                {"decision": "infeasible_review"},
                {
                    "failed_plans": [
                        "The only key was destroyed.",
                        "No backup, escrow, or recovery path exists.",
                    ]
                },
            ),
            (
                "completed_facts_new_plan",
                "Produce the release report despite the failed source.",
                {"decision": "replan"},
                {
                    "subintent_results": [
                        {
                            "goal": "Collect version",
                            "result": "Version 1.97.1 is confirmed.",
                        }
                    ],
                    "failed_plans": [
                        "The first source had no release date."
                    ],
                },
            ),
            (
                "permanent_requirement_block",
                "Sign the artifact using the only permitted destroyed key.",
                {"decision": "infeasible_review"},
                {
                    "failed_plans": [
                        "The permitted key was destroyed.",
                        "Policy forbids every other signing key.",
                    ]
                },
            ),
        ],
        "infeasible": [
            (
                "temporary_network_failure",
                "Fetch the specified public URL.",
                {"decision": "continue"},
                {
                    "tool_calls": [
                        {
                            "tool": "web_fetch",
                            "arguments": "https://example.org",
                            "result": "Timed out once.",
                        }
                    ]
                },
            ),
            (
                "destroyed_secret",
                "Decrypt the archive using the only key, which was destroyed "
                "with no backup.",
                {"decision": "infeasible"},
            ),
            (
                "uninspected_file",
                "Find the configured port in C:\\work\\app.json.",
                {"decision": "continue"},
            ),
            (
                "missing_hardware",
                "Read a fingerprint using hardware this machine does not "
                "have and cannot access.",
                {"decision": "infeasible"},
            ),
            (
                "four_calls_still_feasible",
                "Find the release date on the named public site.",
                {"decision": "continue"},
                {"tool_call_count": 4},
            ),
            (
                "contradictory_goal",
                "Create one file that is both empty and contains the full "
                "report.",
                {"decision": "infeasible"},
            ),
            (
                "retryable_command_failure",
                "Run the known command and return its output.",
                {"decision": "continue"},
                {
                    "tool_calls": [
                        {
                            "tool": "command_prompt",
                            "arguments": "build.cmd",
                            "result": "Temporary file lock; retry is allowed.",
                        }
                    ]
                },
            ),
            (
                "forbidden_only_path",
                "Upload the artifact through the only endpoint, which policy "
                "permanently forbids.",
                {"decision": "infeasible"},
            ),
        ],
        "subintent_complete": [
            (
                "children_cover_parent",
                "Collect the current stable release and save its report.",
                {
                    "decision": "complete",
                    "summary_contains": ["1.97.1", "result.json"],
                },
                {
                    "subintent_results": [
                        {
                            "goal": "Collect release facts",
                            "result": "Stable version is 1.97.1.",
                        },
                        {
                            "goal": "Save report",
                            "result": "Wrote C:\\work\\result.json.",
                        },
                    ]
                },
            ),
            (
                "children_miss_requirement",
                "Collect both version and release date, then save them.",
                {"decision": "replan"},
                {
                    "subintent_results": [
                        {
                            "goal": "Collect release facts",
                            "result": "Version is 1.97.1; date was not found.",
                        },
                        {
                            "goal": "Save report",
                            "result": "No report written because date is "
                            "missing.",
                        },
                    ]
                },
            ),
            (
                "children_preserve_context",
                "Prepare the endpoint and authentication context.",
                {
                    "decision": "complete",
                    "summary_contains": ["api.example.org", "token_env"],
                },
                {
                    "subintent_results": [
                        {
                            "goal": "Resolve endpoint",
                            "result": "Endpoint is api.example.org.",
                        },
                        {
                            "goal": "Resolve auth",
                            "result": "Use token_env for authentication.",
                        },
                    ]
                },
            ),
            (
                "child_reports_incomplete",
                "Patch and verify the parser.",
                {"decision": "replan"},
                {
                    "subintent_results": [
                        {
                            "goal": "Patch parser",
                            "result": "Patch applied.",
                        },
                        {
                            "goal": "Verify parser",
                            "result": "Verification was not run.",
                        },
                    ]
                },
            ),
            (
                "children_exact_summary",
                "Resolve the host, port, and token source.",
                {
                    "decision": "complete",
                    "summary_contains": [
                        "api.example.org",
                        "8443",
                        "SERVICE_TOKEN",
                    ],
                },
                {
                    "subintent_results": [
                        {
                            "goal": "Resolve endpoint",
                            "result": "Host api.example.org uses port 8443.",
                        },
                        {
                            "goal": "Resolve token",
                            "result": "Read it from SERVICE_TOKEN.",
                        },
                    ]
                },
            ),
            (
                "child_side_effect_missing",
                "Patch parser.rs and save the verified result.",
                {"decision": "replan"},
                {
                    "subintent_results": [
                        {
                            "goal": "Patch parser",
                            "result": "Patch generated but not saved.",
                        },
                        {
                            "goal": "Verify parser",
                            "result": "Verification used the generated patch.",
                        },
                    ]
                },
            ),
        ],
        "complete": [
            (
                "tool_result_satisfies_goal",
                "Return the configured port.",
                {
                    "decision": "complete",
                    "summary_contains": ["8443"],
                },
                {
                    "tool_calls": [
                        {
                            "tool": "read_file",
                            "arguments": "C:\\work\\app.json",
                            "result": "The configured port is 8443.",
                        }
                    ],
                    "tool_call_count": 1,
                },
            ),
            (
                "tool_result_missing_value",
                "Return both version and release date.",
                {"decision": "continue"},
                {
                    "tool_calls": [
                        {
                            "tool": "web_fetch",
                            "arguments": "https://example.org/releases",
                            "result": "Version is 1.97.1; date was not found.",
                        }
                    ],
                    "tool_call_count": 1,
                },
            ),
            (
                "recoverable_tool_error",
                "Read C:\\work\\app.json.",
                {"decision": "continue"},
                {
                    "tool_calls": [
                        {
                            "tool": "read_file",
                            "arguments": "C:\\work\\app.json",
                            "result": "Temporary sharing violation.",
                        }
                    ],
                    "tool_call_count": 1,
                },
            ),
            (
                "write_result_complete",
                "Write the known report to C:\\work\\result.json.",
                {
                    "decision": "complete",
                    "summary_contains": ["result.json", "512 bytes"],
                },
                {
                    "tool_calls": [
                        {
                            "tool": "write_file",
                            "arguments": "C:\\work\\result.json",
                            "result": "Wrote 512 bytes to "
                            "C:\\work\\result.json.",
                        }
                    ],
                    "tool_call_count": 1,
                },
            ),
            (
                "four_calls_need_more",
                "Find the release date from the named source.",
                {"decision": "continue"},
                {
                    "tool_calls": [
                        {
                            "tool": "web_fetch",
                            "arguments": "https://example.org/archive",
                            "result": "The archive index has no release date.",
                        }
                    ],
                    "tool_call_count": 4,
                },
            ),
            (
                "exact_error_requested",
                "Return the exact error from the command.",
                {
                    "decision": "complete",
                    "summary_contains": ["Access is denied", "exit code 5"],
                },
                {
                    "tool_calls": [
                        {
                            "tool": "command_prompt",
                            "arguments": "restricted.exe",
                            "result": "exit code 5; stderr: Access is denied.",
                        }
                    ],
                    "tool_call_count": 1,
                },
            ),
            (
                "partial_multi_value",
                "Return host, port, and protocol.",
                {"decision": "continue"},
                {
                    "tool_calls": [
                        {
                            "tool": "read_file",
                            "arguments": "C:\\work\\endpoint.json",
                            "result": "Host is api.example.org and port is "
                            "8443; protocol is absent.",
                        }
                    ],
                    "tool_call_count": 2,
                },
            ),
            (
                "all_values_present",
                "Return host, port, and protocol.",
                {
                    "decision": "complete",
                    "summary_contains": [
                        "api.example.org",
                        "8443",
                        "https",
                    ],
                },
                {
                    "tool_calls": [
                        {
                            "tool": "read_file",
                            "arguments": "C:\\work\\endpoint.json",
                            "result": "Host api.example.org, port 8443, "
                            "protocol https.",
                        }
                    ],
                    "tool_call_count": 2,
                },
            ),
        ],
    }


def main():
    definitions = cases()
    for suite in ["required", "guide"]:
        for stage in STAGES:
            (ROOT / "cases" / suite / stage).mkdir(
                parents=True, exist_ok=True
            )

    required_names = {
        "planning": {"single_url_fetch", "research_then_write"},
        "tool_calling": {"fetch_exact_url", "write_known_content"},
        "replan": {"alternate_source_exists", "all_sources_blocked"},
        "infeasible": {"temporary_network_failure", "destroyed_secret"},
        "subintent_complete": {
            "children_cover_parent",
            "children_miss_requirement",
        },
        "complete": {
            "tool_result_satisfies_goal",
            "tool_result_missing_value",
        },
    }
    for stage, items in definitions.items():
        for item in items:
            name, intent, expected, *rest = item
            extra = rest[0] if rest else {}
            case = {"intent": intent, "expected": expected, **extra}
            write("guide", stage, name, case)
            if name in required_names[stage]:
                write("required", stage, name, case)
    print("generated stage cases")


if __name__ == "__main__":
    main()
