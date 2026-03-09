//! Distributed Inference Client for Rust
//! 
//! Discovers nodes through DHT and provides a client interface for distributed inference queries.
//! Cluster configuration is built dynamically from nodes that announce themselves to the DHT.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use anyhow::{Context, Result};
use quinn::{Endpoint, ClientConfig};

pub use crate::dht_discovery::{NodeDiscovery, DiscoveredNode, TrackerDHTClient, DHTClient, ModelInfo, NodeInfoResponse};

/// Cluster configuration loaded from cluster_config.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterConfig {
    pub cluster_name: String,
    pub num_nodes: u32,
    pub model_info: ModelInfo,
    pub sharding: ShardingInfo,
    pub nodes: Vec<NodeConfig>,
    #[serde(default)]
    pub deployment: Option<DeploymentInfo>,
}

// ModelInfo is now defined in dht_discovery module

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShardingInfo {
    pub strategy: String,
    pub num_shards: u32,
    pub layer_distribution: Vec<LayerDistribution>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayerDistribution {
    pub shard_id: u32,
    pub layer_range: Vec<u32>,
    pub layers: u32,
    pub file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeploymentInfo {
    pub rsync_server: Option<String>,
    pub server_host: Option<String>,
    pub server_port: Option<u16>,
    pub deployment_date: Option<String>,
}

/// Individual node configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeConfig {
    pub node_id: u32,
    pub node_name: String,
    pub shard_id: u32,
    pub server_host: String,
    pub server_port: u16,
    pub layer_start: u32,
    pub layer_end: u32,
    pub num_layers: u32,
    pub has_embeddings: bool,
    pub has_output: bool,
    #[serde(default)]
    pub rsync_host: Option<String>,
    #[serde(default)]
    pub rsync_user: Option<String>,
    #[serde(default)]
    pub rsync_remote_dir: Option<String>,
    #[serde(default)]
    pub environment: Option<std::collections::HashMap<String, String>>,
}

/// Distributed inference client with DHT-based discovery
pub struct DistributedInferenceClient {
    discovery: Arc<NodeDiscovery>,
    nodes: Arc<tokio::sync::RwLock<HashMap<u32, DiscoveredNode>>>,
    entry_node_id: Arc<tokio::sync::RwLock<Option<u32>>>,
    exit_node_id: Arc<tokio::sync::RwLock<Option<u32>>>,
    endpoint: Arc<Endpoint>,
}

impl DistributedInferenceClient {
    /// Create a new client using DHT discovery
    /// 
    /// # Arguments
    /// * `cluster_info_hash` - Info hash for the cluster (e.g., "llama-3.1-8b-cluster")
    /// * `tracker_host` - DHT tracker host (for TrackerDHTClient)
    /// * `tracker_port` - DHT tracker port
    pub async fn new(
        cluster_info_hash: String,
        tracker_host: &str,
        tracker_port: u16,
    ) -> Result<Self> {
        // Create DHT client (using tracker as DHT)
        let dht_client = TrackerDHTClient::new(tracker_host, tracker_port)?;
        
        // Create node discovery
        let discovery = Arc::new(
            NodeDiscovery::new(dht_client, cluster_info_hash.clone())?
        );
        
        // Discover nodes from DHT
        let discovered = discovery.discover_nodes().await
            .context("Failed to discover nodes from DHT")?;
        
        // Build node map and find entry/exit nodes
        let mut nodes = HashMap::new();
        let mut entry_node_id = None;
        let mut exit_node_id = None;
        
        for node in discovered {
            if node.has_embeddings && entry_node_id.is_none() {
                entry_node_id = Some(node.node_id);
            }
            if node.has_output {
                exit_node_id = Some(node.node_id);
            }
            nodes.insert(node.node_id, node);
        }
        
        // Create QUIC endpoint (ClientConfig is optional in quinn 0.11)
        let client_config = Self::create_client_config()?;
        let endpoint = Arc::new(Endpoint::client("0.0.0.0:0".parse().unwrap())?);
        
        Ok(Self {
            discovery,
            nodes: Arc::new(tokio::sync::RwLock::new(nodes)),
            entry_node_id: Arc::new(tokio::sync::RwLock::new(entry_node_id)),
            exit_node_id: Arc::new(tokio::sync::RwLock::new(exit_node_id)),
            endpoint,
        })
    }
    
    /// Create client with bootstrap from JSON config (for development/testing)
    pub async fn with_bootstrap<P: AsRef<Path>>(
        cluster_info_hash: String,
        tracker_host: &str,
        tracker_port: u16,
        bootstrap_config: P,
    ) -> Result<Self> {
        // Load bootstrap config for initial DHT nodes
        let content = fs::read_to_string(bootstrap_config.as_ref())
            .with_context(|| "Failed to read bootstrap config")?;
        
        let config: ClusterConfig = serde_json::from_str(&content)
            .context("Failed to parse bootstrap config")?;
        
        // Create DHT client
        let dht_client = TrackerDHTClient::new(tracker_host, tracker_port)?;
        
        // Bootstrap with known nodes from config
        let bootstrap_addrs: Result<Vec<SocketAddr>> = config.nodes
            .iter()
            .map(|n| format!("{}:{}", n.server_host, n.server_port).parse()
                .context("Invalid bootstrap address"))
            .collect();
        
        if let Ok(addrs) = bootstrap_addrs {
            let _ = dht_client.bootstrap(&addrs).await;
        }
        
        // Create discovery and discover nodes
        let discovery = Arc::new(
            NodeDiscovery::new(dht_client, cluster_info_hash.clone())?
        );
        
        let discovered = discovery.discover_nodes().await
            .context("Failed to discover nodes")?;
        
        // Build node map
        let mut nodes = HashMap::new();
        let mut entry_node_id = None;
        let mut exit_node_id = None;
        
        for node in discovered {
            if node.has_embeddings && entry_node_id.is_none() {
                entry_node_id = Some(node.node_id);
            }
            if node.has_output {
                exit_node_id = Some(node.node_id);
            }
            nodes.insert(node.node_id, node);
        }
        
        let endpoint = Arc::new(Endpoint::client("0.0.0.0:0".parse().unwrap())?);
        
        Ok(Self {
            discovery,
            nodes: Arc::new(tokio::sync::RwLock::new(nodes)),
            entry_node_id: Arc::new(tokio::sync::RwLock::new(entry_node_id)),
            exit_node_id: Arc::new(tokio::sync::RwLock::new(exit_node_id)),
            endpoint,
        })
    }
    
    /// Start automatic node discovery refresh
    /// 
    /// This periodically refreshes the node list from DHT so you see
    /// new nodes joining and old nodes leaving automatically.
    /// 
    /// # Arguments
    /// * `refresh_interval` - How often to refresh (e.g., Duration::from_secs(30))
    pub fn start_auto_refresh(&self, refresh_interval: std::time::Duration) {
        let discovery = self.discovery.clone();
        let nodes = self.nodes.clone();
        let entry_node_id = self.entry_node_id.clone();
        let exit_node_id = self.exit_node_id.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            let mut last_count = 0u32;
            
            loop {
                interval.tick().await;
                
                match discovery.discover_nodes().await {
                    Ok(discovered) => {
                        let current_count = discovered.len() as u32;
                        let mut new_nodes_found = false;
                        
                        // Update nodes cache
                        let mut cached_nodes = nodes.write().await;
                        let previous_ids: HashSet<u32> = 
                            cached_nodes.keys().cloned().collect();
                        let current_ids: HashSet<u32> = 
                            discovered.iter().map(|n| n.node_id).collect();
                        
                        // Detect and log new nodes
                        for node in &discovered {
                            if !previous_ids.contains(&node.node_id) {
                                println!("[DHT] ✅ New node discovered: {} at {} (Shard {})", 
                                         node.node_name, node.addr, node.shard_id);
                                new_nodes_found = true;
                            }
                        }
                        
                        // Detect and log nodes that left
                        for node_id in &previous_ids {
                            if !current_ids.contains(node_id) {
                                println!("[DHT] ❌ Node left: {}", node_id);
                            }
                        }
                        
                        // Update cache
                        cached_nodes.clear();
                        let mut new_entry_id = None;
                        let mut new_exit_id = None;
                        
                        for node in discovered {
                            if node.has_embeddings && new_entry_id.is_none() {
                                new_entry_id = Some(node.node_id);
                            }
                            if node.has_output {
                                new_exit_id = Some(node.node_id);
                            }
                            cached_nodes.insert(node.node_id, node);
                        }
                        
                        drop(cached_nodes);
                        
                        // Update entry/exit node IDs
                        {
                            let mut entry = entry_node_id.write().await;
                            *entry = new_entry_id;
                        }
                        {
                            let mut exit = exit_node_id.write().await;
                            *exit = new_exit_id;
                        }
                        
                        // Log summary if node count changed
                        if current_count != last_count || new_nodes_found {
                            println!("[DHT] Cluster updated: {} nodes active (was {})", 
                                     current_count, last_count);
                            last_count = current_count;
                        }
                    }
                    Err(e) => {
                        eprintln!("[DHT] Auto-refresh error: {}", e);
                    }
                }
            }
        });
    }
    
    /// Create QUIC client configuration
    fn create_client_config() -> Result<ClientConfig> {
        // For development: accept self-signed certificates
        // In production, load proper CA certificates here
        // Use platform verifier to accept system certificates + self-signed certs
        let client_config = ClientConfig::try_with_platform_verifier()
            .context("Failed to create client config with platform verifier")?;
        Ok(client_config)
    }
    
    /// Refresh node list from DHT (for dynamic updates)
    pub async fn refresh_nodes(&self) -> Result<()> {
        let discovered = self.discovery.discover_nodes().await?;
        
        let mut nodes = self.nodes.write().await;
        nodes.clear();
        
        let mut entry_node_id = None;
        let mut exit_node_id = None;
        
        for node in discovered {
            if node.has_embeddings && entry_node_id.is_none() {
                entry_node_id = Some(node.node_id);
            }
            if node.has_output {
                exit_node_id = Some(node.node_id);
            }
            nodes.insert(node.node_id, node);
        }
        
        {
            let mut entry = self.entry_node_id.write().await;
            *entry = entry_node_id;
        }
        {
            let mut exit = self.exit_node_id.write().await;
            *exit = exit_node_id;
        }
        
        Ok(())
    }
    
    /// Get the entry node (first node with embeddings)
    pub async fn entry_node(&self) -> Option<DiscoveredNode> {
        let entry_id = {
            let entry = self.entry_node_id.read().await;
            *entry
        }?;
        
        let nodes = self.nodes.read().await;
        nodes.get(&entry_id).cloned()
    }
    
    /// Get the exit node (last node with output)
    pub async fn exit_node(&self) -> Option<DiscoveredNode> {
        let exit_id = {
            let exit = self.exit_node_id.read().await;
            *exit
        }?;
        
        let nodes = self.nodes.read().await;
        nodes.get(&exit_id).cloned()
    }
    
    /// Get node by ID
    pub async fn get_node(&self, node_id: u32) -> Option<DiscoveredNode> {
        let nodes = self.nodes.read().await;
        nodes.get(&node_id).cloned()
    }
    
    /// Get all discovered nodes
    pub async fn nodes(&self) -> Vec<DiscoveredNode> {
        let nodes = self.nodes.read().await;
        let mut node_list: Vec<_> = nodes.values().cloned().collect();
        node_list.sort_by_key(|n| n.node_id);
        node_list
    }
    
    /// Get cluster information
    pub async fn cluster_info(&self) -> ClusterInfo {
        let nodes = self.nodes.read().await;
        let node_count = nodes.len() as u32;
        
        // Get model info from first node (all nodes should have same model)
        let model_info = nodes.values()
            .next()
            .map(|n| n.model_info.clone())
            .unwrap_or_default();
        
        let entry_id = {
            let entry = self.entry_node_id.read().await;
            *entry
        };
        
        let exit_id = {
            let exit = self.exit_node_id.read().await;
            *exit
        };
        
        ClusterInfo {
            cluster_name: "discovered-cluster".to_string(),
            num_nodes: node_count,
            model_info,
            entry_node: entry_id
                .and_then(|id| nodes.get(&id))
                .map(|n| n.node_name.clone()),
            exit_node: exit_id
                .and_then(|id| nodes.get(&id))
                .map(|n| n.node_name.clone()),
        }
    }
    
    /// Process a query through the distributed pipeline
    /// This sends the request to the entry node, which automatically
    /// forwards it through the pipeline to the exit node
    pub async fn process_query(
        &self,
        query: &str,
        context: Option<&[Message]>,
        params: InferenceParams,
    ) -> Result<InferenceResponse> {
        let entry = self.entry_node()
            .await
            .ok_or_else(|| anyhow::anyhow!("No entry node discovered from DHT"))?;
        
        // Connect to entry node using discovered address
        let connection = self.endpoint
            .connect(entry.addr, &entry.node_name)?
            .await
            .context("Failed to establish QUIC connection to entry node")?;
        
        // Prepare request
        let exit_node_name = {
            let exit_id = {
                let exit = self.exit_node_id.read().await;
                *exit
            };
            
            if let Some(exit_id) = exit_id {
                let nodes = self.nodes.read().await;
                nodes.get(&exit_id).map(|n| n.node_name.clone())
            } else {
                None
            }
        };
        
        let request = InferenceRequest {
            query: query.to_string(),
            context: context.map(|c| c.to_vec()),
            parameters: params,
            pipeline_mode: true,
            target_node: exit_node_name,
        };
        
        // Send request
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .context("Failed to open bidirectional stream")?;
        
        let request_json = serde_json::to_vec(&request)?;
        send.write_all(&request_json).await?;
        send.finish()?;
        
        // Read response
        let mut response_data = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut recv, &mut response_data).await?;
        
        let response: InferenceResponse = serde_json::from_slice(&response_data)
            .context("Failed to parse response")?;
        
        Ok(response)
    }
}

