# Rust Client: DHT-Based Node Discovery

## Overview

Instead of static JSON configuration files, the cluster configuration will be **dynamically discovered through the Torrent DHT**. Nodes announce themselves to the DHT, and clients query the DHT to find available inference nodes.

## Architecture

### Discovery Flow

```
1. Nodes announce themselves to DHT
   ↓
2. DHT stores (info_hash → [node_ip:port])
   ↓
3. Client queries DHT for cluster info_hash
   ↓
4. DHT returns list of active node IPs
   ↓
5. Client builds cluster config from discovered IPs
   ↓
6. Client connects to nodes via QUIC
```

### Info Hash Strategy

Each model/shard combination has its own info_hash:
- **Cluster info_hash**: Discover all nodes (e.g., `llama-3.1-8b-cluster`)
- **Shard-specific info_hash**: Discover nodes for specific shard (e.g., `llama-3.1-8b-shard-00`)

## Updated Client Implementation

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use anyhow::{Context, Result};
use tokio::time::{timeout, Duration};

/// DHT-based node discovery
pub struct DHTNodeDiscovery {
    dht_client: DHTClient,  // Your DHT implementation
    cluster_info_hash: String,
    bootstrap_nodes: Vec<SocketAddr>,
}

impl DHTNodeDiscovery {
    /// Create DHT discovery with bootstrap nodes from config
    pub fn new(cluster_info_hash: String, bootstrap_config: Option<&ClusterConfig>) -> Self {
        let bootstrap_nodes = bootstrap_config
            .map(|cfg| cfg.nodes.iter()
                .map(|n| format!("{}:{}", n.server_host, n.server_port).parse().unwrap())
                .collect())
            .unwrap_or_default();
        
        Self {
            dht_client: DHTClient::new(),
            cluster_info_hash,
            bootstrap_nodes,
        }
    }
    
    /// Discover all nodes from DHT
    pub async fn discover_nodes(&mut self) -> Result<Vec<DiscoveredNode>> {
        // Query DHT for cluster info_hash
        let peers = self.dht_client
            .get_peers(&self.cluster_info_hash)
            .await
            .context("Failed to query DHT for nodes")?;
        
        // Convert peers to discovered nodes
        let mut nodes = Vec::new();
        for peer in peers {
            // Connect to peer and get node metadata
            if let Ok(node_info) = self.fetch_node_metadata(peer).await {
                nodes.push(node_info);
            }
        }
        
        Ok(nodes)
    }
    
    /// Fetch node metadata from discovered peer
    async fn fetch_node_metadata(&self, addr: SocketAddr) -> Result<DiscoveredNode> {
        // Connect via QUIC and request node info
        let node_info = self.query_node_info(addr).await?;
        Ok(DiscoveredNode {
            addr,
            node_id: node_info.node_id,
            node_name: node_info.node_name,
            shard_id: node_info.shard_id,
            layer_start: node_info.layer_start,
            layer_end: node_info.layer_end,
            has_embeddings: node_info.has_embeddings,
            has_output: node_info.has_output,
            model_info: node_info.model_info,
        })
    }
    
