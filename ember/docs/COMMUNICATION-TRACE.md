# Communication Trace: Android App → Pinggy → gRPC → Inference → Back

Full trace of a query from the Android app through pinggy, ember-server, grpc_server, and the inference engine.

---

## Overview Diagram

```
┌─────────────┐     QUIC/UDP      ┌─────────────┐     UDP      ┌──────────────┐     gRPC/QUIC  ┌─────────────┐     in-process   ┌─────────────────┐
│ Android App │ ────────────────► │   Pinggy    │ ───────────► │ ember-server │ ──────────────► │ grpc_server │ ───────────────► │ InferenceEngine │
│ (ember-     │  xxx.pinggy.link  │ (reverse    │  127.0.0.1   │ (QUIC bridge) │  127.0.0.1     │ (Feb17)     │  llama.cpp      │ (llama.cpp)     │
│  client)    │  :port            │  proxy)     │  :4433       │ UDP 4433     │  :50051        │ QUIC 50051  │  generate_stream│                 │
└─────────────┘                   └─────────────┘              └──────────────┘                └─────────────┘                  └─────────────────┘
       ▲                                    │                           │                              │                                │
       │                                    │                           │                              │                                │
       └────────────────────────────────────┴───────────────────────────┴──────────────────────────────┴────────────────────────────────┘
                                                    Response flows back (reverse path)
```

---

## 1. Android App (Request)

**Location:** `ember/client/src/lib.rs`, `ember/client/src/jni.rs`

| Step | Action | Code |
|------|--------|------|
| 1.1 | User enters prompt, taps Ask | `MainActivity.askAi(addr, prompt)` |
| 1.2 | JNI calls Rust | `EmberClient.askStreaming(addr, prompt, callback)` |
| 1.3 | Resolve hostname | `addr_str.to_socket_addrs()` → e.g. `jkazjsnynw.a.pinggy.link:10822` |
| 1.4 | Extract host for TLS SNI | `extract_host(addr_str)` → `jkazjsnynw.a.pinggy.link` |
| 1.5 | QUIC connect | `endpoint.connect(server_addr, server_name)` |
| 1.6 | Open bidirectional stream | `connection.open_bi()` |
| 1.7 | Send prompt (raw UTF-8) | `send.write_all(prompt.as_bytes())` |
| 1.8 | Finish send stream | `send.finish()` |

**Payload:** Raw UTF-8 bytes of the prompt (e.g. `"What is 2+2?"`).

---

## 2. Pinggy (Reverse Proxy)

**Location:** `pinggy.exe` (external binary)

| Step | Action | Notes |
|------|--------|-------|
| 2.1 | Receives QUIC/UDP packets | From phone at `xxx.a.pinggy.link:port` |
| 2.2 | Forwards to local | `-R0:127.0.0.1:4433` → sends to `127.0.0.1:4433` |
| 2.3 | Receives response UDP | From ember-server |
| 2.4 | Forwards back to phone | Via pinggy cloud to client |

**Protocol:** UDP tunnel. Pinggy transparently forwards UDP datagrams. QUIC runs over UDP, so the entire QUIC connection is tunneled.

---

## 3. ember-server (QUIC Bridge)

**Location:** `ember/server/src/main.rs`

| Step | Action | Code | Log |
|------|--------|------|-----|
| 3.1 | Accept QUIC connection | `endpoint.accept()` | `[CONN] client connected from 127.0.0.1:xxxxx` |
| 3.2 | Accept bidirectional stream | `connection.accept_bi()` | |
| 3.3 | Read prompt bytes | `recv.read_to_end()` | `[RECV] N bytes from app` |
| 3.4 | Parse prompt string | `String::from_utf8_lossy()` | `[RECV] prompt (N chars): ...` |
| 3.5 | Connect to gRPC (lazy) | `https://127.0.0.1:50051` = QUIC, `http://` = TCP fallback | |
| 3.6 | Build gRPC request | `CompleteRequest { prompt: ChatPrompt { prompt } }` | |
| 3.7 | Call complete_stream | `client.complete_stream(Request::new(...))` | |
| 3.8 | Send stream_start | `{"type":"stream_start","interaction_id":N}` | |
| 3.9 | For each token from gRPC | `stream.next()` | |
| 3.10 | Send stream_token | `{"type":"stream_token","token":"..."}` | |
| 3.11 | Send stream_end | `{"type":"stream_end"}` | `[SEND] response stream (N chars)` |
| 3.12 | Finish send stream | `send.finish()` | |

**Request format:** Raw UTF-8 prompt bytes.  
**Response format:** Newline-delimited JSON: `stream_start`, `stream_token`, `stream_end`, or `stream_error`.

---

## 4. grpc_server (Feb17)

**Location:** `Feb17/src/bin/grpc_server.rs`  
**Start with:** `cargo run --bin grpc_server -p Feb17 -- --quic --host localhost` (QUIC) or omit `--quic` for TCP.

| Step | Action | Code | Log |
|------|--------|------|-----|
| 4.1 | Receive gRPC CompleteStreamRequest | `complete_stream(request)` | `[GRPC] CompleteStream request from 127.0.0.1:xxxxx` |
| 4.2 | Extract prompt | `req.prompt.and_then(|p| p.prompt)` | `[RECV] prompt (N chars): ...` |
| 4.3 | Spawn blocking inference | `engine.generate_stream()` | |
| 4.4 | For each token | `tx.send(Ok(CompleteStreamReply { token }))` | |
| 4.5 | Stream completes | `drop(tx)` | |

**gRPC:** `complete_stream` RPC, streaming `CompleteStreamReply { token: String }`.

---

## 5. Inference Engine (llama.cpp)

**Location:** `Feb17` inference engine via llama.cpp

| Step | Action |
|------|--------|
| 5.1 | `generate_stream(prompt, max_tokens, params, callback)` |
| 5.2 | Token-by-token generation via callback |
| 5.3 | Each token → `CompleteStreamReply { token }` |

---

## 6. Response Path (Back to Android)

| Hop | From | To | Content |
|-----|------|-----|---------|
| 6.1 | InferenceEngine | grpc_server | `CompleteStreamReply { token }` stream |
| 6.2 | grpc_server | ember-server | gRPC streaming response |
| 6.3 | ember-server | QUIC | `{"type":"stream_token","token":"..."}\n` |
| 6.4 | ember-server | pinggy | UDP over 127.0.0.1:4433 |
| 6.5 | pinggy | Android | UDP via pinggy cloud |
| 6.6 | Android | UI | `parse_stream_response` → `on_token` → `ChatAdapter.updateLastAiMessage` |

---

## Log Points

| Component | Log | When |
|------------|-----|------|
| `ember-server` | `ember-connections.log` | CONNECT, REQUEST, RESPONSE, ERROR |
| `ember-server` | stdout | `[CONN]`, `[RECV]`, `[SEND]` |
| `grpc_server` | stdout / `--log-file` | `[GRPC]`, `[RECV]`, `[SEND]` |
| pinggy | Pinggy window | Request/Response bytes, Live Connections |

---

## Debugging

1. **No requests in grpc_server:** Check ember-server is running and pinggy is forwarding to 4433.
2. **Response 0 in pinggy:** Check grpc_server logs for `[SEND]`; if present, issue is ember→pinggy or pinggy→phone.
3. **Timeout on Android:** Increase QUIC timeout or check network/firewall.
