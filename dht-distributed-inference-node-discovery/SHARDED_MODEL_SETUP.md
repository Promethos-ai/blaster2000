# Sharded Model Setup Guide

## Overview

The AI processor now supports **sharded models** using **pipeline parallelism**. This allows distributing a large Llama model across multiple nodes, with each node processing a subset of transformer layers.

## How Sharding Works

### Pipeline Parallelism

```
Query → Node 0 (Layers 0-7) → Node 1 (Layers 8-15) → Node 2 (Layers 16-23) → Node 3 (Layers 24-31 + Output) → Answer
```

- **Node 0**: Processes embeddings + layers 0-7, forwards activations to Node 1
- **Node 1**: Receives activations, processes layers 8-15, forwards to Node 2
- **Node 2**: Receives activations, processes layers 16-23, forwards to Node 3
- **Node 3**: Receives activations, processes layers 24-31 + output head, returns final answer

### Benefits

1. **Memory Distribution**: Each node only needs memory for ~1/N of the model
2. **Scalability**: Can split large models across multiple servers
3. **Load Balancing**: Work is distributed evenly across nodes

## Setup Instructions

### Step 1: Create Shard Metadata

First, create shard metadata using the sharding tool:

```bash
python shard_llama_simple.py model.gguf ./shards --num-shards 4
```

This creates:
- `shard_metadata.json`: Sharding plan
- `SHARDING_INSTRUCTIONS.md`: Usage instructions

### Step 2: Configure Each Node

Each node needs to know its shard configuration. Set environment variables or configure in code:

#### Node 0 (First shard - includes embeddings)
```bash
export LLAMA_SHARD_ID=0
export LLAMA_LAYER_START=0
export LLAMA_LAYER_END=8
export LLAMA_NEXT_SHARD_IP=192.168.1.11
export LLAMA_NEXT_SHARD_PORT=7001
```

#### Node 1 (Middle shard)
```bash
export LLAMA_SHARD_ID=1
export LLAMA_LAYER_START=8
export LLAMA_LAYER_END=16
export LLAMA_NEXT_SHARD_IP=192.168.1.12
export LLAMA_NEXT_SHARD_PORT=7001
```

#### Node 2 (Middle shard)
```bash
export LLAMA_SHARD_ID=2
export LLAMA_LAYER_START=16
export LLAMA_LAYER_END=24
export LLAMA_NEXT_SHARD_IP=192.168.1.13
export LLAMA_NEXT_SHARD_PORT=7001
```

#### Node 3 (Last shard - includes output head)
```bash
export LLAMA_SHARD_ID=3
export LLAMA_LAYER_START=24
export LLAMA_LAYER_END=32
# No next shard - this is the last one
```

### Step 3: Place Shard Metadata

Place `shard_metadata.json` in the working directory of each node, or specify path:

```python
from ai_processor import AiProcessingConfig, AiProcessor

config = AiProcessingConfig(
    shard_id=0,
    layer_range=(0, 8),
    includes_embeddings=True,
    includes_output=False,
    next_shard_node=("192.168.1.11", 7001),
    shard_metadata_path="./shards/shard_metadata.json"
)

processor = AiProcessor(config)
```

### Step 4: Start Servers

Start each node with its shard configuration:

```bash
# Node 0
LLAMA_SHARD_ID=0 LLAMA_LAYER_START=0 LLAMA_LAYER_END=8 \
  LLAMA_NEXT_SHARD_IP=192.168.1.11 LLAMA_NEXT_SHARD_PORT=7001 \
  python3 quic_tracker_server.py 7001

# Node 1
LLAMA_SHARD_ID=1 LLAMA_LAYER_START=8 LLAMA_LAYER_END=16 \
  LLAMA_NEXT_SHARD_IP=192.168.1.12 LLAMA_NEXT_SHARD_PORT=7001 \
  python3 quic_tracker_server.py 7001

# Node 2
LLAMA_SHARD_ID=2 LLAMA_LAYER_START=16 LLAMA_LAYER_END=24 \
  LLAMA_NEXT_SHARD_IP=192.168.1.13 LLAMA_NEXT_SHARD_PORT=7001 \
  python3 quic_tracker_server.py 7001

# Node 3 (last)
LLAMA_SHARD_ID=3 LLAMA_LAYER_START=24 LLAMA_LAYER_END=32 \
  python3 quic_tracker_server.py 7001
```

