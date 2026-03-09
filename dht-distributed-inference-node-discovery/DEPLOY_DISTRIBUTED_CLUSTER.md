# Deploying Distributed Server Cluster

## Overview

This guide explains how to set up multiple servers for distributed AI processing using the work distribution system.

## Architecture

```
Client Request → Server A (Primary)
                ↓ (if overloaded or unavailable)
                → Server B (Secondary)
                ↓ (if overloaded)
                → Server C (Tertiary)
```

Each server:
- Has its own complete Llama model instance
- Can process requests locally
- Can delegate work to other nodes
- Uses weighted load balancing

## Prerequisites

1. **Multiple servers** with QUIC server running
2. **Same model file** on each server (e.g., `llama-3.1-8b-instruct.Q4_K_M.gguf`)
3. **Network connectivity** between servers
4. **Python dependencies** on all servers:
   ```bash
   pip install aioquic cryptography
   ```

## Step 1: Configure Cluster

### Option A: Use PowerShell Script (Windows)

```powershell
.\setup_distributed_cluster.ps1
```

This creates `cluster_config.json` with your node configuration.

### Option B: Manual Configuration

Create `cluster_config.json`:

```json
{
  "nodes": [
    {
      "id": "ai-node-1",
      "ip": "162.221.207.169",
      "port": 7001,
      "capabilities": ["ai_processing", "file_serving", "tracker"],
      "weight": 2.0,
      "max_concurrent": 100
    },
    {
      "id": "ai-node-2",
      "ip": "192.168.1.101",
      "port": 7001,
      "capabilities": ["ai_processing"],
      "weight": 1.5,
      "max_concurrent": 80
    },
    {
      "id": "ai-node-3",
      "ip": "192.168.1.102",
      "port": 7001,
      "capabilities": ["ai_processing"],
      "weight": 1.0,
      "max_concurrent": 50
    }
  ]
}
```

## Step 2: Deploy Files to Each Server

For each server, deploy:

1. **Server files** (already deployed):
   - `quic_tracker_server_ai.py`
   - `ai_processor.py`
   - `work_distribution.py`
   - `byte_level_alpn_fix.py`

2. **New files** (need to deploy):
   - `quic_client_server.py` (server-to-server client)
   - `distributed_config.py` (cluster configuration loader)
   - `cluster_config.json` (cluster configuration)

### Deploy Script

```powershell
# Deploy to server 1
$server1 = "162.221.207.169"
$user = "dbertrand"
$pass = "Fuckstix!"

pscp -pw $pass quic_client_server.py ${user}@${server1}:~/
pscp -pw $pass distributed_config.py ${user}@${server1}:~/
pscp -pw $pass cluster_config.json ${user}@${server1}:~/
```

## Step 3: Deploy Model to Each Server

Each server needs the same model file:

```bash
# On each server
cd ~
wget https://huggingface.co/TheBloke/Llama-3.1-8B-Instruct-GGUF/resolve/main/llama-3.1-8b-instruct.Q4_K_M.gguf
```

## Step 4: Configure AI Processor on Each Server

Update `ai_processor.py` or create config:

```python
# In quic_tracker_server_ai.py or separate config file
from ai_processor import AiProcessingConfig, get_ai_processor

config = AiProcessingConfig(
    model_name="llama-3.1-8b-instruct",
    model_path="/home/dbertrand/llama-3.1-8b-instruct.Q4_K_M.gguf",
    use_gpu=True,  # If GPU available
    gpu_layers=35,
    context_window=128000,
    temperature=0.7,
    max_tokens=512,
)

ai_processor = get_ai_processor(config)
```

## Step 5: Start Servers

Start server on each node:

```bash
# On each server
cd ~
python3 -u quic_tracker_server_ai.py 7001 > /tmp/quic_server.log 2>&1 &
```

Or use the deployment script:

```powershell
.\start_ubuntu_server.ps1
```

## Step 6: Verify Cluster

### Check Node Registration

Look for logs like:
```
[WORK_DIST] Registered node: ai-node-1 (162.221.207.169:7001) with capabilities: ['ai_processing', 'file_serving', 'tracker'], weight=2.0
[WORK_DIST] Registered node: ai-node-2 (192.168.1.101:7001) with capabilities: ['ai_processing'], weight=1.5
```

### Test Work Distribution

Send an AI request and watch logs:

**Server A (receives request):**
```
[REQUEST] REQUEST_TYPE: AiRequest
[WORK_DIST] Attempting work delegation
[WORK_DIST] Selected node: ai-node-2 (192.168.1.101:7001) (weight=1.5, load=5/80)
[QUIC_CLIENT] Connecting to 192.168.1.101:7001
[QUIC_CLIENT] Sent request on stream 0
[QUIC_CLIENT] Received response from 192.168.1.101:7001
[WORK_DIST] Delegated work completed successfully
```

**Server B (processes request):**
```
[REQUEST] REQUEST_TYPE: AiRequest
[AI_PROCESSOR] process_query() called
[AI_PROCESSOR] process_query() returned
```

## Weight Configuration

### Weight Guidelines

- **Weight 2.0+**: Primary/high-capacity nodes (more work)
- **Weight 1.0-1.5**: Secondary nodes (medium work)
- **Weight 0.5-1.0**: Backup nodes (less work)

### Load Balancing

The system uses **effective weight** = `weight * (1 - load_factor)`

- Node with 0 active requests: full weight
- Node with 50% capacity used: 50% weight
- Node at capacity: weight = 0 (won't be selected)

## Troubleshooting

### Connection Failures

```bash
# Check firewall
sudo ufw allow 7001/udp

# Test connectivity
nc -u 162.221.207.169 7001
```

### Model Not Found

```bash
# Verify model file exists
ls -lh ~/llama-3.1-8b-instruct.Q4_K_M.gguf

# Check permissions
chmod 644 ~/llama-3.1-8b-instruct.Q4_K_M.gguf
```

### Work Delegation Not Working

1. Check `cluster_config.json` exists on each server
2. Verify node IPs are correct
3. Check server logs for registration messages
4. Ensure `quic_client_server.py` is deployed

## Monitoring

### View Cluster Status

```python
# In Python shell on server
from work_distribution import WorkDistributionManager
from distributed_config import load_cluster_from_config

work_dist = load_cluster_from_config('~/cluster_config.json')

# List all nodes
for node_id, node_info in work_dist.nodes.items():
    print(f"{node_id}: {node_info.ip}:{node_info.port} - "
          f"load={node_info.active_requests}/{node_info.max_concurrent}, "
          f"weight={node_info.weight}")
```

### Log Monitoring

```bash
# Watch work distribution
tail -f ~/quic_tracker_server.log | grep WORK_DIST

# Watch AI processing
tail -f ~/quic_tracker_server.log | grep AI_PROCESSOR

# Watch QUIC client (delegation)
tail -f ~/quic_tracker_server.log | grep QUIC_CLIENT
```

## Performance Tips

1. **Place primary node close to clients** (lowest latency)
2. **Use higher weights for nodes with GPUs** (faster processing)
3. **Monitor load** and adjust weights dynamically
4. **Use health checks** to remove unhealthy nodes (future enhancement)

## Next Steps

- Implement health checks
- Add dynamic weight adjustment
- Implement node discovery (automatic registration)
- Add metrics collection

