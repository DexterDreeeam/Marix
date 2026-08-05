import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent

SYSTEM = (
    "You control exactly one execution stage of one Intent. Evaluate only "
    "the supplied Intent state and the named stage. Do not perform work from "
    "a later stage. Follow the stage output protocol exactly."
)

UNION = {
    "planning": (
        "[PlanningStage]\n"
        "Decide whether the Intent needs child Intents. Use `plan` only when "
        "the goal contains at least two distinct operations, dependent "
        "stages, or independent targets. Use `tool_calling` when one ordinary "
        "tool invocation can advance the whole goal.\n"
        "Return JSON only. Plan: "
        '{"decision":"plan","reason":"...","subintents":[{"goal":"..."}]}. '
        "Direct execution: "
        '{"decision":"tool_calling","reason":"..."}'
    ),
    "tool_calling": (
        "[ToolCallingStage]\n"
        "Select exactly one supplied ordinary tool whose single invocation "
        "advances the current Intent. Return only its native function call. "
        "Do not return JSON text and do not claim completion."
    ),
    "replan": (
        "[ReplanStage]\n"
        "The prior Plan contains an infeasible child. Use `replan` only when "
        "a materially different feasible Plan exists; preserve completed "
        "facts and do not repeat a failed Plan. Otherwise use "
        "`infeasible_review` so a separate stage can judge final "
        "infeasibility.\nReturn JSON only. Replan: "
        '{"decision":"replan","reason":"...","subintents":[{"goal":"..."}]}. '
        "Review infeasibility: "
        '{"decision":"infeasible_review","reason":"..."}'
    ),
    "infeasible": (
        "[InfeasibleStage]\n"
        "Decide whether a permanent blocker makes this Intent impossible. "
        "Use `continue` for an untried tool, uninspected input, retryable "
        "error, or any remaining execution path. Use `infeasible` only when "
        "no feasible path exists.\nReturn JSON only: "
        '{"decision":"continue|infeasible","reason":"..."}'
    ),
    "subintent_complete": (
        "[SubIntentCompleteStage]\n"
        "Judge whether all completed child results fully satisfy this parent "
        "Intent. Use `complete` only when every requirement and side effect "
        "is covered; return a context summary containing concrete facts useful "
        "to later Plans. Otherwise use `replan`.\nReturn JSON only: "
        '{"decision":"complete|replan","reason":"...",'
        '"context_summary":"..."}'
    ),
    "complete": (
        "[CompleteStage]\n"
        "Judge whether the ordinary tool results fully satisfy every current "
        "Intent requirement. Use `complete` only when all facts and side "
        "effects are present; preserve concrete values in the summary. "
        "Otherwise use `continue` and summarize what the latest call added.\n"
        "Return JSON only: "
        '{"decision":"complete|continue","reason":"...",'
        '"summary":"..."}'
    ),
}

BOOLEAN = {
    **UNION,
    "planning": (
        "[PlanningStage]\n"
        "Decide whether the Intent needs child Intents. Planning is required "
        "for at least two distinct operations, dependent stages, or "
        "independent targets. One ordinary tool invocation means no Plan.\n"
        "Return JSON only: "
        '{"should_plan":true|false,"reason":"...",'
        '"subintents":[{"goal":"..."}]}. '
        "Use an empty subintents array when false."
    ),
    "replan": (
        "[ReplanStage]\n"
        "Decide whether a materially different feasible Plan exists after "
        "the failed child. Preserve completed facts and never repeat a failed "
        "Plan.\nReturn JSON only: "
        '{"should_replan":true|false,"reason":"...",'
        '"subintents":[{"goal":"..."}]}. '
        "False requests a later infeasibility review."
    ),
    "infeasible": (
        "[InfeasibleStage]\n"
        "A permanent blocker with no remaining path is infeasible. An untried "
        "tool, uninspected input, retryable error, or remaining path is still "
        "feasible.\nReturn JSON only: "
        '{"is_infeasible":true|false,"reason":"..."}'
    ),
    "subintent_complete": (
        "[SubIntentCompleteStage]\n"
        "Judge whether completed child results satisfy every parent Intent "
        "requirement and side effect.\nReturn JSON only: "
        '{"can_complete":true|false,"reason":"...",'
        '"context_summary":"..."}'
    ),
    "complete": (
        "[CompleteStage]\n"
        "Judge whether the ordinary tool results satisfy every current Intent "
        "requirement. Preserve concrete values.\nReturn JSON only: "
        '{"is_complete":true|false,"reason":"...","summary":"..."}'
    ),
}

