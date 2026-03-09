//! DHT-based node discovery for distributed inference cluster
//! 
//! Nodes announce themselves to the DHT with cluster info_hash.
//! Clients query the DHT to discover active node IP addresses.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
// AsyncReadExt/AsyncWriteExt used via tokio::io:: prefix
use quinn;

/// DHT client interface for node discovery
#[async_trait::async_trait]
pub trait DHTClient: Send + Sync {
    /// Get peers (node IPs) from DHT for a given info_hash
    async fn get_peers(&self, info_hash: &str) -> Result<Vec<SocketAddr>>;
    
    /// Announce this node to the DHT
    async fn announce(&self, info_hash: &str, port: u16) -> Result<()>;
    
    /// Bootstrap DHT with known nodes
    async fn bootstrap(&self, nodes: &[SocketAddr]) -> Result<()>;
}

/// Tracker-based DHT client (uses QUIC tracker as DHT)
pub struct TrackerDHTClient {
    tracker_addr: SocketAddr,
    quic_endpoint: Arc<quinn::Endpoint>,
}

impl TrackerDHTClient {
    pub fn new(tracker_host: &str, tracker_port: u16) -> Result<Self> {
        let tracker_addr = format!("{}:{}", tracker_host, tracker_port)
            .parse()
            .context("Invalid tracker address")?;
        
        let endpoint = Arc::new(quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())?);
        
        Ok(Self {
            tracker_addr,
            quic_endpoint: endpoint,
        })
    }
}

#[async_trait::async_trait]
impl DHTClient for TrackerDHTClient {
    async fn get_peers(&self, info_hash: &str) -> Result<Vec<SocketAddr>> {
        // Connect to tracker and request peers for this info_hash
        let connection = self.quic_endpoint
            .connect(self.tracker_addr, "localhost")?
            .await
            .context("Failed to connect to tracker")?;
        
        // Send announce request to get peer list
        let request = serde_json::json!({
            "type": "TrackerAnnounceRequest",
            "info_hash": info_hash,
            "peer_id": generate_peer_id(),
            "port": 0,  // We just want to get peers, not announce
            "event": "none",
        });
        
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .context("Failed to open stream")?;
        
        let request_bytes = serde_json::to_vec(&request)?;
        send.write_all(&request_bytes).await?;
        send.finish()?;
        
        // Read response
        let mut response_bytes = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut recv, &mut response_bytes).await?;
        
        let response: TrackerResponse = serde_json::from_slice(&response_bytes)
            .context("Failed to parse tracker response")?;
        
        // Convert peer list to SocketAddrs
        let peers: Result<Vec<SocketAddr>> = response.peers
            .iter()
            .map(|p| format!("{}:{}", p.ip, p.port).parse()
                .context("Invalid peer address"))
            .collect();
        
        peers
    }
    
    async fn announce(&self, info_hash: &str, port: u16) -> Result<()> {
        let connection = self.quic_endpoint
            .connect(self.tracker_addr, "localhost")?
            .await
            .context("Failed to connect to tracker")?;
        
        // Get our external IP (simplified - in production use proper method)
        let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        
        let request = serde_json::json!({
            "type": "TrackerAnnounceRequest",
            "info_hash": info_hash,
            "peer_id": generate_peer_id(),
            "port": port,
            "uploaded": 0,
            "downloaded": 0,
            "left": 0,
            "event": "started",
            "ip": local_ip,
        });
        
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .context("Failed to open stream")?;
        
        let request_bytes = serde_json::to_vec(&request)?;
        send.write_all(&request_bytes).await?;
        send.finish()?;
        
        // Read response to confirm
        let mut response_bytes = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut recv, &mut response_bytes).await?;
        
        Ok(())
    }
    
    async fn bootstrap(&self, _nodes: &[SocketAddr]) -> Result<()> {
        // Tracker already knows about nodes, no bootstrap needed
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct TrackerResponse {
    interval: Option<u32>,
    peers: Vec<PeerInfo>,
    complete: Option<u32>,
    incomplete: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PeerInfo {
    ip: String,
    port: u16,
}

fn generate_peer_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:020}", timestamp)
}

fn get_local_ip() -> Option<String> {
    // Simplified - in production use proper network interface detection
    // For now, return None to use default
    None
}

