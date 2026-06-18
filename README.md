# Marix

Marix is a full-featured AI agent framework designed to build autonomous, tool-using agents powered by mainstream LLM backends — both local and online.

## Vision

Build a complete agent system that can:

- **Reason & Plan** — decompose complex tasks into actionable steps
- **Use Tools** — invoke external tools, APIs, and system commands
- **Maintain Context** — manage conversation history and working memory
- **Self-Correct** — observe results, detect failures, and retry with alternative strategies

## Supported Model Backends

Marix abstracts the LLM layer so agents can run on any of the following:

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
│            Agent Loop               │
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
├── core/           # Agent loop, planner, executor
├── models/         # Model adapters (OpenAI, Anthropic, local, etc.)
├── tools/          # Built-in tool implementations
├── memory/         # Context management and persistence
├── config/         # Configuration and model registry
└── main.py         # Entry point
```

## Getting Started

> 🚧 Under active development

```bash
# Clone
git clone https://github.com/DexterDreeeam/Marix.git
cd Marix

# Install dependencies
pip install -r requirements.txt

# Configure model backend
cp config/example.yaml config/local.yaml
# Edit config/local.yaml with your API keys or local model paths

# Run
python main.py
```

## License

MIT
