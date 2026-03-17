# Feb17 RAG / Cognitive Memory – All Parameters

Reference for all changeable parameters in the Feb17 Promethos cognitive memory system and related components.

---

## 1. Promethos Config (`config/promethos.toml`)

Primary config file for the RAG/cognitive memory system. Loaded by `PromethosConfig::load()`.

### 1.1 Paths
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `paths.data_dir` | `"data"` | — | SQLite DB and memory data directory |
| `paths.models_dir` | `"models"` | — | Model files directory |
| `paths.cache_dir` | `"cache"` | — | Cache directory |

### 1.2 Embedding
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `embedding.embedding_dim` | `384` | ≥ 1 | Embedding vector dimension |
| `embedding.model_name` | `"sentence-transformers/all-MiniLM-L6-v2"` | — | Embedding model (used when `semantic` feature enabled) |

### 1.3 Retrieval (RAG)
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `retrieval.top_k` | `5` | 1–50 | Max summaries to retrieve by similarity |
| `retrieval.similarity_threshold` | `0.7` | 0.0–1.0 | Min cosine similarity to include |
| `retrieval.max_summary_tokens` | `800` | ≥ 100 | Max tokens for retrieved summaries block |

### 1.4 Cold Pass (async analysis)
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `cold_pass.summary_max_chars` | `100` | 10–500 | Max chars per summary in cold pass output |
| `cold_pass.importance_default` | `0.5` | 0.0–1.0 | Default importance when stub cold pass used |

### 1.5 Cold Queue
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `cold_queue.max_backlog` | `200` | ≥ 10 | Max pending cold jobs; oldest dropped when full |
| `cold_queue.retry_count` | `3` | ≤ 10 | Retries per job before discard |

### 1.6 Injection (personality + context)
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `injection.max_total_personality_block_tokens` | `200` | ≥ 50 | Max tokens for personality bias block |
| `injection.min_injection_threshold` | `0.2` | 0.0–1.0 | Min effective weight to inject a trait |
| `injection.max_traits_per_turn` | `6` | 1–20 | Max traits injected per turn |
| `injection.trait_influence_multiplier` | `0.25` | ≥ 0 | Weight of trait in effective weight |
| `injection.modifier_influence_multiplier` | `0.5` | ≥ 0 | Weight of modifier in effective weight |

### 1.7 Weight Engine
| Parameter | Default | Valid Range | Effect |
|-----------|---------|-------------|--------|
| `weight_engine.max_modifier` | `0.35` | 0.0–1.0 | Max modifier value |
| `weight_engine.base_step` | `0.02` | ≥ 0 | Base step for reinforcement updates |
| `weight_engine.trait_promotion_threshold` | `0.15` | 0.0–1.0 | Reinforcement threshold to promote to base |
| `weight_engine.confidence_promotion_min` | `0.70` | 0.0–1.0 | Min confidence to promote trait |
| `weight_engine.persistent_decay_per_day` | `0.001` | ≥ 0 | Daily decay for persistent traits |
| `weight_engine.modifier_decay_per_day` | `0.25` | ≥ 0 | Daily decay for modifiers |
| `weight_engine.reinforcement_clamp_min` | `-0.25` | — | Min reinforcement |
| `weight_engine.reinforcement_clamp_max` | `0.25` | — | Max reinforcement |

---

## 2. Memory Plugin (`src/plugins/memory.rs`)

Used by `http_server` for vector-based memory. Not used by `grpc_server` directly.

| Parameter | Default | Effect |
|-----------|---------|--------|
| `retrieve_top_k` | `5` | Past exchanges to retrieve by similarity |
| `inject_max_chars` | `2000` | Max chars of past context to inject |
| `max_entries` | `500` | Max DB entries; oldest pruned when exceeded |
| `RECENT_ALWAYS_INCLUDE` | `2` | Always-included recent exchanges (hardcoded) |

Builder API: `.retrieve_top_k(k)`, `.inject_max_chars(n)`, `.max_entries(n)`.

---

## 3. HTTP Server Web Config (`http_server`)

Exposed via web UI and config API. Includes Promethos and inference params.

### 3.1 Memory (plugin)
| Parameter | Default | Effect |
|-----------|---------|--------|
| `memory_enabled` | `true` | Enable memory plugin |
| `retrieve_top_k` | `5` | Past exchanges to retrieve |
| `inject_max_chars` | `2000` | Max context chars to inject |
| `max_entries` | `500` | Max memory entries |