    /// Query a node for its configuration
    async fn query_node_info(&self, addr: SocketAddr) -> Result<NodeInfoResponse> {
        // Use your existing QUIC client to query node
        // Send: {"type": "node_info_request"}
        // Receive: NodeInfoResponse with node details
        todo!("Implement QUIC query to node")
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredNode {
    pub addr: SocketAddr,
    pub node_id: u32,
    pub node_name: String,
    pub shard_id: u32,
    pub layer_start: u32,
    pub layer_end: u32,
    pub has_embeddings: bool,
    pub has_output: bool,
    pub model_info: ModelInfo,
}

#[derive(Debug, Deserialize)]
pub struct NodeInfoResponse {
    pub node_id: u32,
    pub node_name: String,
    pub shard_id: u32,
    pub layer_start: u32,
    pub layer_end: u32,
    pub has_embeddings: bool,
    pub has_output: bool,
    pub model_info: ModelInfo,
}

/// Updated client that uses DHT discovery
pub struct DHTDistributedClient {
    discovery: DHTNodeDiscovery,
    nodes: HashMap<u32, DiscoveredNode>,
    entry_node: Option<u32>,
    exit_node: Option<u32>,
    endpoint: Arc<Endpoint>,
}

impl DHTDistributedClient {
    /// Create client with DHT-based discovery
    pub async fn new(
        cluster_info_hash: String,
        bootstrap_config: Option<ClusterConfig>,  // Optional bootstrap
    ) -> Result<Self> {
        let mut discovery = DHTNodeDiscovery::new(
            cluster_info_hash,
            bootstrap_config.as_ref(),
        );
        
        // Discover nodes from DHT
        let discovered_nodes = discovery.discover_nodes().await
            .context("Failed to discover nodes from DHT")?;
        
        // Build node map
        let mut nodes = HashMap::new();
        let mut entry_node = None;
        let mut exit_node = None;
        
        for node in discovered_nodes {
            if node.has_embeddings && entry_node.is_none() {
                entry_node = Some(node.node_id);
            }
            if node.has_output {
                exit_node = Some(node.node_id);
            }
            nodes.insert(node.node_id, node);
        }
        
        // Create QUIC endpoint
        let endpoint = Arc::new(Endpoint::client("0.0.0.0:0".parse().unwrap())?);
        
        Ok(Self {
            discovery,
            nodes,
            entry_node,
            exit_node,
            endpoint,
        })
    }
    
    /// Refresh node list from DHT (for dynamic updates)
    pub async fn refresh_nodes(&mut self) -> Result<()> {
        let discovered = self.discovery.discover_nodes().await?;
        
        // Update node map
        self.nodes.clear();
        self.entry_node = None;
        self.exit_node = None;
        
        for node in discovered {
            if node.has_embeddings && self.entry_node.is_none() {
                self.entry_node = Some(node.node_id);
            }
            if node.has_output {
                self.exit_node = Some(node.node_id);
            }
            self.nodes.insert(node.node_id, node);
        }
        
        Ok(())
    }
    
    /// Process query using discovered nodes
    pub async fn process_query(
        &self,
        query: &str,
        context: Option<&[Message]>,
        params: InferenceParams,
    ) -> Result<InferenceResponse> {
        let entry_id = self.entry_node
            .ok_or_else(|| anyhow::anyhow!("No entry node discovered"))?;
        
        let entry_node = self.nodes.get(&entry_id)
            .ok_or_else(|| anyhow::anyhow!("Entry node not found"))?;
        
        // Connect and send request
        let connection = self.endpoint
            .connect(entry_node.addr, &entry_node.node_name)?
            .await
            .context("Failed to connect to entry node")?;
        
        let request = InferenceRequest {
            query: query.to_string(),
            context: context.map(|c| c.to_vec()),
            parameters: params,
            pipeline_mode: true,
        };
        
        // Send request (same as before)
        let (mut send, mut recv) = connection.open_bi().await?;
        let request_json = serde_json::to_vec(&request)?;
        send.write_all(&request_json).await?;
        send.finish().await?;
        
        let mut response_data = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut recv, &mut response_data).await?;
        
        let response: InferenceResponse = serde_json::from_slice(&response_data)?;
        Ok(response)
    }
}

// DHT Client interface (implement based on your DHT library)
pub trait DHTClient {
    async fn get_peers(&mut self, info_hash: &str) -> Result<Vec<SocketAddr>>;
    async fn announce(&mut self, info_hash: &str, port: u16) -> Result<()>;
}

// Example implementation using a DHT library (adjust based on your choice)
pub struct DHTClientImpl {
    // Your DHT client implementation
}

impl DHTClient for DHTClientImpl {
    async fn get_peers(&mut self, info_hash: &str) -> Result<Vec<SocketAddr>> {
        // Query DHT for info_hash
        // Returns list of (IP, port) tuples
        todo!("Implement DHT get_peers")
    }
    