## How It Works

### Processing Flow

1. **Client sends query** to Node 0
2. **Node 0** processes:
   - Tokenization (text → token IDs)
   - Embeddings (token IDs → vectors)
   - Layers 0-7 (transformer processing)
3. **Node 0 forwards activations** to Node 1
4. **Node 1** processes layers 8-15, forwards to Node 2
5. **Node 2** processes layers 16-23, forwards to Node 3
6. **Node 3** processes layers 24-31 + output head, decodes tokens
7. **Response flows back** through the pipeline to the client

### Communication

- Uses QUIC for inter-node communication
- Activations are sent as tensors between nodes (currently stubbed)
- Pipeline requests are flagged with `pipeline_request: True`

## Configuration Options

### AiProcessingConfig

```python
@dataclass
class AiProcessingConfig:
    # Model configuration
    model_name: str = "llama-2-7b-chat"
    model_path: Optional[str] = None
    temperature: float = 0.7
    max_tokens: int = 512
    
    # Sharding configuration
    shard_id: Optional[int] = None  # This node's shard ID
    layer_range: Optional[Tuple[int, int]] = None  # (start, end) layers
    includes_embeddings: bool = True  # Shard 0 only
    includes_output: bool = False  # Last shard only
    next_shard_node: Optional[Tuple[str, int]] = None  # (ip, port) of next shard
    shard_metadata_path: Optional[str] = None  # Path to shard_metadata.json
```

## Environment Variables

The AI processor automatically loads shard configuration from environment:

- `LLAMA_SHARD_ID`: Shard identifier (0, 1, 2, ...)
- `LLAMA_LAYER_START`: First layer in this shard
- `LLAMA_LAYER_END`: Last layer (exclusive) in this shard
- `LLAMA_NEXT_SHARD_IP`: IP of next shard in pipeline
- `LLAMA_NEXT_SHARD_PORT`: Port of next shard

## Example: 4-Node Sharding

For a 32-layer model across 4 nodes:

```
Node 0 (192.168.1.10): Layers 0-7   + Embeddings
  ↓ (forwards activations)
Node 1 (192.168.1.11): Layers 8-15
  ↓ (forwards activations)
Node 2 (192.168.1.12): Layers 16-23
  ↓ (forwards activations)
Node 3 (192.168.1.13): Layers 24-31 + Output Head
  ↓ (returns answer)
Client
```

## Testing

1. Start all nodes with proper shard configuration
2. Send AI query to Node 0
3. Check logs to see pipeline execution:
   - `[SHARDED_PIPELINE]` logs show shard processing
   - `[SHARDED_PIPELINE] Forwarding activations` shows pipeline forwarding
   - Response includes `sharded: True` in metadata

## Notes

- **Current Implementation**: Stub-based (simulates processing)
- **Future**: Will load actual GGUF shards and process real activations
- **Communication**: Currently sends query text; future will send tensor data
- **Error Handling**: Pipeline failures return error responses with proper structure

## Troubleshooting

**Issue**: Pipeline forward fails
- **Check**: Next shard node is running and accessible
- **Check**: Network connectivity between nodes
- **Check**: Ports are open in firewall

**Issue**: Wrong layers processed
- **Check**: `layer_range` matches shard metadata
- **Check**: Environment variables are set correctly

**Issue**: Embeddings/output not included
- **Check**: `includes_embeddings=True` for shard 0
- **Check**: `includes_output=True` for last shard



