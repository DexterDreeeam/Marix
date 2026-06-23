# Agent implementation research index

This directory collects source-level research notes for leading open-source agent implementations and agent architecture references. Each entry has an English document and a synchronized Chinese version.

## Coding agents

| Project | Research notes |
|---|---|
| OpenHands | [English](openhands.md) / [中文](openhands.cn.md) |
| Cline | [English](cline.md) / [中文](cline.cn.md) |
| Aider | [English](aider.md) / [中文](aider.cn.md) |
| OpenCode | [English](opencode.md) / [中文](opencode.cn.md) |
| Goose | [English](goose.md) / [中文](goose.cn.md) |
| Continue | [English](continue.md) / [中文](continue.cn.md) |

## Agent frameworks and platforms

| Project | Research notes |
|---|---|
| AutoGPT | [English](autogpt.md) / [中文](autogpt.cn.md) |
| CrewAI | [English](crewai.md) / [中文](crewai.cn.md) |
| LangGraph | [English](langgraph.md) / [中文](langgraph.cn.md) |
| DeerFlow | [English](deerflow.md) / [中文](deerflow.cn.md) |
| Microsoft AutoGen | [English](autogen.md) / [中文](autogen.cn.md) |
| Agno | [English](agno.md) / [中文](agno.cn.md) |

## Specialized agents and references

| Project or reference | Research notes |
|---|---|
| Xiaomi MiMo Code | [English](mimo-code.md) / [中文](mimo-code.cn.md) |
| browser-use | [English](browser-use.md) / [中文](browser-use.cn.md) |
| Claude Code from Source | [English](claude-code-from-source.md) / [中文](claude-code-from-source.cn.md) |

## Common comparison dimensions

Each research file is organized around the same implementation dimensions so {{proj}} can compare designs consistently:

- source and activity evidence
- technology stack and project role
- entry points and module boundaries
- agent loop and execution model
- tools, model providers, and context construction
- state, memory, checkpointing, and persistence
- permissions, sandboxing, and security risks
- events, logging, observability, and auditability
- tests, validation strategy, and extension mechanisms
- reusable design lessons and anti-patterns
