# Ember Protocol & Event System

Documentation and diagrams for the ember server/client protocol and event system.

## Contents

| Document | Description |
|----------|-------------|
| [architecture.html](architecture.html) | Full protocol architecture: Android app ↔ ember-server ↔ grpc_server, QUIC flow, ports |
| [event-system.html](event-system.html) | Push event system: proactive_queue, __fetch_push__ long-poll, TCP push channel, file watcher |
| [stream-frames.html](stream-frames.html) | Stream frame types: stream_token, stream_rich, stream_control_payload, etc. |
| [control-pipe.html](control-pipe.html) | Control pipe: __word__ messages, control vs. user prompts, AI → app commands |

## Quick Reference

### Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 4433 | QUIC/UDP | App ↔ ember-server (prompts, streaming responses) |
| 4434 | TCP | Push channel (external processes → proactive_queue) |
| 50051 | gRPC | ember-server ↔ grpc_server (LLM inference) |

### Control Messages (App → Server)

| Message | Action |
|---------|--------|
| `__get_style__` | Return CSS from server |
| `__fetch_push__` | Long-poll: pop from queue, return when push arrives (or 60s timeout) |
| `__check_in__` | If queue has msg → stream it; else synthetic prompt → LLM |

### Stream Frame Types (Server → App)

| Type | Payload | Client action |
|------|---------|---------------|
| `stream_token` | Chat text | Append to last AI message |
| `stream_rich` | HTML | Update rich content area |
| `stream_style` | CSS | Apply to rich area |
| `stream_layout` | JSON | rich_height, inference_timeout_sec |
| `stream_audio` | Text | TTS speak |
| `stream_control_payload` | Payload | applyPushPayload (e.g. "app clear") |
| `stream_start` | — | Ignored |
| `stream_end` | — | Finish |
| `stream_error` | error | Show error, stop |
