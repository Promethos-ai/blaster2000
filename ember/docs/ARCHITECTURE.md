# Ember — Full Architecture & Functionality

A detailed explanation of the Ember project: a QUIC-based mobile AI chat client that connects to a local inference server via an optional reverse proxy (e.g. eagleoneonline.ca).

---

## 1. High-Level Overview

Ember lets you run an AI assistant on your home PC and access it from your smartphone over the internet. The phone app sends questions over QUIC (UDP), the ember-server bridges to a local gRPC inference service (Feb17), and responses stream back token-by-token.

![Ember System Overview](images/ember-system-overview.png)

---

## 2. End-to-End Request Flow

![Request Flow Sequence](images/ember-request-flow.png)

---

## 3. Network Architecture

**Scenario A (Local):** Phone on same WiFi → PC (ember-server :4433, Feb17 :50051)  
**Scenario B (Remote):** Phone → eagleoneonline.ca:4433 (reverse proxy) → PC

![Network Topology](images/ember-network-topology.png)

---

## 4. Component Diagram

![Component Responsibilities](images/ember-components.png)

| Layer | Components |
|-------|------------|
| **Android** | MainActivity, ChatAdapter, EmberClient, TokenCallback, SplashActivity |
| **Ember Client** | lib.rs (QUIC), jni.rs (JNI), parse_stream_response |
| **Ember Server** | main.rs, stream_inference, call_inference_stream |
| **Feb17** | grpc_server, complete_stream RPC |

---

## 5. Streaming Protocol (JSON Frames)

Frames are newline-delimited JSON objects (`\n`-terminated). Server → client.

![Streaming Protocol](images/ember-streaming-protocol.png)

**Frame types:** `stream_start`, `stream_token`, `stream_end`, `stream_error`  
**Client logic:** Read chunks, split on newline, parse JSON; `stream_token` → `on_token()`; `stream_end` → return; legacy: non-JSON lines treated as raw text.

---

## 6. Data Flow (Simplified)

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                    DATA FLOW                                                      │
└─────────────────────────────────────────────────────────────────────────────────┘

  REQUEST (Phone → Server):
  ─────────────────────────
  • User types prompt in EditText
  • MainActivity calls EmberClient.askStreaming(addr, prompt, callback)
  • JNI spawns thread; Rust connects QUIC, sends prompt as raw UTF-8 bytes
  • send.write_all(prompt.as_bytes()); send.finish()


  RESPONSE (Server → Phone):
  ─────────────────────────
  • ember-server receives prompt, calls Feb17 complete_stream
  • Feb17 streams tokens; ember-server wraps each in stream_token JSON
  • Client parse_stream_response reads chunks, parses JSON, calls on_token
  • JNI thread receives tokens via channel, calls callback.onToken(token)
  • MainActivity updates ChatAdapter; UI shows token progressively
```

---

## 7. File Layout

```
ember/
├── Cargo.toml                 # Workspace: server, client
├── client/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # QUIC client, streaming
│       ├── main.rs            # Desktop CLI
│       ├── jni.rs             # Android JNI
│       └── ios.rs             # iOS bindings
├── server/
│   ├── Cargo.toml
│   ├── build.rs               # tonic-prost proto compile
│   ├── proto/llm.proto        # de.kherud.grpc.llm
│   └── src/main.rs            # QUIC bridge
├── android/
│   └── app/src/main/
│       ├── java/com/ember/android/
│       │   ├── MainActivity.kt
│       │   ├── EmberClient.kt
│       │   ├── ChatAdapter.kt
│       │   └── SplashActivity.kt
│       ├── jniLibs/           # libember_native.so (arm64, armv7)
│       └── res/values/
│           ├── server_defaults.xml   # Default server URL
│           └── strings.xml
├── build-android.ps1          # cargo ndk + gradle assembleRelease
├── release-android.ps1        # gh release create + APK upload
└── docs/
    ├── ARCHITECTURE.md        # This file
    └── DEVELOPMENT.md
```

---

## 8. Protocols & Ports

| Protocol | Port | Direction | Purpose |
|----------|------|-----------|---------|
| QUIC (UDP) | 4433 | Phone → ember-server | User prompts, streaming responses |
| gRPC (TCP) | 50051 | ember-server → Feb17 | LLM inference |

---

## 9. Configuration Summary

| Item | Location | Default |
|------|----------|---------|
| ember-server listen | `server/src/main.rs` | `0.0.0.0:4433` |
| Feb17 gRPC | `--inference` | `http://127.0.0.1:50051` |
| Android default server | `server_defaults.xml` | `eagleoneonline.ca:4433` |
| Connection log | `--log-file` | `ember-connections.log` |

---

## 10. Startup Order

1. **Feb17 grpc_server** (TCP 50051).
2. **ember-server** (UDP 4433).
3. **eagleoneonline.ca** reverse proxy (optional, for remote access).
4. **Android app**: Connect using `eagleoneonline.ca:4433` or local IP.

---

## 11. Mermaid Diagrams

### System Context

```mermaid
flowchart TB
    subgraph Phone ["📱 Phone"]
        App[App UI]
    end
    
    subgraph PC ["🖥️ PC"]
        Ember[ember-server]
        Feb17[Feb17 grpc_server]
    end
    
    subgraph Proxy ["☁️ Optional"]
        Eagle[eagleoneonline.ca]
    end
    
    App -->|QUIC| Eagle
    Eagle -->|forward| Ember
    App -->|QUIC| Ember
    Ember -->|gRPC| Feb17
```

### Request Flow

```mermaid
sequenceDiagram
    participant U as User
    participant A as Android
    participant R as Rust Client
    participant S as ember-server
    participant F as Feb17

    U->>A: Tap Ask
    A->>R: askStreaming(addr, prompt, callback)
    R->>S: QUIC connect + send prompt
    S->>F: complete_stream(prompt)
    loop Tokens
        F->>S: token
        S->>R: stream_token JSON
        R->>A: onToken(token)
        A->>U: Update UI
    end
    F->>S: stream end
    S->>R: stream_end JSON
    R->>A: return full result
```

### Component Dependencies

```mermaid
flowchart LR
    Main[MainActivity]
    Adapter[ChatAdapter]
    Ember[EmberClient]
    JNI[jni.rs]
    Lib[lib.rs]
    Server[ember-server]
    Feb17[Feb17]

    Main --> Adapter
    Main --> Ember
    Ember --> JNI
    JNI --> Lib
    Lib --> Server
    Server --> Feb17
```
