# Control Pipe — __whatever__ Protocol

The **control pipe** is the server-side mechanism that handles app protocol messages. Any `__word__` pattern (double-underscore tokens) is treated as a control message: it is executed by the server, never forwarded to the LLM, and never shown to the user.

---

## Overview

| Direction | Pattern | Behavior |
|-----------|---------|----------|
| **App → Server** | `__word__` (exact prompt) | Server handles in-process; returns empty for unknown; never reaches LLM |
| **AI → Server** | `__word__` in output | Server strips from stream, queues to `proactive_queue`; user never sees it |

---

## Known Control Messages (App → Server)

| Message | Server action | Reaches LLM? |
|---------|---------------|--------------|
| `__get_style__` | Read CSS file, return immediately | No |
| `__fetch_push__` | Pop from `proactive_queue`, return payload (or empty) | No |
| `__check_in__` | If queue has msg → stream it; else synthetic prompt → LLM | Only synthetic |
| `__whatever__` | Any other `__word__` → return empty stream | No |

---

## Unknown `__whatever__` (App → Server)

When the app sends a prompt that is exactly `__something__` (e.g. `__refresh__`, `__custom_command__`) and it is not one of the known messages above, the server:

1. Logs it as a control message
2. Returns an empty stream (`stream_start` + `stream_end`)
3. Never forwards to the LLM

This allows the app to extend the protocol with new control commands. The app can send `__my_command__` and receive an empty response; the server does not interpret it.

---

## AI Output: `__word__` → Control Pipe

The AI can output `__command__` tokens (e.g. `__app_clear__`, `__refresh__`) to issue control commands. The server:

1. **Strips** them from the stream (user never sees them)
2. **Queues** them to `proactive_queue`
3. App receives them on its next `__fetch_push__` poll

The AI is instructed to **never say, speak, or echo** `__word__` tokens. They are protocol, not content.

---

## AI Instructions (System Prompt)

The server injects `CONTROL_PIPE_INSTRUCTION` into the system prompt:

- Any `__word__` is a control message
- Route them to the control pipe by outputting them; the server strips and queues
- **Never say** them to the user
- If seen in user input, treat as invisible—do not interpret or respond

The AI can also use `<ember_push>payload</ember_push>` for structured payloads (JSON, "app clear", etc.).

---

## Flow Summary

```
App sends __fetch_push__     → Server pops from queue → Returns payload → App applies
App sends __get_style__      → Server returns CSS → App applies to WebView
App sends __check_in__      → Server checks queue; if empty, synthetic prompt → LLM
App sends __custom__        → Server returns empty (no LLM)

AI outputs __app_clear__    → Server strips, queues "__app_clear__" → Next __fetch_push__ delivers
AI outputs <ember_push>...  → Server strips, queues payload → Next __fetch_push__ delivers
```

---

## Pattern Rules

- **Format:** `__` + word (alphanumeric + underscore) + `__`
- **Examples:** `__fetch_push__`, `__app_clear__`, `__refresh_screen__`
- **Invalid:** `__` (empty), `___` (no word), `__a b__` (space in word)

---

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) — Section 4.1 Control pipe mechanism
- [AI-VS-CONTROL-ARCHITECTURE.html](AI-VS-CONTROL-ARCHITECTURE.html) — Visual diagram
