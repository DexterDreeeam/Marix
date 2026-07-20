# Marix

Marix 是一个功能完整的工具调用型 AI 运行时，底层支持主流的本地和在线大模型。

## 愿景

构建一个完整的编排系统，具备以下能力：

- **推理与规划** — 将复杂任务分解为可执行的步骤
- **工具使用** — 调用外部工具、API 和系统命令
- **上下文管理** — 管理对话历史和工作记忆
- **自我纠错** — 观察执行结果，检测失败并用替代策略重试

## 支持的模型后端

Marix 抽象了 LLM 层，Server 可以运行在以下任意后端上：

| 类型 | 提供商 | 模型 |
|------|--------|------|
| 在线 | OpenAI | GPT-4o, GPT-4-turbo, o1/o3 系列 |
| 在线 | Anthropic | Claude Sonnet, Claude Opus |
| 在线 | Google | Gemini Pro, Gemini Flash |
| 在线 | DeepSeek | DeepSeek-V3, DeepSeek-R1 |
| 本地 | Ollama | Llama 3, Qwen 2.5, Mistral 等 |
| 本地 | llama.cpp | 通过 HTTP 服务加载 GGUF 模型 |
| 本地 | vLLM | 任意 HuggingFace 模型 |

## 架构

```
┌─────────────────────────────────────┐
│          Server 编排循环             │
│  (规划 → 执行 → 观察 → 反思)        │
├─────────────────────────────────────┤
│           工具注册表                  │
│    (内置工具 + 用户自定义工具)        │
├─────────────────────────────────────┤
│         记忆 / 上下文                 │
│    (短期记忆 + 长期存储)             │
├─────────────────────────────────────┤
│         模型适配层                    │
│    (所有 LLM 的统一接口)             │
├──────────┬──────────┬───────────────┤
│  OpenAI  │ Anthropic│  本地/其他     │
└──────────┴──────────┴───────────────┘
```

## 项目结构

```
marix/
├── src/
│   ├── server/     # Server 编排、模型、会话、计划、步骤与任务运行时
│   ├── host/       # Host 侧工具执行
│   ├── client/     # 用户侧 Client 入口
│   ├── common/     # 共享配置、日志、外部适配与结构
│   ├── protocol/   # 共享协议数据契约
│   ├── prompt/     # 模型 prompt 模板
│   └── tool/       # 原生工具可执行程序
└── overview/       # GitHub Pages 总览 UI
```

## 快速开始

> 🚧 正在积极开发中

```bash
# 克隆
git clone https://github.com/DexterDreeeam/Marix.git
cd Marix

# 构建 Rust crate
cargo build
```

## 部署拓扑

- Server Telemetry 与 Server 部署在 Ubuntu 服务器上。部署后先启动
  Server Telemetry，再启动 Server。
- Host 只部署到 Hyper-V guest `Marix_TestVm` 的 `C:\MarixHost\`，并在
  Server 之后启动。
- Client 只部署在本地物理机上。部署流程不会把 Client artifacts 复制到
  Hyper-V guest，也不会启动 Client；Client 由用户手动启动。

## 许可证

MIT
