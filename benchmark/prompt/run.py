import argparse
import concurrent.futures
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import tomllib


ROOT = Path(__file__).resolve().parent
REPO_ROOT = ROOT.parents[1]
RESULTS_ROOT = ROOT / "results"
TEMP_ROOT = REPO_ROOT / ".temp" / "benchmark-prompt"
CATEGORIES = [
    "workflow_plan",
    "workflow_complete",
    "workflow_infeasible",
    "ordinary",
]
CURL = shutil.which("curl") or shutil.which("curl.exe")


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def load_model_config():
    config_path = os.environ.get("MARIX_CONFIG", "").strip()
    if not config_path:
        raise RuntimeError(
            "MARIX_CONFIG must point to a Marix config with model credentials"
        )
    with open(config_path, "rb") as config_file:
        config = tomllib.load(config_file)
    selected = config["model"]["selected"]
    model = config["model"][selected]
    return {
        "endpoint": model["endpoint"],
        "model": model["model"],
        "api_key": model["api_key"],
    }


def verify_suite(config, suite):
    suite_config = config[suite]
    case_path = ROOT / suite_config["case_file"]
    manifest_path = ROOT / suite_config["manifest_file"]
    manifest = load_json(manifest_path)
    payload = case_path.read_bytes()
    actual_hash = sha256(payload)
    if actual_hash != manifest["sha256"]:
        raise RuntimeError(
            f"{suite} cases changed without a manifest update: "
            f"expected {manifest['sha256']}, got {actual_hash}"
        )
    cases = json.loads(payload.decode("utf-8"))
    if len(cases) != manifest["total"]:
        raise RuntimeError(
            f"{suite} manifest expects {manifest['total']} cases, "
            f"found {len(cases)}"
        )
    ids = [case["id"] for case in cases]
    if len(ids) != len(set(ids)):
        raise RuntimeError(f"{suite} contains duplicate case IDs")
    for case in cases:
        if case.get("suite") != suite:
            raise RuntimeError(
                f"{case['id']} belongs to {case.get('suite')!r}, not {suite!r}"
            )
        if case["category"] not in CATEGORIES:
            raise RuntimeError(
                f"{case['id']} has unsupported category {case['category']!r}"
            )
    if suite == "smoke":
        if len(cases) != suite_config["expected_total"]:
            raise RuntimeError("smoke case total does not match benchmark.json")
        for category in CATEGORIES:
            count = sum(case["category"] == category for case in cases)
            if count != suite_config["expected_per_category"]:
                raise RuntimeError(
                    f"smoke expected {suite_config['expected_per_category']} "
                    f"{category} cases, found {count}"
                )
    return manifest, cases


def append_completed(lines, candidate, calls):
    if not calls:
        return
    lines.extend(
        [
            "",
            candidate["completed_header"],
            candidate["completed_notice"],
        ]
    )
    lines.extend(f"{index}. {value}" for index, value in enumerate(calls, 1))


def append_fail_plans(lines, candidate, plans):
    if not plans:
        return
    lines.extend(["", candidate["fail_plans_header"]])
    for plan in plans:
        lines.extend(f"- {goal}" for goal in plan["goals"])
        lines.append(
            f"{candidate['fail_reason_label']} {plan['reason']}"
        )


def render_ancestor(candidate, ancestor):
    lines = [
        candidate["goal_header"],
        ancestor["goal"],
        "",
        candidate["plan_header"],
    ]
    for index, item in enumerate(ancestor["plan"], 1):
        if item["status"] == "completed":
            lines.extend([f"{index}. {item['goal']}:", item["result"]])
        elif item["status"] == "executing":
            lines.append(f"{index}. [EXECUTING NOW] {item['goal']}")
        else:
            lines.append(f"{index}. {item['goal']}")
    append_completed(lines, candidate, ancestor.get("completed_calls", []))
    append_fail_plans(lines, candidate, ancestor.get("fail_plans", []))
    return "\n".join(lines)


def render_current(candidate, case):
    lines = [
        candidate["current_header"],
        "",
        candidate["goal_header"],
        case["current_task"],
    ]
    append_completed(lines, candidate, case.get("completed_calls", []))
    append_fail_plans(lines, candidate, case.get("fail_plans", []))
    return "\n".join(lines)


