# Rust Client: DHT-Based Node Discovery Usage

## Overview

The client now uses **DHT-based discovery** where nodes announce themselves to the DHT, and clients discover node IP addresses dynamically. The cluster configuration IPs come from nodes that announce themselves to the DHT.

## Architecture Flow

```
1. Inference nodes start up
   ↓
2. Each node announces itself to DHT with cluster info_hash
   ↓
3. DHT stores: info_hash → [node_ip:port, node_ip:port, ...]
   ↓
4. Client queries DHT for cluster info_hash
   ↓
5. DHT returns list of active node IPs
   ↓
6. Client connects to each node to get metadata
   ↓
7. Client builds cluster config from discovered nodes
   ↓
8. Client uses discovered nodes for inference
```

## Client Usage

### Basic DHT Discovery

```rust
use distributed_client::DistributedInferenceClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Create client - discovers nodes from DHT
    let client = DistributedInferenceClient::new(
        "llama-3.1-8b-cluster".to_string(),  // Cluster info_hash
        "162.221.207.169",                    // DHT tracker host
        7001,                                  // DHT tracker port
    ).await?;
    
    // Process queries (uses discovered nodes)
    let response = client.process_query(
        "Your question",
        None,
        InferenceParams {
            temperature: 0.7,
            max_tokens: 512,
            top_p: 0.9,
        },
    ).await?;
    
    println!("Answer: {}", response.answer);
    Ok(())
}
```

### With Bootstrap (Development)

For development/testing, you can bootstrap DHT with nodes from JSON config:

```rust
let client = DistributedInferenceClient::with_bootstrap(
    "llama-3.1-8b-cluster".to_string(),
    "162.221.207.169",
    7001,
    "./node_configs/cluster_config.json",  // Bootstrap from JSON
).await?;
```

## Node Announcement

Each inference node must announce itself to the DHT:

```rust
use distributed_client::{NodeAnnouncer, NodeConfig, ModelInfo};

// Load this node's config
let node_config: NodeConfig = serde_json::from_str(
    &std::fs::read_to_string("./node_configs/node-00.json")?
)?;

let announcer = NodeAnnouncer::new(
    "llama-3.1-8b-cluster".to_string(),  // Cluster info_hash
    node_config,
    model_info,
    "162.221.207.169",                    // Tracker host
    7001,                                  // Tracker port
    7001,                                  // This node's QUIC port
)?;

// Start announcing (announces every 60 seconds)
announcer.start().await?;
```

## How It Works

1. **Node Startup**: Each node loads its config, then announces to DHT
2. **Client Discovery**: Client queries DHT, gets list of node IPs
3. **Metadata Fetch**: Client connects to each discovered IP to get node details
4. **Cluster Building**: Client builds cluster config from discovered nodes
5. **Query Processing**: Client routes queries through discovered entry node

## Info Hash Strategy

All nodes for the same model/cluster use the same info_hash:
- Cluster: `llama-3.1-8b-cluster` (discover all nodes)
- Shard-specific: `llama-3.1-8b-shard-00` (discover specific shard)

## Dynamic Updates

Clients can refresh the node list as nodes join/leave:

```rust
// Refresh discovered nodes
client.refresh_nodes().await?;

// Check current nodes
let nodes = client.nodes().await;
println!("Currently {} nodes discovered", nodes.len());
```

## Key Differences from Static Config

| Static Config (Old) | DHT Discovery (New) |
|---------------------|---------------------|
| Load from JSON file | Query DHT |
| Fixed IP addresses | Dynamic IPs from announcements |
| Manual updates needed | Automatic discovery |
| Nodes must be pre-configured | Nodes self-register |

## Benefits

- **No static configuration**: Nodes can join/leave dynamically
- **Automatic discovery**: Clients find nodes automatically
- **Distributed**: No central configuration server needed
- **Scalable**: New nodes appear in cluster automatically
- **Resilient**: Node failures handled gracefully

The JSON configs are still generated for:
- Node startup (nodes read their own config)
- Development/testing bootstrap
- Documentation of expected cluster structure
