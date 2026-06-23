# Agent 实现研究索引

本目录整理领先开源 agent 实现与 agent 架构资料的源码级研究笔记。每个条目都有英文文档和同步的中文版本。

## Coding agents

| 项目 | 研究笔记 |
|---|---|
| OpenHands | [English](openhands.md) / [中文](openhands.cn.md) |
| Cline | [English](cline.md) / [中文](cline.cn.md) |
| Aider | [English](aider.md) / [中文](aider.cn.md) |
| OpenCode | [English](opencode.md) / [中文](opencode.cn.md) |
| Goose | [English](goose.md) / [中文](goose.cn.md) |
| Continue | [English](continue.md) / [中文](continue.cn.md) |

## Agent frameworks and platforms

| 项目 | 研究笔记 |
|---|---|
| AutoGPT | [English](autogpt.md) / [中文](autogpt.cn.md) |
| CrewAI | [English](crewai.md) / [中文](crewai.cn.md) |
| LangGraph | [English](langgraph.md) / [中文](langgraph.cn.md) |
| DeerFlow | [English](deerflow.md) / [中文](deerflow.cn.md) |
| Microsoft AutoGen | [English](autogen.md) / [中文](autogen.cn.md) |
| Agno | [English](agno.md) / [中文](agno.cn.md) |

## Specialized agents and references

| 项目或资料 | 研究笔记 |
|---|---|
| Xiaomi MiMo Code | [English](mimo-code.md) / [中文](mimo-code.cn.md) |
| browser-use | [English](browser-use.md) / [中文](browser-use.cn.md) |
| Claude Code from Source | [English](claude-code-from-source.md) / [中文](claude-code-from-source.cn.md) |

## 统一比较维度

每份研究文档都围绕同一组实现维度整理，便于 {{proj}} 横向比较设计：

- 来源与近期活跃证据
- 技术栈与项目定位
- 入口与模块边界
- agent loop 与执行模型
- 工具、模型 provider 与上下文构建
- 状态、记忆、checkpoint 与持久化
- 权限、沙箱与安全风险
- 事件、日志、可观测性与审计
- 测试、验证策略与扩展机制
- 可复用设计经验与反模式