### 3.2 Inference
| Parameter | Default | Effect |
|-----------|---------|--------|
| `inference_temperature` | `0.45` | Sampling temperature |
| `inference_max_tokens` | `512` | Max output tokens |
| `inference_top_k` | `40` | Top-K sampling |
| `inference_top_p` | `0.9` | Nucleus sampling |
| `inference_repeat_penalty` | `1.15` | Repetition penalty |
| `inference_penalty_last_n` | `64` | Context for repetition penalty |

### 3.3 Promethos (in WebConfig)
Same structure as `PromethosConfigSection`; all Promethos params can be set via web config.

---

## 4. Control Plane (`config/control_planes.toml`)

| Parameter | Default | Effect |
|-----------|---------|--------|
| `default_plane` | `"main"` | Default control plane |
| `control_planes.server.port` | `1236` | Control plane server port |
| `control_planes.providers[].id` | — | Provider ID (e.g. feb17, lmstudio) |
| `control_planes.providers[].endpoint` | — | Provider HTTP endpoint |
| `control_planes.task_types[].task_type` | — | Task type name |
| `control_planes.task_types[].max_tokens` | — | Max tokens for that task |
| `control_planes.task_types[].temperature` | — | Temperature for that task |
| `control_planes.policy.tenant_id_header` | `"X-Tenant-ID"` | Tenant header |
| `control_planes.policy.rate_limit_requests_per_minute` | `60` | Rate limit |
| `control_planes.orchestration.routing_strategy` | `"round_robin"` | Routing strategy |
| `control_planes.orchestration.canary_percent` | `0.0` | Canary traffic % |
| `control_planes.observability.audit_retention_days` | `90` | Audit retention |

---

## 5. grpc_server CLI

| Flag | Default | Effect |
|------|--------|--------|
| `--model` / `-m` | OS-specific default | GGUF model path |
| `--port` / `-p` | `50051` | gRPC listen port |
| `--quic` | off | Use QUIC (HTTP/3) instead of TCP |
| `--host` | `eagleoneonline.ca` | TLS hostname (when QUIC) |
| `--log-file` | — | Append logs to file |
| `--load-estimate` | `45` | Progress bar estimate (seconds) |
| `--announce` / `--no-announce` | `--announce` | Toast/beep when model loaded |

Inference params come from the gRPC request (e.g. from ember `inference_params.json`).

---

## 6. Control Config (`control.toml`)

| Parameter | Default | Effect |
|-----------|---------|--------|
| `server.port` | `1236` | Control plane port |
| `providers[].id` | — | Provider ID |
| `providers[].endpoint` | — | Provider URL |

---

## 7. Environment Variables

| Variable | Effect |
|----------|--------|
| `FEB17_MEMORY_VERBOSE` | Set to `1` to log memory operations |
| `BRAVE_API_KEY` | (Ember) Brave Search API key |

---

## 8. Cargo Features

| Feature | Effect |
|---------|--------|
| `semantic` | Use fastembed for embeddings; else hash-based fallback |
| `cuda` | Use CUDA for inference (default) |

---

## 9. Injection Order (prompt assembly)

Order of blocks in the final prompt:

1. `[System Baseline]`
2. `[Control Plane Constraint Block]`
3. `[Personality Bias Block]` (from traits)
4. `[Short-Term State Block]`
5. `[Retrieved Summaries Block]` (RAG)
6. User query

---

## 10. Cold Pass Output (JSON contract)

| Field | Type | Constraints |
|-------|------|-------------|
| `summary` | string | Required, non-empty |
| `importance` | number | 0.0–1.0 |
| `tags` | array of strings | Optional |
| `traits` | array | Each: `trait_key`, `signal_kind` (positive/negative/neutral), `strength` (0–1), `reason?` |

---

## 11. Influence Bands (personality injection)

| Band | Weight Range | Rendered As |
|------|--------------|-------------|
| Weak | 0.00–0.25 | "slight preference" |
| Moderate | 0.26–0.50 | "moderate bias" |
| Strong | 0.51–0.75 | "strong bias" |
| Dominant | 0.76–1.00 | "dominant bias" |

Effective weight: `clamp((base + reinforcement) * (1 + modifier), 0, 1)`.
