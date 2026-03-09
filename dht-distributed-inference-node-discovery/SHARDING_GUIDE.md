# Llama Model Sharding Guide

## Overview

This guide explains how to split Llama GGUF models into statistically effective slices for distributed inference across multiple nodes.

## Sharding Strategies

### 1. Layer-wise Sharding (Pipeline Parallelism)

**Best for:** Sequential processing, minimal communication overhead

**How it works:**
- Split transformer layers across nodes
- Node 0: Embeddings + Layers 0-N
- Node 1: Layers N+1-2N
- Node N: Layers ...-M + Output Head

**Advantages:**
- Minimal communication (only activations between layers)
- Even memory distribution
- Simple to implement

**Disadvantages:**
- Sequential bottleneck (each node waits for previous)
- Lower utilization if layers have different compute times

**Statistical Effectiveness:**
- Load balance: Excellent (even layer distribution)
- Communication: Minimal (~1 activation tensor per layer boundary)
- Memory: Each node needs ~1/N of model memory
- Throughput: ~N× improvement with N nodes (theoretical)

### 2. Tensor Parallelism

**Best for:** Parallel processing within layers, high communication bandwidth

**How it works:**
- Split attention heads and FFN dimensions across nodes
- All nodes process all layers simultaneously
- Synchronize at each layer boundary

**Advantages:**
- Higher parallelism within layers
- Better for models with many attention heads

**Disadvantages:**
- High communication overhead (all-to-all at each layer)
- Requires fast inter-node communication

**Statistical Effectiveness:**
- Load balance: Good (even tensor distribution)
- Communication: High (all-to-all per layer)
- Memory: Each node needs full layer structure but split tensors
- Throughput: ~N× improvement with N nodes (requires fast network)

### 3. Hybrid Sharding

**Best for:** Large-scale distribution, optimal resource utilization

**How it works:**
- Combine layer and tensor parallelism
- Groups of nodes handle different layer ranges
- Within each group, nodes split tensors

**Advantages:**
- Best of both strategies
- Scales to many nodes efficiently

**Disadvantages:**
- Complex to implement
- Requires careful load balancing

**Statistical Effectiveness:**
- Load balance: Excellent (optimized distribution)
- Communication: Moderate (layer boundaries + tensor splits)
- Memory: Optimized per node
- Throughput: Best overall scaling

## Usage

### Creating Shards

```bash
# Layer-wise sharding (recommended for most cases)
python shard_llama_simple.py model.gguf ./shards --num-shards 4

# This creates:
# - shard_metadata.json: Sharding plan and metadata
# - SHARDING_INSTRUCTIONS.md: Usage instructions
```

### Using Shards in Distributed System

#### Option 1: Inference-time Sharding (Recommended)

Instead of physically splitting files, configure your inference system to load specific layer ranges:

```python
# Node 0: Load full model but only process layers 0-7
from llama_cpp import Llama

model = Llama(model_path="model.gguf", n_ctx=4096)
# Configure to only process layers 0-7
# (Implementation depends on llama-cpp-python API)

# Node 1: Process layers 8-15
# Node 2: Process layers 16-23
# Node 3: Process layers 24-31 + output
```

#### Option 2: Physical File Sharding

For true file splitting, you would need:
1. GGUF parser to extract tensor data
2. Create new GGUF files with subset of tensors
3. Ensure proper metadata in each shard

**Note:** Physical sharding is complex and may not be necessary if your inference system supports layer range configuration.

## Statistical Effectiveness Metrics

### Layer-wise Sharding

For a 32-layer model split across 4 nodes:

```
Node 0: Layers 0-7   (8 layers)  - Embeddings included
Node 1: Layers 8-15 (8 layers)
Node 2: Layers 16-23 (8 layers)
Node 3: Layers 24-31 (8 layers)  - Output head included
```

**Memory per node:**
- Model weights: ~1.25GB (4.92GB / 4)
- Activations: ~batch_size × seq_len × 4096 × 4 bytes

**Communication:**
- Between nodes: 1 activation tensor per layer boundary
- Size: batch_size × seq_len × hidden_dim × 4 bytes
- Frequency: Once per layer group

**Throughput:**
- Theoretical: 4× speedup (if communication is fast)
- Practical: 3-3.5× speedup (accounting for communication overhead)

### Optimal Shard Count

For best statistical effectiveness:

1. **Small models (7B-8B):** 2-4 shards
2. **Medium models (13B-30B):** 4-8 shards
3. **Large models (65B+):** 8-16 shards

**Rule of thumb:** Number of shards should be ≤ number of layers / 4

## Integration with Distributed System

### Work Distribution Integration

The sharding system integrates with `work_distribution.py`:

```python
from work_distribution import WorkDistributionManager, NodeCapability

# Register nodes with their shard assignments
work_dist.register_node("node-0", NodeInfo(
    ip="192.168.1.10",
    port=7001,
    capabilities=[NodeCapability.AI_PROCESSING],
    weight=1.0
))
# Node 0 handles layers 0-7

work_dist.register_node("node-1", NodeInfo(
    ip="192.168.1.11",
    port=7001,
    capabilities=[NodeCapability.AI_PROCESSING],
    weight=1.0
))
# Node 1 handles layers 8-15
```

### Pipeline Execution

```python
# Client sends request to Node 0
# Node 0 processes embeddings + layers 0-7
# Node 0 sends activations to Node 1
# Node 1 processes layers 8-15
# ... continues through pipeline
# Last node processes final layers + output head
# Response sent back through pipeline
```

## Best Practices

1. **Start with layer-wise sharding** - Simplest and most effective
2. **Match shard count to available nodes** - Don't over-shard
3. **Ensure fast inter-node communication** - Use low-latency network
4. **Monitor load balance** - Adjust if layers have uneven compute
5. **Use hybrid for large scale** - Only if you have 8+ nodes

## Troubleshooting

**Issue:** Uneven load across nodes
- **Solution:** Check layer compute times, may need to rebalance layer assignments

**Issue:** High communication overhead
- **Solution:** Reduce number of shards, or use tensor parallelism for fast networks

**Issue:** Memory issues
- **Solution:** Reduce batch size or sequence length, or increase number of shards

