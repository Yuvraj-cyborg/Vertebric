# Vertebric - Multi-Provider Agentic CLI

Vertebric is a 2,269-lines Rust tool that coordinates large language models to execute tasks. It supports multiple LLM providers, including Anthropic, OpenAI, Google Gemini, and any generic OpenAI-compatible API.

## Build Status: Compiles to a 4.7 MB static release binary

## Source Map

| Rust File | What It Does |
|---|---|
| `types.rs` | Message, ContentBlock, StreamEvent, Usage |
| `config.rs` | Provider config, API key resolution |
| `cli.rs` | clap CLI argument parsing |
| `cost.rs` | Multi-model pricing + session cost tracker |
| `tools/mod.rs` | Tool trait, ToolRegistry, schema gen |
| `tools/bash.rs` | Shell exec with timeout + truncation |
| `tools/file_read.rs` | File reading with line numbers |
| `tools/file_write.rs` | File creation with auto-mkdir |
| `tools/file_edit.rs` | Find-and-replace with uniqueness check |
| `tools/grep.rs` | ripgrep wrapper with grep fallback |
| `tools/glob_tool.rs` | File pattern matching |
| `tools/web_fetch.rs` | URL fetching with truncation |
| `api.rs` | Multi-provider SSE streaming |
| `engine.rs` | Agentic query loop with tool dispatch |
| `context.rs` | System prompt + AGENTS.md + git status |
| `session.rs` | JSONL transcript persistence |
| `main.rs` | CLI entry point |
| `lib.rs` | Module re-exports |

## Architecture Approach

The CLI is structured sequentially from initialization down to a recursive execution engine:

1. `main.rs` handles the immediate startup, configures the environment, and builds the baseline runtime context.
2. An async execution engine (`engine.rs`) starts the primary loop, submitting prompts to a unified `ApiClient`.
3. Standardized SSE stream processing normalizes chunk data across different AI providers, returning identical `StreamEvent` collections.
4. When tool calls are parsed by the engine, the engine matches the payload against a localized `ToolRegistry`.
5. The tool is executed on the local filesystem, producing results that are automatically appended safely back into the context buffer before restarting the turn.

## Providers

| Provider | Auth Env Var | Default Endpoint | Tool Schema Format |
|---|---|---|---|
| `claude` | `ANTHROPIC_API_KEY` | `api.anthropic.com/v1/messages` | Native (name + input_schema) |
| `openai` | `OPENAI_API_KEY` | `api.openai.com/v1/chat/completions` | Function calling |
| `gemini` | `GEMINI_API_KEY` | `generativelanguage.googleapis...` | Function calling |
| `custom` | `CUSTOM_API_KEY` | `CUSTOM_API_BASE` env | OpenAI-compatible |

## What The Project Explicitly Skips

To keep memory limits low and logic straight, Vertebric explicitly omits:
- TUI implementations
- Event telemetry tracking software
- Interactive GUI popups
- Coordinator routing (Agent Swarms)
- Auto-compacting histories (Context window truncation)