def compose_messages(candidate, case):
    messages = [
        {
            "role": "system",
            "content": candidate["system"].replace(
                "{{system}}", "Windows on amd64"
            ),
        },
        {
            "role": "system",
            "content": candidate["policy"].replace(
                "{{goal}}", case["overall_goal"]
            ),
        },
    ]
    if case["ancestors"]:
        background = candidate["background_header"] + "\n\n\n"
        background += "\n\n\n".join(
            render_ancestor(candidate, ancestor)
            for ancestor in case["ancestors"]
        )
        messages.append({"role": "user", "content": background})
    messages.append(
        {"role": "user", "content": render_current(candidate, case)}
    )
    return messages


def post_request(model_config, body):
    if not CURL:
        raise RuntimeError("curl is unavailable")
    TEMP_ROOT.mkdir(parents=True, exist_ok=True)
    for attempt in range(3):
        request_path = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                suffix=".json",
                prefix="prompt-benchmark-",
                dir=TEMP_ROOT,
                delete=False,
            ) as request_file:
                json.dump(body, request_file, ensure_ascii=False)
                request_path = request_file.name
            curl_config = "\n".join(
                [
                    f'url = "{model_config["endpoint"]}"',
                    'request = "POST"',
                    f'header = "Authorization: Bearer {model_config["api_key"]}"',
                    'header = "Content-Type: application/json"',
                    'header = "Accept: application/json"',
                    "silent",
                    "show-error",
                    "fail-with-body",
                    "max-time = 90",
                ]
            )
            response = subprocess.run(
                [CURL, "--config", "-", "--data-binary", f"@{request_path}"],
                input=curl_config,
                text=True,
                encoding="utf-8",
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            if response.returncode != 0:
                raise RuntimeError(
                    response.stderr.strip()
                    or response.stdout.strip()
                    or f"curl exited with {response.returncode}"
                )
            return json.loads(response.stdout)
        except (OSError, RuntimeError, json.JSONDecodeError):
            if attempt == 2:
                raise
            time.sleep(1.5 * (attempt + 1))
        finally:
            if request_path:
                Path(request_path).unlink(missing_ok=True)


def parse_calls(payload):
    calls = []
    message = payload["choices"][0]["message"]
    for call in message.get("tool_calls") or []:
        function = call.get("function") or {}
        raw_arguments = function.get("arguments", "{}")
        try:
            arguments = json.loads(raw_arguments)
        except json.JSONDecodeError:
            arguments = {"_invalid": raw_arguments}
        calls.append(
            {
                "name": function.get("name", ""),
                "arguments": arguments,
            }
        )
    return calls


def evaluate_case(model_config, tools, candidate, case):
    started = time.monotonic()
    body = {
        "model": model_config["model"],
        "messages": compose_messages(candidate, case),
        "thinking": {"type": "disabled"},
        "stream": False,
        "tools": tools,
        "tool_choice": "required",
    }
    try:
        payload = post_request(model_config, body)
        calls = parse_calls(payload)
        actual = [call["name"] for call in calls]
        passed = actual == case["expected_tools"]
        detail = ""
        if passed and actual == ["workflow_plan"]:
            goals = calls[0]["arguments"].get("goals")
            if not isinstance(goals, list) or len(goals) < 2:
                passed = False
                detail = "workflow_plan returned fewer than two goals"
            else:
                failed_goals = {
                    goal
                    for plan in case.get("fail_plans", [])
                    for goal in plan["goals"]
                }
                overlap = failed_goals.intersection(goals)
                if overlap:
                    passed = False
                    detail = f"workflow_plan repeated failed goals: {sorted(overlap)}"
        return {
            "case_id": case["id"],
            "suite": case["suite"],
            "category": case["category"],
            "depth": case["depth"],
            "family": case["family"],
            "source": case.get("source"),
            "expected": case["expected_tools"],
            "actual": actual,
            "calls": calls,
            "passed": passed,
            "detail": detail,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "usage": payload.get("usage"),
        }
    except Exception as error:
        return {
            "case_id": case["id"],
            "suite": case["suite"],
            "category": case["category"],
            "depth": case["depth"],
            "family": case["family"],
            "source": case.get("source"),
            "expected": case["expected_tools"],
            "actual": [],
            "calls": [],
            "passed": False,
            "detail": str(error),
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "usage": None,
        }


def smoke_batch(config, cases, batch):
    size = config["smoke"]["batch_size_per_category"]
    start = batch * size
    end = start + size
    selected = []
    for category in CATEGORIES:
        category_cases = [
            case for case in cases if case["category"] == category
        ]
        selected.extend(category_cases[start:end])
    expected = size * len(CATEGORIES)
    if len(selected) != expected:
        raise RuntimeError(
            f"smoke batch {batch} selected {len(selected)}, expected {expected}"
        )
    return selected


def metrics(results):
    values = {}
    for category in CATEGORIES:
        subset = [
            result for result in results if result["category"] == category
        ]
        passed = sum(bool(result["passed"]) for result in subset)
        values[category] = {
            "passed": passed,
            "total": len(subset),
            "rate": passed / len(subset) if subset else 0.0,
        }
    return values


def smoke_gate(config, batch):
    early = config["smoke"]["early_gates"].get(str(batch))
    return early or config["smoke"]["thresholds"]


def run_metadata(
    run_id,
    candidate_name,
    candidate_hash,
    tool_hash,
    manifests,
    model,
):
    return {
        "schema_version": 1,
        "run_id": run_id,
        "created_at": dt.datetime.now().isoformat(timespec="seconds"),
        "candidate": candidate_name,
        "candidate_sha256": candidate_hash,
        "tool_sha256": tool_hash,
        "suite_sha256": {
            suite: manifest["sha256"]
            for suite, manifest in manifests.items()
        },
        "model": model,
    }


def ensure_run(run_dir, expected):
    run_dir.mkdir(parents=True, exist_ok=True)
    metadata_path = run_dir / "run.json"
    if metadata_path.exists():
        existing = load_json(metadata_path)
        stable_keys = [
            "candidate",
            "candidate_sha256",
            "tool_sha256",
            "suite_sha256",
            "model",
        ]
        for key in stable_keys:
            if existing[key] != expected[key]:
                raise RuntimeError(
                    f"run {existing['run_id']} is frozen and {key} changed"
                )
        return existing
    metadata_path.write_bytes(
        (json.dumps(expected, ensure_ascii=False, indent=2) + "\n").encode(
            "utf-8"
        )
    )
    return expected


def existing_smoke_results(run_dir):
    results = []
    for path in sorted(run_dir.glob("smoke-batch-*.json")):
        results.extend(load_json(path)["results"])
    return results


def report(payload):
    lines = [
        f"# Prompt benchmark: {payload['suite']}",
        "",
        f"- Run: `{payload['run_id']}`",
        f"- Candidate: `{payload['candidate']}`",
        f"- Model: `{payload['model']}`",
        f"- Cases in invocation: {len(payload['results'])}",
        f"- Cumulative cases: {payload['cumulative_total']}",
        "",
        "| Category | Passed | Total | Rate | Gate |",
        "|---|---:|---:|---:|---:|",
    ]
    for category in CATEGORIES:
        value = payload["cumulative_metrics"][category]
        gate = payload["gate"].get(category)
        gate_text = f"{gate:.1%}" if gate is not None else "-"
        lines.append(
            f"| {category} | {value['passed']} | {value['total']} | "
            f"{value['rate']:.1%} | {gate_text} |"
        )
    lines.extend(
        [
            "",
            f"**Passed: {payload['passed']}**",
            "",
            "## Failures",
            "",
        ]
    )
    failures = [
        result for result in payload["results"] if not result["passed"]
    ]
    if not failures:
        lines.append("- None")
    else:
        for result in failures:
            lines.append(
                f"- `{result['case_id']}`: expected "
                f"`{','.join(result['expected'])}`, actual "
                f"`{','.join(result['actual']) or 'ERROR'}`"
                + (f"; {result['detail']}" if result["detail"] else "")
            )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("suite", choices=["smoke", "practice"])
    parser.add_argument("--candidate")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--batch", type=int, choices=range(10))
    parser.add_argument("--workers", type=int, default=6)
    args = parser.parse_args()

    config = load_json(ROOT / "benchmark.json")
    candidate_name = args.candidate or config["default_candidate"]
    candidate_path = ROOT / "prompts" / f"{candidate_name}.json"
    candidate_bytes = candidate_path.read_bytes()
    candidate = json.loads(candidate_bytes.decode("utf-8"))
    candidate_hash = sha256(candidate_bytes)

    tool_path = ROOT / "tools.json"
    tool_bytes = tool_path.read_bytes()
    tools = json.loads(tool_bytes.decode("utf-8-sig"))
    if len(tools) != config["tool_count"]:
        raise RuntimeError(
            f"expected {config['tool_count']} tools, found {len(tools)}"
        )
    tool_hash = sha256(tool_bytes)

    manifests = {}
    suites = {}
    for suite in ["smoke", "practice"]:
        manifests[suite], suites[suite] = verify_suite(config, suite)

    if args.suite == "smoke" and args.batch is None:
        raise RuntimeError("--batch is required for smoke")
    if args.suite == "practice" and args.batch is not None:
        raise RuntimeError("--batch is not used for practice")

    model_config = load_model_config()
    run_dir = RESULTS_ROOT / args.run_id
    metadata = run_metadata(
        args.run_id,
        candidate_name,
        candidate_hash,
        tool_hash,
        manifests,
        model_config["model"],
    )
    ensure_run(run_dir, metadata)

    if args.suite == "smoke":
        result_path = run_dir / f"smoke-batch-{args.batch:02d}.json"
        report_path = run_dir / f"smoke-batch-{args.batch:02d}.md"
        if result_path.exists() or report_path.exists():
            raise RuntimeError("this frozen smoke batch already exists")
        existing = sorted(run_dir.glob("smoke-batch-*.json"))
        expected = [
            run_dir / f"smoke-batch-{index:02d}.json"
            for index in range(args.batch)
        ]
        if existing != expected:
            raise RuntimeError("smoke batches must run sequentially from 0")
        selected = smoke_batch(config, suites["smoke"], args.batch)
    else:
        result_path = run_dir / "practice.json"
        report_path = run_dir / "practice.md"
        if result_path.exists() or report_path.exists():
            raise RuntimeError("practice already ran for this frozen run")
        selected = suites["practice"]

    results = []
    with concurrent.futures.ThreadPoolExecutor(
        max_workers=args.workers
    ) as executor:
        futures = [
            executor.submit(
                evaluate_case,
                model_config,
                tools,
                candidate,
                case,
            )
            for case in selected
        ]
        for index, future in enumerate(
            concurrent.futures.as_completed(futures), 1
        ):
            result = future.result()
            results.append(result)
            marker = "PASS" if result["passed"] else "FAIL"
            print(
                f"[{index:03d}/{len(futures):03d}] {marker} "
                f"{result['case_id']} -> "
                f"{','.join(result['actual']) or 'ERROR'}",
                flush=True,
            )
    results.sort(key=lambda value: (value["category"], value["case_id"]))

    if args.suite == "smoke":
        cumulative = existing_smoke_results(run_dir) + results
        cumulative_metrics = metrics(cumulative)
        gate = smoke_gate(config, args.batch)
        passed = all(
            cumulative_metrics[category]["rate"] >= gate[category]
            for category in CATEGORIES
        )
    else:
        cumulative = results
        cumulative_metrics = metrics(cumulative)
        gate = {
            category: config["practice"]["threshold"]
            for category in CATEGORIES
            if cumulative_metrics[category]["total"]
        }
        passed = all(result["passed"] for result in results)

    payload = {
        "schema_version": 1,
        "generated_at": dt.datetime.now().isoformat(timespec="seconds"),
        "run_id": args.run_id,
        "suite": args.suite,
        "batch": args.batch,
        "candidate": candidate_name,
        "candidate_sha256": candidate_hash,
        "case_sha256": manifests[args.suite]["sha256"],
        "tool_sha256": tool_hash,
        "model": model_config["model"],
        "gate": gate,
        "cumulative_metrics": cumulative_metrics,
        "cumulative_total": len(cumulative),
        "passed": passed,
        "results": results,
    }
    result_path.write_bytes(
        (json.dumps(payload, ensure_ascii=False, indent=2) + "\n").encode(
            "utf-8"
        )
    )
    report_path.write_text(report(payload), encoding="utf-8")
    print(f"RESULT={result_path}")
    print(f"REPORT={report_path}")
    print(f"PASSED={passed}")
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
