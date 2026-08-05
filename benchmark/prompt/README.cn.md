# Intent 阶段 Prompt Benchmark

这个 benchmark 在不修改 Marix 源码的前提下，测试新的六阶段 Intent
控制器：

1. `PlanningStage`
2. `ToolCallingStage`
3. `ReplanStage`
4. `InfeasibleStage`
5. `SubIntentCompleteStage`
6. `CompleteStage`

每个请求都会继续传递完整 ordinary tool schema。只有
`ToolCallingStage` 使用 `tool_choice: required`；其他 stage 使用
`tool_choice: none` 并返回 JSON。这样既能保证非工具 stage 不调用工具，也为未来
prompt cache 保持相同的 tool surface。

## 运行

必须在 Ubuntu server 上运行，不要在 Windows host 上运行：

```bash
export MARIX_CONFIG=/root/marix-stage-benchmark/marix-config.toml
python3 run.py required --candidate candidate-008 --run-id check-001
python3 run.py guide --candidate candidate-008 --run-id check-001
```

`benchmark.json` 将 `temperature` 固定为 0。

## Suite

- `required`：每个 stage 两个关键 case，必须全部通过。
- `guide`：覆盖更多 stage 边界和 summary 事实保留。

每个 case 是一个独立 JSON 文件：

```text
cases/<suite>/<stage>/<meaningful_name>.json
```

文件名就是 case id，必须使用 snake_case，最多五个下划线分段。

## 候选协议

| Candidate | 协议 | 结果 |
|---|---|---|
| 001 | stage 专属判别联合 JSON | refinement 前 42/44 |
| 002 | stage 专属布尔 JSON | summary 会丢具体值 |
| 003 | 统一 `next_stage` JSON | refinement 前 42/44 |
| 004 | requirements-first completion JSON | evidence 较好，但 summary 被压缩 |
| 005 | union contract 作为 system message | Replan 过度保守 |
| 006 | 不启用 provider JSON mode | Planning 边界回退 |
| 007 | JSON 内固定 `Intent-*` token | Planning 和 summary 都较弱 |
| **008** | **统一 `next_stage` JSON + 两个边界修正** | **56/56，连续三轮** |
| 009 | 修正后的判别联合 JSON | 55/56 |

Candidate 008 可直接映射未来的 `IntentRuntime::run_stage`：runtime 根据
`next_stage` 驱动状态迁移，stage payload 携带 `subintents`、
`context_summary` 或 `summary`。

## 重新生成

```bash
python3 generate_cases.py
python3 generate_candidates.py
```

生成过程是确定性的，不会调用模型。
