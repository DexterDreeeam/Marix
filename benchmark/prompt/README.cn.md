# Prompt Benchmark

该 Benchmark 使用接近生产环境的单阶段请求评测 Marix Workflow 选择：

- 同一个请求同时提供三个 Workflow Tool 和十四个普通工具；
- `tool_choice` 为 `required`；
- 模型必须返回 Native Tool Call；
- 不存在 `workflow_go` 或独立路由阶段；
- 本 Benchmark 不包含 Tool Call Summary。

Benchmark 包含两个 Suite：

| Suite | 用途 | 通过规则 |
|---|---|---|
| `smoke` | 广泛覆盖 Workflow 类别和 Context 深度的合成 Case | 按类别阈值 |
| `practice` | 从真实失败任务中收集的回归 Case | 每个 Case 必须通过 |

Practice 是 Smoke 的**必需前置条件**。必须先使用相同的 Run ID 和 Candidate 完整运行
Practice。只有 Practice 已经结束且 100% 通过后，才能开始 Smoke。如果 Practice
尚未完成或任意 Practice Case 失败，应立即停止；无论预期 Smoke 百分比如何，都没有继续
运行 Smoke 的必要。

## 必需指标

Smoke 最终累计结果必须满足：

| 类别 | 最低要求 |
|---|---:|
| `workflow_plan` | 95% |
| `workflow_complete` | 99% |
| `workflow_infeasible` | 50% |
| 普通工具 | 90% |

Practice 只有一条规则：**100% Case 必须通过**。只要一个 Practice Case 失败，
Benchmark 就失败，与 Smoke 百分比无关。

阈值保存在 `benchmark.json` 中，不写死在 Runner 里。

## 目录结构

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
│   └── candidate-012.json
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

生成的结果默认被 Git 忽略。

## Tool Surface

`tools.json` 保存固定的 17 Tool 生产 Tool Surface：

- `workflow_plan`
- `workflow_complete`
- `workflow_infeasible`
- 十四个普通工具

Workflow Tool 与其他 Tool 一样参与同一次选择。它们不会放在特殊请求里，也不会预先
调用独立分类器。

一次 Run 中途不得修改 `tools.json`。Runner 会记录其 SHA-256，后续 Batch 发现变化时
会拒绝继续。

## Smoke Suite

Smoke 包含 400 个 Case：

- 100 个 `workflow_plan`
- 100 个 `workflow_complete`
- 100 个 `workflow_infeasible`
- 100 个普通工具 Case

每类包含 25 个手工整理的语义家族，分别渲染为 Context Depth 1、2、3、4。因此四个
深度各有 100 个 Case。

Depth 含义：

1. 只有 Current Task。
2. 一个 Parent Plan + Current Task。
3. 两个 Parent Plan + Current Task。
4. 三个 Parent Plan + Current Task。

Parent Plan 包含已完成工作、正在执行的 Child 和后续 Parent-only 工作，用于测量模型
能否只为 Current Task 选择，而不吸收父级要求。

### 重新生成 Smoke

从仓库根目录执行：

```powershell
python benchmark\prompt\smoke\generate.py
```

Generator 会写入 `smoke/cases.json` 和 `smoke/manifest.json`。随后检查 Diff。
Case 内容变化时 Suite SHA-256 会有意发生变化。

## Practice Suite

Practice 保存真实 Task 和 Telemetry 中发生过的失败。Practice Case 可以属于任意
Workflow 或普通工具类别，但不计算平均值：每个 Case 都必须通过。

当前第一个 Case：

```text
complete-telemetry-098446153288258912138544578473665883451
```

它重现 Telemetry Request Key
`098446153288258912138544578473665883451` 和 Response Record `1921`。
Current Task 已获得所有 RFC 2324 事实，因此预期 Tool 是 `workflow_complete`。
生产环境观察到的错误响应却创建了包含重复检索和父级写文件工作的 Plan。

### 新增 Practice Case

1. 把完整 Model-visible Context 追加到 `practice/cases.json`。
2. 提供稳定且唯一的 `id`、`category`、`depth`、`family`、
   `expected_tools` 和全部渲染 Context 数据。
3. 如果可用，在 `source` 中加入 Telemetry Key、Record ID、Session/Task ID 和观察到的
   错误 Tool。
4. 刷新 Manifest：

   ```powershell
   python benchmark\prompt\update_manifest.py practice
   ```

5. 运行完整 Practice Suite。Practice 不允许分批或抽样。

禁止为了让 Prompt 通过而降低 Practice 预期，应修复 Candidate 或 Workflow 行为。

## Prompt Candidate

Candidate JSON 控制 Benchmark 中所有可修改 Prompt 段：

- 第一个 System Message；
- Workflow Policy System Message；
- Background Context Header；
- Current Task Header；
- Goal 和 Plan 标题；
- Completed Calls 标题和 Notice；
- Fail Plans 标题和 Reason Label。