NEXT_STAGE = {
    **UNION,
    "planning": (
        "[PlanningStage]\n"
        "Choose `subintent_iteration` when at least two distinct operations, "
        "dependent stages, or independent targets require a Plan. Choose "
        "`tool_calling` when one ordinary tool invocation advances the goal.\n"
        "Return JSON only: "
        '{"next_stage":"subintent_iteration|tool_calling","reason":"...",'
        '"subintents":[{"goal":"..."}]}'
    ),
    "replan": (
        "[ReplanStage]\n"
        "Choose `subintent_iteration` with a materially different Plan when "
        "a feasible alternative exists. Otherwise choose "
        "`infeasible_review`.\nReturn JSON only: "
        '{"next_stage":"subintent_iteration|infeasible_review",'
        '"reason":"...","subintents":[{"goal":"..."}]}'
    ),
    "infeasible": (
        "[InfeasibleStage]\n"
        "Choose `infeasible` only for a permanent blocker with no remaining "
        "path. Otherwise choose `tool_calling`.\nReturn JSON only: "
        '{"next_stage":"infeasible|tool_calling","reason":"..."}'
    ),
    "subintent_complete": (
        "[SubIntentCompleteStage]\n"
        "Choose `complete` only when child results cover every parent "
        "requirement; otherwise choose `replan`. Preserve useful facts.\n"
        "Return JSON only: "
        '{"next_stage":"complete|replan","reason":"...",'
        '"context_summary":"..."}'
    ),
    "complete": (
        "[CompleteStage]\n"
        "Choose `complete` only when tool results cover every Intent "
        "requirement; otherwise choose `tool_calling`. Preserve useful facts.\n"
        "Return JSON only: "
        '{"next_stage":"complete|tool_calling","reason":"...",'
        '"summary":"..."}'
    ),
}

REQUIREMENTS = {
    **UNION,
    "subintent_complete": (
        "[SubIntentCompleteStage]\n"
        "Inventory every parent Intent requirement before deciding. Mark each "
        "as satisfied or missing and cite the child result that supports it. "
        "Use `complete` only when none are missing; otherwise use `replan`.\n"
        "Return JSON only: "
        '{"requirements":[{"requirement":"...","status":"satisfied|missing",'
        '"evidence":"..."}],"decision":"complete|replan","reason":"...",'
        '"context_summary":"..."}'
    ),
    "complete": (
        "[CompleteStage]\n"
        "Inventory every current Intent requirement before deciding. Mark "
        "each as satisfied or missing and cite the tool result that supports "
        "it. Use `complete` only when none are missing; otherwise use "
        "`continue`.\nReturn JSON only: "
        '{"requirements":[{"requirement":"...","status":"satisfied|missing",'
        '"evidence":"..."}],"decision":"complete|continue","reason":"...",'
        '"summary":"..."}'
    ),
}