    async fn announce(&mut self, info_hash: &str, port: u16) -> Result<()> {
        // Announce to DHT that we're sharing this info_hash
        todo!("Implement DHT announce")
    }
}
```

## Node Announcement (Server Side)

Each inference node should announce itself to the DHT:

```rust
// In your inference node code
pub struct InferenceNode {
    node_config: NodeConfig,
    dht_client: DHTClient,
}

impl InferenceNode {
    pub async fn announce_to_dht(&mut self) -> Result<()> {
        // Create cluster info_hash
        let cluster_hash = self.generate_cluster_info_hash();
        
        // Announce this node to DHT
        self.dht_client.announce(
            &cluster_hash,
            self.node_config.server_port,
        ).await?;
        
        // Also announce for shard-specific hash
        let shard_hash = format!("{}-shard-{:02}", cluster_hash, self.node_config.shard_id);
        self.dht_client.announce(&shard_hash, self.node_config.server_port).await?;
        
        Ok(())
    }
    
    fn generate_cluster_info_hash(&self) -> String {
        // Generate consistent hash from model name + version
        // e.g., "llama-3.1-8b-cluster"
        format!("llama-3.1-8b-cluster")
    }
}
```

## Hybrid Approach (DHT + Bootstrap)

For initial connection, use JSON config as bootstrap:

```rust
pub async fn create_client_with_bootstrap(
    cluster_info_hash: String,
    bootstrap_path: Option<&Path>,
) -> Result<DHTDistributedClient> {
    let bootstrap_config = if let Some(path) = bootstrap_path {
        // Load initial nodes from JSON for bootstrap
        Some(load_cluster_config(path)?)
    } else {
        None
    };
    
    // Use bootstrap nodes to bootstrap DHT
    let client = DHTDistributedClient::new(cluster_info_hash, bootstrap_config).await?;
    
    Ok(client)
}
```

## Usage Example

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Option 1: Pure DHT discovery (no bootstrap)
    let mut client = DHTDistributedClient::new(
        "llama-3.1-8b-cluster".to_string(),
        None,
    ).await?;
    
    // Option 2: DHT with bootstrap from JSON config
    let mut client = DHTDistributedClient::new(
        "llama-3.1-8b-cluster".to_string(),
        Some(load_cluster_config("./node_configs/cluster_config.json")?),
    ).await?;
    
    // Process queries
    let response = client.process_query(
        "Your question",
        None,
        InferenceParams {
            temperature: 0.7,
            max_tokens: 512,
            top_p: 0.9,
        },
    ).await?;
    
    // Refresh node list periodically (nodes may join/leave)
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if let Err(e) = client.refresh_nodes().await {
                eprintln!("Failed to refresh nodes: {}", e);
            }
        }
    });
    
    Ok(())
}
```

## DHT Library Options for Rust

1. **rust-dht** - Pure Rust DHT implementation
2. **libp2p** - Full P2P stack including DHT
3. **kademlia-rust** - Kademlia DHT implementation

Example with libp2p:

```rust
use libp2p::{
    kad::{record::store::MemoryStore, Kademlia, KademliaEvent},
    Swarm, SwarmBuilder,
};

// DHT setup with libp2p
let local_key = identity::Keypair::generate_ed25519();
let local_peer_id = PeerId::from(local_key.public());

let transport = libp2p::development_transport(local_key).await?;
let behaviour = Kademlia::new(local_peer_id, MemoryStore::new(local_peer_id)).await?;

let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
```

## Summary

- **Nodes announce** themselves to DHT with cluster info_hash
- **Clients query DHT** to discover active nodes
- **JSON config** can be used for bootstrap/initial discovery
- **Dynamic updates** - nodes can join/leave without config changes
- **No static IPs** - all discovery happens through DHT

The generated JSON configs are still useful for:
- Bootstrapping initial DHT connections
- Development/testing (skip DHT)
- Documentation of expected cluster structure
- Node startup (nodes can load their own config)
