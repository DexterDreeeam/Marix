# Marix

Marix is a full-featured tool-using AI runtime powered by mainstream LLM backends — both local and online.

## Vision

Build a complete orchestration system that can:

- **Reason & Plan** — decompose complex tasks into actionable steps
- **Use Tools** — invoke external tools, APIs, and system commands
- **Maintain Context** — manage conversation history and working memory
- **Self-Correct** — observe results, detect failures, and retry with alternative strategies

## Supported Model Backends

Marix abstracts the LLM layer so the server can run on any of the following:

| Type | Provider | Models |
|------|----------|--------|
| Online | OpenAI | GPT-4o, GPT-4-turbo, o1/o3 series |
| Online | Anthropic | Claude Sonnet, Claude Opus |
| Online | Google | Gemini Pro, Gemini Flash |
| Online | DeepSeek | DeepSeek-V3, DeepSeek-R1 |
| Local | Ollama | Llama 3, Qwen 2.5, Mistral, etc. |
| Local | llama.cpp | GGUF models via HTTP server |
| Local | vLLM | Any HuggingFace model |

## Architecture

```
┌─────────────────────────────────────┐
│       Server Orchestration Loop     │
│  (Plan → Act → Observe → Reflect)  │
├─────────────────────────────────────┤
│          Tool Registry              │
│   (built-in + user-defined tools)   │
├─────────────────────────────────────┤
│        Memory / Context             │
│  (short-term + long-term storage)   │
├─────────────────────────────────────┤
│      Model Adapter Layer            │
│  (unified interface to all LLMs)    │
├──────────┬──────────┬───────────────┤
│  OpenAI  │ Anthropic│  Local/Other  │
└──────────┴──────────┴───────────────┘
```

## Project Structure

```
marix/
├── src/
│   ├── server/     # Server orchestration, model, session, plan, step, and task runtime
│   ├── host/       # Host-side tool execution
│   ├── client/     # User-facing client entrypoints
│   ├── common/     # Shared config, logging, external adapters, and structures
│   ├── protocol/   # Shared protocol data contracts
│   ├── prompt/     # Model prompt templates
│   └── tool/       # Native tool executables
└── overview/       # GitHub Pages overview UI
```

## Getting Started

> 🚧 Under active development

```bash
# Clone
git clone https://github.com/DexterDreeeam/Marix.git
cd Marix

# Build Rust crate
cargo build
```

## Deployment Topology

- Server Telemetry and Server are deployed on the Ubuntu server. After deployment,
  start Server Telemetry first and poll its collector TCP port until a connection
  succeeds, with a finite total timeout and explicit failure. A systemd active
  state or `After=` ordering alone is not readiness. Only then start Server and
  confirm that its systemd unit is active.
- Host is deployed only into the Hyper-V guest `Marix_TestVm` under
  `C:\MarixHost\`, and starts only after the Server active-state gate succeeds.
- Client is deployed only on the local physical machine. Deployment never copies
  Client artifacts into the Hyper-V guest and never starts Client; the user
  starts Client manually.
- Prefer loopback for Ubuntu's Telemetry path only when role-specific config
  generation can independently preserve the public Server address used by Host
  and Client. Never change a shared endpoint address to loopback for every role.

## License

MIT
