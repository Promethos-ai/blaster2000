# Distributed Inference Deployment Plan

## Overview

This plan describes how to deploy a distributed Llama model inference system using pipeline parallelism across multiple nodes.

## Architecture

```
Client → Node 0 (Embeddings + Layers 0-7) → Node 1 (Layers 8-15) → Node 2 (Layers 16-23) → Node 3 (Layers 24-31 + Output) → Response
```

Each node:
- Processes a subset of transformer layers
- Communicates with adjacent nodes via QUIC
- Loads only its assigned layer range (saves memory)

## Prerequisites

1. **Model Shards**: Created using `shard_models.ps1`
2. **Rsync Server**: Configured with credentials
3. **Nodes**: Multiple machines (or containers) ready for deployment
4. **Server Code**: `quic_tracker_server_ai.py` on each node

## Quick Start

### Step 1: Create Shards

```powershell
.\shard_models.ps1 -ModelPath "E:\rust\llamaModels\Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf" -NumShards 4
```

### Step 2: Upload Shards

```powershell
.\upload_shards.ps1 -ShardsDir ".\shards" -UploadModel
```

### Step 3: Generate Node Configurations

```powershell
.\generate_node_configs.ps1 -ShardsDir ".\shards" -ServerHost "162.221.207.169" -ServerPort 7001
```

### Step 4: Deploy Nodes

Copy configuration files to each node and start servers.

## Detailed Deployment

### Phase 1: Preparation

1. **Create Shards**
   - Run `shard_models.ps1` to generate shard metadata
   - Review `shards/shard_metadata.json`

2. **Upload to Rsync**
   - Upload metadata and model: `upload_shards.ps1 -UploadModel`
   - Verify files on rsync server

3. **Generate Configs**
   - Run `generate_node_configs.ps1`
   - Review generated `node_configs/` directory

### Phase 2: Node Deployment

For each node:

1. **Copy Files**
   ```bash
   # On target node
   mkdir -p /opt/llama-distributed
   # Copy from deployment machine:
   scp node_configs/node-XX.* user@node-XX:/opt/llama-distributed/
   scp quic_tracker_server_ai.py user@node-XX:/opt/llama-distributed/
   scp ai_processor.py user@node-XX:/opt/llama-distributed/
   scp shard_rsync_manager.py user@node-XX:/opt/llama-distributed/
   ```

2. **Install Dependencies**
   ```bash
   pip3 install aioquic llama-cpp-python transformers torch
   ```

3. **Configure Environment**
   ```bash
   # Load node-specific configuration
   source /opt/llama-distributed/node-XX.sh
   # Or manually set:
   export LLAMA_SHARD_ID=0
   export LLAMA_LAYER_START=0
   export LLAMA_LAYER_END=8
   export QUIC_SERVER_PORT=7001
   ```

4. **Start Server**
   ```bash
   cd /opt/llama-distributed
   python3 quic_tracker_server_ai.py $QUIC_SERVER_PORT
   ```

### Phase 3: Verification

1. **Check Node Status**
   - Each node should log "I'm ALIVE!" with capabilities
   - Verify layer range in logs

2. **Test Pipeline**
   ```powershell
   # Connect client to Node 0
   .\target\release\client.exe console 162.221.207.169 7001
   
   # Test AI query (should route through pipeline)
   /ai "What is machine learning?"
   ```

3. **Monitor Performance**
   - Check memory usage per node (should be ~1/N of full model)
   - Monitor network traffic between nodes
   - Verify end-to-end latency

## Node Configuration

Each node configuration includes:

- **Shard ID**: Unique identifier (0 to N-1)
- **Layer Range**: Which layers this node processes
- **Environment Variables**: Layer limits, server config, rsync info
- **Startup Scripts**: Bash (.sh) and PowerShell (.ps1) versions

### Example Node 0 Configuration

```json
{
  "node_id": 0,
  "node_name": "node-00",
  "shard_id": 0,
  "layer_start": 0,
  "layer_end": 7,
  "has_embeddings": true,
  "has_output": false,
  "server_host": "162.221.207.169",
  "server_port": 7001
}
```

## Communication Protocol

### Inter-Node Communication

Nodes communicate via QUIC streams:
- **Node i → Node i+1**: Forward activations after processing layers
- **Activations**: Hidden state tensors at layer boundaries
- **Format**: Binary tensors over QUIC streams

### Client Communication

- **Client → Node 0**: Initial request (tokenized input)
- **Node N → Client**: Final response (token probabilities/logits)

## Memory Requirements

Per Node:
- **Model Memory**: ~ModelSize / NumNodes
- **Activation Memory**: ~BatchSize × SeqLen × HiddenDim × 4 bytes
- **Example (8B model, 4 nodes)**: ~1.2 GB model + activations

## Performance Considerations

### Latency
- **Single Node**: L layers × T_time_per_layer
- **Pipeline (N nodes)**: ~L/N × T_time_per_layer + (N-1) × T_communication
- **Bottleneck**: Last node (waits for all previous)

### Throughput
- **Pipeline Efficiency**: Improves with batch size
- **Optimal Batch**: Keep pipeline filled (N × batch_size)

### Network
- **Bandwidth**: ~HiddenDim × SeqLen × 4 bytes per token
- **Example (4096 dim)**: ~16 KB per token

## Troubleshooting

### Node Can't Load Model
```bash
# Check rsync connection
rsync -avz zh5605@zh5605.rsync.net:/home/zh5605/model_shards/shard_cache/ /tmp/model_cache/

# Verify environment variables
echo $LLAMA_SHARD_ID
echo $LLAMA_LAYER_START
echo $LLAMA_LAYER_END
```

### Wrong Layer Range
- Verify environment variables match shard configuration
- Check `shard_metadata.json` for correct ranges
- Restart node with correct environment

### Communication Errors
- Verify QUIC server ports are accessible
- Check firewall rules between nodes
- Ensure nodes can reach each other on network

### Memory Issues
- Reduce batch size
- Check each node has enough RAM
- Verify layer range isn't too large

## Scaling

### Horizontal Scaling
- Add more nodes: Increase `NumShards` and regenerate configs
- Load balance: Multiple pipelines behind load balancer

### Vertical Scaling
- Increase batch size per node
- Use GPU acceleration per node

## Monitoring

### Key Metrics
- **Per-Node**: Memory usage, CPU, layer processing time
- **Pipeline**: End-to-end latency, throughput
- **Network**: Inter-node bandwidth, latency

### Logging
- Server logs: `/tmp/quic_server.log`
- Node status: "I'm ALIVE!" messages with capabilities
- AI processing: Layer processing logs

## Security

- **QUIC**: TLS 1.3 encryption (built-in)
- **Rsync**: SSH-based transfer
- **Authentication**: Node registration with server
- **Access Control**: Network isolation between nodes

## Maintenance

### Updating Model
1. Generate new shards
2. Upload to rsync
3. Update node configs
4. Rolling restart of nodes

### Adding Nodes
1. Generate new config for additional shards
2. Deploy new node
3. Update cluster configuration

## Support Files

- `shard_metadata.json`: Shard configuration
- `cluster_config.json`: Complete cluster setup
- `DEPLOYMENT_PLAN.md`: Node-specific deployment guide
- `node-XX.json`: Individual node config
- `node-XX.sh` / `node-XX.ps1`: Startup scripts

---

Generated: 2025-12-01
Version: 1.0


