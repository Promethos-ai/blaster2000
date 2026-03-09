//! Node announcement to DHT
//! 
//! Inference nodes use this to announce themselves to the DHT
//! so clients can discover them.

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::time::{interval, Duration};

use crate::dht_discovery::{DHTClient, TrackerDHTClient, NodeInfoResponse, ModelInfo};
use crate::distributed_client::NodeConfig;

/// Node announcer - announces this node to DHT
pub struct NodeAnnouncer {
    dht_client: Arc<dyn DHTClient>,
    cluster_info_hash: String,
    node_config: NodeConfig,
    model_info: ModelInfo,
    port: u16,
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl NodeAnnouncer {
    /// Create a new node announcer
    pub fn new(
        cluster_info_hash: String,
        node_config: NodeConfig,
        model_info: ModelInfo,
        tracker_host: &str,
        tracker_port: u16,
        node_port: u16,
    ) -> Result<Self> {
        let dht_client = Arc::new(TrackerDHTClient::new(tracker_host, tracker_port)?);
        
        Ok(Self {
            dht_client,
            cluster_info_hash,
            node_config,
            model_info,
            port: node_port,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }
    
    /// Start announcing to DHT
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // Initial announce
        self.announce_once().await?;
        
        // Start periodic announcement task
        let cluster_hash = self.cluster_info_hash.clone();
        let dht_client = self.dht_client.clone();
        let port = self.port;
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Announce every 60 seconds
            
            loop {
                interval.tick().await;
                
                // Check if still running
                {
                    let r = running.read().await;
                    if !*r {
                        break;
                    }
                }
                
                // Re-announce
                if let Err(e) = dht_client.announce(&cluster_hash, port).await {
                    eprintln!("Failed to re-announce to DHT: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop announcing
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        
        // Send "stopped" event (if DHT supports it)
        // This would require extending DHTClient trait
    }
    
    /// Announce once
    async fn announce_once(&self) -> Result<()> {
        self.dht_client
            .announce(&self.cluster_info_hash, self.port)
            .await
            .context("Failed to announce to DHT")
    }
    
    /// Get node info response (for NodeInfoRequest)
    pub fn get_node_info_response(&self) -> NodeInfoResponse {
        NodeInfoResponse {
            node_id: self.node_config.node_id,
            node_name: self.node_config.node_name.clone(),
            shard_id: self.node_config.shard_id,
            layer_start: self.node_config.layer_start,
            layer_end: self.node_config.layer_end,
            num_layers: self.node_config.num_layers,
            has_embeddings: self.node_config.has_embeddings,
            has_output: self.node_config.has_output,
            model_info: self.model_info.clone(),
        }
    }
}