MARKER = {
    **UNION,
    "planning": (
        "[PlanningStage]\n"
        "Decide whether the Intent needs child Intents. Plan for at least two "
        "distinct operations, dependent stages, or independent targets; use "
        "ToolCalling for one ordinary-tool operation.\nReturn JSON only. "
        "Plan: "
        '{"token":"Intent-Plan","reason":"...",'
        '"subintents":[{"goal":"..."}]}. Direct execution: '
        '{"token":"Intent-ToolCalling","reason":"..."}'
    ),
    "replan": (
        "[ReplanStage]\n"
        "Return Intent-Replan when a materially different feasible Plan "
        "exists; otherwise request an infeasibility review. Preserve completed "
        "facts and do not repeat failed Plans.\nReturn JSON only. Replan: "
        '{"token":"Intent-Replan","reason":"...",'
        '"subintents":[{"goal":"..."}]}. Review infeasibility: '
        '{"token":"Intent-InfeasibleReview","reason":"..."}'
    ),
    "infeasible": (
        "[InfeasibleStage]\n"
        "Return Intent-Infeasible only for a permanent blocker with no "
        "remaining path. An untried tool, uninspected input, retryable error, "
        "or remaining path returns Intent-Continue.\nReturn JSON only: "
        '{"token":"Intent-Infeasible","reason":"..."} or '
        '{"token":"Intent-Continue","reason":"..."}'
    ),
    "subintent_complete": (
        "[SubIntentCompleteStage]\n"
        "Return Intent-Complete only when child results cover every parent "
        "requirement and side effect; otherwise return Intent-Replan. A "
        "completion must preserve concrete facts for later Plans.\nReturn "
        "JSON only with token `Intent-Complete` or `Intent-Replan`: "
        '{"token":"...","reason":"...","context_summary":"..."}'
    ),
    "complete": (
        "[CompleteStage]\n"
        "Return Intent-Complete only when ordinary-tool results cover every "
        "Intent requirement and side effect. Otherwise return Intent-Continue "
        "and preserve what the latest result added.\nReturn JSON only with "
        "token `Intent-Complete` or `Intent-Continue`: "
        '{"token":"...","reason":"...","summary":"..."}'
    ),
}


def write(number, name, style, prompts, role="user", json_mode=True):
    payload = {
        "name": name,
        "style": style,
        "contract_role": role,
        "json_mode": json_mode,
        "system": SYSTEM,
        "stage_prompts": prompts,
    }
    path = ROOT / "prompts" / f"candidate-{number:03d}.json"
    path.parent.mkdir(exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def main():
    write(1, "union-user-json-mode", "union", UNION)
    write(2, "boolean-user-json-mode", "boolean", BOOLEAN)
    write(3, "next-stage-user-json-mode", "next_stage", NEXT_STAGE)
    write(4, "requirements-user-json-mode", "requirements", REQUIREMENTS)
    write(5, "union-system-json-mode", "union", UNION, role="system")
    write(6, "union-user-prompt-only", "union", UNION, json_mode=False)
    write(7, "marker-user-json-mode", "marker", MARKER)

    next_stage_refined = dict(NEXT_STAGE)
    next_stage_refined["planning"] = (
        NEXT_STAGE["planning"]
        + "\nStarting a process and later waiting for or reading its output "
        "are dependent stages, so that goal requires subintent_iteration."
    )
    next_stage_refined["subintent_complete"] = (
        NEXT_STAGE["subintent_complete"]
        + "\nA generated but unsaved change does not satisfy a requested "
        "side effect, even if the generated version was inspected or tested."
    )
    write(
        8,
        "next-stage-refined-boundaries",
        "next_stage",
        next_stage_refined,
    )

    union_refined = dict(UNION)
    union_refined["planning"] = (
        UNION["planning"]
        + "\nStarting a process and later waiting for or reading its output "
        "are dependent stages, so that goal requires plan."
    )
    union_refined["subintent_complete"] = (
        UNION["subintent_complete"]
        + "\nA generated but unsaved change does not satisfy a requested "
        "side effect, even if the generated version was inspected or tested."
    )
    write(9, "union-refined-boundaries", "union", union_refined)
    print("generated prompt candidates")


if __name__ == "__main__":
    main()