`prompts/candidate-012.json` 是当前默认 Candidate。它在完整 Run
`candidate012-20260801-a` 中取得：

| 类别 | 已验证 400 Case 结果 |
|---|---:|
| Plan | 98% |
| Complete | 100% |
| Infeasible | 100% |
| Ordinary | 95% |

在开始 Smoke 前，必需的 Practice Suite 也已通过 1/1。该结果只适用于此 Run ID
记录的冻结 Candidate、Case、Tool 和 Model Hash；任何输入改变后都必须重新运行。

Candidate 开始一次 Run 后不得修改。测试其他 Prompt 组合时应创建新的 Candidate 文件和
新的 Run ID。

## Model 配置

Runner 要求 Python 3.11 或更高版本，并从 Marix TOML 配置读取选中的 Model：

```text
MARIX_CONFIG=<path-to-config.toml>
```

Runner 不会输出 API Key。由于 Windows 本机直连 DeepSeek Endpoint 曾出现 TLS 协商
失败，建议在已部署的 Server Host 上运行。

Linux 示例：

```bash
cd /path/to/Marix
MARIX_CONFIG=/opt/marix/server/config.toml \
  python3 benchmark/prompt/run.py practice \
  --run-id candidate012-20260801 \
  --candidate candidate-012
```

## 运行 Practice

Practice 必须最先且完整运行：

```powershell
$env:MARIX_CONFIG = 'C:\path\to\config.toml'
python benchmark\prompt\run.py practice `
  --run-id candidate012-20260801 `
  --candidate candidate-012
```

任意 Practice Case 失败时命令返回非零 Exit Code。

只有命令成功结束并且所有 Practice Case 均通过后，才能继续 Smoke。Practice 未完成或
失败时会阻断 Smoke。

## 分批运行 Smoke

在同一 Run ID 和 Candidate 的必需 Practice Suite 成功完成前，不应运行 Smoke。
不得用 Smoke 结果补偿 Practice 失败，也不得在 Practice 失败后继续测试。

Smoke 分十个 Batch，每个 Batch 从每类取十个 Case，共四十次调用。

```powershell
$env:MARIX_CONFIG = 'C:\path\to\config.toml'

python benchmark\prompt\run.py smoke `
  --run-id candidate012-20260801 `
  --candidate candidate-012 `
  --batch 0

python benchmark\prompt\run.py smoke `
  --run-id candidate012-20260801 `
  --candidate candidate-012 `
  --batch 1
```

只有累计 Gate 通过时才继续到 `--batch 9`。

用于节省 Token 的 Early Gate：

| Batch | Plan | Complete | Infeasible | Ordinary |
|---:|---:|---:|---:|---:|
| 0 | 70% | 80% | 30% | 70% |
| 1 累计 | 85% | 90% | 40% | 80% |
| 2–9 累计 | 95% | 99% | 50% | 90% |

Batch 未通过 Gate 时停止该 Candidate。不得修改 Candidate 后继续复用同一个 Run ID；
应创建新 Candidate 和 Run ID。

## Frozen Run 保证

每个 Run ID 会创建 `results/<run-id>/run.json`，记录：

- Candidate 文件名和 SHA-256；
- Smoke 和 Practice Case SHA-256；
- Tool List SHA-256；
- Model Name。

后续 Batch 发现任意冻结输入变化时会 Fail Closed。Smoke Batch 必须从 0 连续运行且不能
覆盖；每个 Run ID 只能运行一次 Practice。

## 计分规则

返回的 Tool Name List 与 `expected_tools` 完全相等时，Case 才通过。

Plan 还需满足：

- 至少返回两个 Goal；
- Goal 不得完全重复 Fail Plan Goal。

一个单 Tool Case 返回多个普通工具时失败。Workflow Tool 正确但 Plan Body 无效时也失败。

## 报告

每次调用会在以下目录写入 JSON 和 Markdown：

```text
benchmark/prompt/results/<run-id>/
```

JSON 保留 Call、Arguments、Token Usage、耗时、Source Metadata 和失败详情；
Markdown 提供累计类别指标和失败列表。

Result 是本地 Artifact，默认不提交。

## 维护规则

- `README.md` 与 `README.cn.md` 必须同步。
- 一次 Run 中不得修改 Candidate。
- Smoke 或 Practice Case 改动后必须刷新对应 Manifest。
- Practice 必须在 Smoke 前全量运行，不得使用百分比或抽样；Practice 未完成或失败时
  不得运行 Smoke。
- 三个 Workflow Tool 与普通工具始终放在同一个请求中。
- 本 Benchmark 不得加入 Tool Call Summary。
- Practice 应保留真实失败的原始 Context，只能移除敏感信息。
