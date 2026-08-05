import argparse
import concurrent.futures
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import tomllib


ROOT = Path(__file__).resolve().parent
RESULTS_ROOT = ROOT / "results"
TEMP_ROOT = ROOT / ".temp"
STAGES = [
    "planning",
    "tool_calling",
    "replan",
    "infeasible",
    "subintent_complete",
    "complete",
]
NAME_PATTERN = re.compile(r"[a-z0-9]+(_[a-z0-9]+)*")
CURL = shutil.which("curl") or shutil.which("curl.exe")


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def load_model_config():
    config_path = os.environ.get("MARIX_CONFIG", "").strip()
    if not config_path:
        raise RuntimeError(
            "MARIX_CONFIG must point to a config with resolved credentials"
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


def load_cases(config, suite):
    root = ROOT / config[suite]["case_dir"]
    cases = []
    for stage in STAGES:
        stage_dir = root / stage
        if not stage_dir.is_dir():
            raise RuntimeError(f"missing case directory: {stage_dir}")
        for path in sorted(stage_dir.glob("*.json")):
            case = load_json(path)
            if case.get("id") != path.stem:
                raise RuntimeError(f"{path}: id must equal file stem")
            if case.get("stage") != stage:
                raise RuntimeError(f"{path}: stage must equal directory")
            if not NAME_PATTERN.fullmatch(path.stem):
                raise RuntimeError(f"{path}: name is not snake_case")
            if len(path.stem.split("_")) > 5:
                raise RuntimeError(f"{path}: name exceeds five segments")
            cases.append(case)
    return cases


def render_context(case):
    sections = [
        "[INTENT]",
        case["intent"],
    ]
    if case.get("plan"):
        sections.extend(
            [
                "",
                "[PLAN]",
                "\n".join(
                    f"{index}. [{item['status']}] {item['goal']}"
                    for index, item in enumerate(case["plan"], 1)
                ),
            ]
        )
    if case.get("subintent_results"):
        sections.extend(
            [
                "",
                "[SUBINTENT RESULTS]",
                "\n".join(
                    f"{index}. {item['goal']} => {item['result']}"
                    for index, item in enumerate(
                        case["subintent_results"], 1
                    )
                ),
            ]
        )
    if case.get("tool_calls"):
        sections.extend(
            [
                "",
                "[TOOL CALLS]",
                "\n".join(
                    f"{index}. {item['tool']}({item['arguments']}) => "
                    f"{item['result']}"
                    for index, item in enumerate(case["tool_calls"], 1)
                ),
            ]
        )
    if case.get("failed_plans"):
        sections.extend(
            [
                "",
                "[FAILED PLANS]",
                "\n".join(
                    f"{index}. {item}"
                    for index, item in enumerate(case["failed_plans"], 1)
                ),
            ]
        )
    if case.get("tool_call_count") is not None:
        sections.extend(
            [
                "",
                f"[TOOL CALL COUNT] {case['tool_call_count']}",
            ]
        )
    return "\n".join(sections)


def compose_messages(candidate, case):
    context = render_context(case)
    contract = candidate["stage_prompts"][case["stage"]]
    messages = [
        {"role": "system", "content": candidate["system"]},
        {"role": "user", "content": context},
    ]
    role = candidate.get("contract_role", "user")
    messages.append({"role": role, "content": contract})
    return messages


def post_request(model_config, body):
    if not CURL:
        raise RuntimeError("curl is unavailable")
    TEMP_ROOT.mkdir(exist_ok=True)
    for attempt in range(3):
        request_path = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                suffix=".json",
                prefix="stage-benchmark-",
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


def parse_tool_calls(payload):
    message = payload["choices"][0]["message"]
    calls = []
    for call in message.get("tool_calls") or []:
        function = call.get("function") or {}
        raw = function.get("arguments", "{}")
        try:
            arguments = json.loads(raw)
        except json.JSONDecodeError:
            arguments = {"_invalid": raw}
        calls.append(
            {"name": function.get("name", ""), "arguments": arguments}
        )
    return calls


def parse_json_content(payload):
    content = payload["choices"][0]["message"].get("content") or ""
    return json.loads(content)


def decision_for_boolean(stage, value):
    mapping = {
        "planning": ("should_plan", "plan", "tool_calling"),
        "replan": ("should_replan", "replan", "infeasible_review"),
        "infeasible": ("is_infeasible", "infeasible", "continue"),
        "subintent_complete": ("can_complete", "complete", "replan"),
        "complete": ("is_complete", "complete", "continue"),
    }
    field, yes, no = mapping[stage]
    if field not in value or not isinstance(value[field], bool):
        raise RuntimeError(f"missing boolean field {field}")
    return yes if value[field] else no


def decision_for_next_stage(stage, value):
    next_stage = value.get("next_stage")
    mapping = {
        ("planning", "subintent_iteration"): "plan",
        ("planning", "tool_calling"): "tool_calling",
        ("replan", "subintent_iteration"): "replan",
        ("replan", "infeasible_review"): "infeasible_review",
        ("infeasible", "infeasible"): "infeasible",
        ("infeasible", "tool_calling"): "continue",
        ("subintent_complete", "complete"): "complete",
        ("subintent_complete", "replan"): "replan",
        ("complete", "complete"): "complete",
        ("complete", "tool_calling"): "continue",
    }
    return mapping.get((stage, next_stage), next_stage)


def decision_for_marker(stage, value):
    token = value.get("token")
    mapping = {
        ("planning", "Intent-Plan"): "plan",
        ("planning", "Intent-ToolCalling"): "tool_calling",
        ("replan", "Intent-Replan"): "replan",
        ("replan", "Intent-InfeasibleReview"): "infeasible_review",
        ("infeasible", "Intent-Infeasible"): "infeasible",
        ("infeasible", "Intent-Continue"): "continue",
        ("subintent_complete", "Intent-Complete"): "complete",
        ("subintent_complete", "Intent-Replan"): "replan",
        ("complete", "Intent-Complete"): "complete",
        ("complete", "Intent-Continue"): "continue",
    }
    return mapping.get((stage, token), token)


def validate_json(candidate, case, value):
    style = candidate["style"]
    if not isinstance(value, dict):
        return False, None, "response is not a JSON object"
    if not isinstance(value.get("reason"), str) or not value["reason"].strip():
        return False, None, "reason is missing"

    if style == "boolean":
        decision = decision_for_boolean(case["stage"], value)
    elif style == "next_stage":
        decision = decision_for_next_stage(case["stage"], value)
    elif style == "marker":
        decision = decision_for_marker(case["stage"], value)
    else:
        decision = value.get("decision")

    expected = case["expected"]["decision"]
    if decision != expected:
        return False, decision, f"expected {expected}, got {decision}"

    if decision == "plan":
        items = value.get("subintents")
        if not isinstance(items, list) or len(items) < 2:
            return False, decision, "subintents must contain at least two items"
        if any(
            not isinstance(item, dict)
            or not isinstance(item.get("goal"), str)
            or not item["goal"].strip()
            for item in items
        ):
            return False, decision, "subintents contain an invalid goal"
    if case["stage"] == "replan" and decision == "replan":
        items = value.get("subintents")
        if not isinstance(items, list) or not items:
            return False, decision, "replan must contain a new subintent"

    if case["stage"] in ("complete", "subintent_complete"):
        summary = value.get("summary") or value.get("context_summary") or ""
        if decision == "complete" and not summary.strip():
            return False, decision, "completion summary is missing"
        missing = [
            token
            for token in case["expected"].get("summary_contains", [])
            if token not in summary
        ]
        if missing:
            return (
                False,
                decision,
                f"summary misses required values: {missing}",
            )

    if style == "requirements" and case["stage"] in (
        "complete",
        "subintent_complete",
    ):
        requirements = value.get("requirements")
        if not isinstance(requirements, list) or not requirements:
            return False, decision, "requirements inventory is missing"
        if any(
            item.get("status") not in ("satisfied", "missing")
            for item in requirements
            if isinstance(item, dict)
        ):
            return False, decision, "requirements contain an invalid status"
    return True, decision, ""


def evaluate_case(model_config, tools, candidate, case, temperature):
    started = time.monotonic()
    body = {
        "model": model_config["model"],
        "messages": compose_messages(candidate, case),
        "thinking": {"type": "disabled"},
        "stream": False,
        "tools": tools,
        "temperature": temperature,
    }
    try:
        if case["stage"] == "tool_calling":
            body["tool_choice"] = "required"
            payload = post_request(model_config, body)
            calls = parse_tool_calls(payload)
            actual = [call["name"] for call in calls]
            expected = case["expected"]["tools"]
            passed = actual == expected
            detail = "" if passed else f"expected {expected}, got {actual}"
            output = calls
        else:
            body["tool_choice"] = "none"
            if candidate.get("json_mode", True):
                body["response_format"] = {"type": "json_object"}
            payload = post_request(model_config, body)
            output = parse_json_content(payload)
            passed, actual, detail = validate_json(
                candidate, case, output
            )
        return {
            "case_id": case["id"],
            "stage": case["stage"],
            "passed": passed,
            "actual": actual,
            "detail": detail,
            "output": output,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "usage": payload.get("usage"),
        }
    except Exception as error:
        return {
            "case_id": case["id"],
            "stage": case["stage"],
            "passed": False,
            "actual": None,
            "detail": str(error),
            "output": None,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "usage": None,
        }


def report(payload):
    lines = [
        f"# Stage prompt benchmark: {payload['suite']}",
        "",
        f"- Run: `{payload['run_id']}`",
        f"- Candidate: `{payload['candidate']}`",
        f"- Model: `{payload['model']}`",
        f"- Cases: {len(payload['results'])}",
        "",
        "| Stage | Passed | Total | Rate |",
        "|---|---:|---:|---:|",
    ]
    for stage in STAGES:
        value = payload["metrics"][stage]
        lines.append(
            f"| {stage} | {value['passed']} | {value['total']} | "
            f"{value['rate']:.1%} |"
        )
    lines.extend(["", f"**Passed: {payload['passed']}**", "", "## Failures", ""])
    failures = [result for result in payload["results"] if not result["passed"]]
    if not failures:
        lines.append("- None")
    else:
        for result in failures:
            lines.append(
                f"- `{result['case_id']}`: `{result['actual']}`; "
                f"{result['detail']}"
            )
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("suite", choices=["required", "guide"])
    parser.add_argument("--candidate")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args()

    config = load_json(ROOT / "benchmark.json")
    candidate_name = args.candidate or config["default_candidate"]
    candidate = load_json(ROOT / "prompts" / f"{candidate_name}.json")
    tools = load_json(ROOT / "tools.json")
    if len(tools) != config["tool_count"]:
        raise RuntimeError("tool_count does not match tools.json")
    cases = load_cases(config, args.suite)
    model_config = load_model_config()

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
                config["temperature"],
            )
            for case in cases
        ]
        for index, future in enumerate(
            concurrent.futures.as_completed(futures), 1
        ):
            result = future.result()
            results.append(result)
            marker = "PASS" if result["passed"] else "FAIL"
            print(
                f"[{index:03d}/{len(futures):03d}] {marker} "
                f"{result['case_id']} -> {result['actual']}",
                flush=True,
            )
    results.sort(key=lambda value: (value["stage"], value["case_id"]))

    metrics = {}
    for stage in STAGES:
        subset = [result for result in results if result["stage"] == stage]
        passed = sum(bool(result["passed"]) for result in subset)
        metrics[stage] = {
            "passed": passed,
            "total": len(subset),
            "rate": passed / len(subset) if subset else 0.0,
        }
    threshold = config[args.suite]["threshold"]
    passed = all(
        value["rate"] >= threshold
        for value in metrics.values()
        if value["total"]
    )
    payload = {
        "schema_version": 1,
        "generated_at": dt.datetime.now().isoformat(timespec="seconds"),
        "run_id": args.run_id,
        "suite": args.suite,
        "candidate": candidate_name,
        "candidate_sha256": sha256(
            (ROOT / "prompts" / f"{candidate_name}.json").read_bytes()
        ),
        "model": model_config["model"],
        "temperature": config["temperature"],
        "metrics": metrics,
        "passed": passed,
        "results": results,
    }
    run_dir = RESULTS_ROOT / args.run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    result_path = run_dir / f"{args.suite}.json"
    report_path = run_dir / f"{args.suite}.md"
    if result_path.exists() or report_path.exists():
        raise RuntimeError("this run id and suite already exist")
    result_path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    report_path.write_text(report(payload), encoding="utf-8")
    print(f"RESULT={result_path}")
    print(f"REPORT={report_path}")
    print(f"PASSED={passed}")
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