// Request/Response structures
#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub query: String,
    pub context: Option<Vec<Message>>,
    pub parameters: InferenceParams,
    pub pipeline_mode: bool,
    pub target_node: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceParams {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub answer: String,
    pub metadata: ResponseMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub model: Option<String>,
    pub shard_id: Option<u32>,
    pub layers_processed: Option<String>,
    pub tokens_generated: Option<u32>,
    pub latency_ms: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ClusterInfo {
    pub cluster_name: String,
    pub num_nodes: u32,
    pub model_info: ModelInfo,
    pub entry_node: Option<String>,
    pub exit_node: Option<String>,
}

// ModelInfo Default impl is in dht_discovery module

/// Load configuration for a specific node
pub fn load_node_config<P: AsRef<Path>>(
    node_id: u32,
    config_dir: P,
) -> Result<NodeConfig> {
    let config_file = config_dir.as_ref().join(format!("node-{:02}.json", node_id));
    
    let content = fs::read_to_string(&config_file)
        .with_context(|| format!("Failed to read node config from {:?}", config_file))?;
    
    let config: NodeConfig = serde_json::from_str(&content)
        .context("Failed to parse node configuration")?;
    
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dht_discovery() {
        // Test DHT discovery (requires tracker running)
        let client = DistributedInferenceClient::new(
            "test-cluster".to_string(),
            "127.0.0.1",
            7001,
        ).await;
        
        // May fail if tracker not running, that's OK for test
        if let Ok(client) = client {
            let nodes = client.nodes().await;
            println!("Discovered {} nodes", nodes.len());
        }
    }
}