/// Discovered node from DHT
#[derive(Debug, Clone)]
pub struct DiscoveredNode {
    pub addr: SocketAddr,
    pub node_id: u32,
    pub node_name: String,
    pub shard_id: u32,
    pub layer_start: u32,
    pub layer_end: u32,
    pub num_layers: u32,
    pub has_embeddings: bool,
    pub has_output: bool,
    pub model_info: ModelInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub n_layers: u32,
    pub n_ctx: u32,
    pub n_vocab: u32,
    pub n_embd: u32,
    pub n_head: u32,
    pub n_kv_head: u32,
}

impl Default for ModelInfo {
    fn default() -> Self {
        Self {
            n_layers: 0,
            n_ctx: 0,
            n_vocab: 0,
            n_embd: 0,
            n_head: 0,
            n_kv_head: 0,
        }
    }
}

/// Node discovery manager
pub struct NodeDiscovery {
    dht_client: Arc<dyn DHTClient>,
    cluster_info_hash: String,
    discovered_nodes: Arc<RwLock<HashMap<u32, DiscoveredNode>>>,
    quic_endpoint: Arc<quinn::Endpoint>,
}

impl NodeDiscovery {
    pub fn new<D: DHTClient + 'static>(
        dht_client: D,
        cluster_info_hash: String,
    ) -> Result<Self> {
        let endpoint = Arc::new(quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())?);
        
        Ok(Self {
            dht_client: Arc::new(dht_client),
            cluster_info_hash,
            discovered_nodes: Arc::new(RwLock::new(HashMap::new())),
            quic_endpoint: endpoint,
        })
    }
    
    /// Discover all nodes from DHT
    pub async fn discover_nodes(&self) -> Result<Vec<DiscoveredNode>> {
        // Query DHT for cluster info_hash
        let peer_addrs = self.dht_client
            .get_peers(&self.cluster_info_hash)
            .await
            .context("Failed to query DHT for nodes")?;
        
        let mut discovered = Vec::new();
        
        // Query each peer for node metadata
        for addr in peer_addrs {
            match self.fetch_node_metadata(addr).await {
                Ok(node_info) => {
                    discovered.push(node_info.clone());
                    
                    // Store in cache
                    let mut nodes = self.discovered_nodes.write().await;
                    nodes.insert(node_info.node_id, node_info);
                }
                Err(e) => {
                    eprintln!("Failed to fetch metadata from {}: {}", addr, e);
                    // Continue with other nodes
                }
            }
        }
        
        Ok(discovered)
    }
    
    /// Fetch node metadata from a discovered peer
    async fn fetch_node_metadata(&self, addr: SocketAddr) -> Result<DiscoveredNode> {
        // Connect via QUIC and request node info
        let connection = self.quic_endpoint
            .connect(addr, "localhost")?
            .await
            .context("Failed to connect to node")?;
        
        // Request node information
        let request = serde_json::json!({
            "type": "NodeInfoRequest",
        });
        
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .context("Failed to open stream")?;
        
        let request_bytes = serde_json::to_vec(&request)?;
        send.write_all(&request_bytes).await?;
        send.finish()?;
        
        // Read response
        let mut response_bytes = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut recv, &mut response_bytes).await?;
        
        let node_info: NodeInfoResponse = serde_json::from_slice(&response_bytes)
            .context("Failed to parse node info response")?;
        
        Ok(DiscoveredNode {
            addr,
            node_id: node_info.node_id,
            node_name: node_info.node_name,
            shard_id: node_info.shard_id,
            layer_start: node_info.layer_start,
            layer_end: node_info.layer_end,
            num_layers: node_info.num_layers,
            has_embeddings: node_info.has_embeddings,
            has_output: node_info.has_output,
            model_info: node_info.model_info,
        })
    }
    
    /// Refresh discovered nodes
    pub async fn refresh(&self) -> Result<Vec<DiscoveredNode>> {
        let mut nodes = self.discovered_nodes.write().await;
        nodes.clear();
        drop(nodes);
        
        self.discover_nodes().await
    }
    
    /// Get discovered nodes
    pub async fn get_nodes(&self) -> HashMap<u32, DiscoveredNode> {
        self.discovered_nodes.read().await.clone()
    }
}

#[derive(Debug, Deserialize)]
pub struct NodeInfoResponse {
    pub node_id: u32,
    pub node_name: String,
    pub shard_id: u32,
    pub layer_start: u32,
    pub layer_end: u32,
    pub num_layers: u32,
    pub has_embeddings: bool,
    pub has_output: bool,
    pub model_info: ModelInfo,
}
